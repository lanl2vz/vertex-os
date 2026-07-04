use super::vfs_paths::{resolve_vfs_path_under_root, split_vfs_parent_child};
use super::vfs_wire::serial_write_vfs_mount_flags;
use super::*;

#[derive(Clone, Copy)]
pub(super) struct RuntimeReapTarget {
    pub(super) pid: ProcessId,
    pub(super) name: &'static str,
    pub(super) cr3: u64,
}

#[derive(Clone, Copy)]
pub(super) struct StagingBuild {
    pub(super) initial_context: ProcessContext,
    pub(super) old_contexts: [Option<RuntimeReapTarget>; MAX_PROCESSES],
    pub(super) old_context_count: usize,
}

fn state_volume_vfs_name(id: &'static str) -> Result<VfsName, InitError> {
    VfsName::from_static(state_volume_mount_component(id)?)
}

fn state_volume_vfs_path(id: &'static str) -> Result<VfsPath, InitError> {
    let component = state_volume_mount_component(id)?.as_bytes();
    const PREFIX: &[u8] = b"/state/";
    let len = PREFIX
        .len()
        .checked_add(component.len())
        .ok_or(InitError::InvalidBootManifest)?;
    if len > MAX_VFS_PATH_BYTES {
        return Err(InitError::InvalidBootManifest);
    }
    let mut bytes = [0u8; MAX_VFS_PATH_BYTES];
    let mut index = 0;
    while index < PREFIX.len() {
        bytes[index] = PREFIX[index];
        index += 1;
    }
    let mut component_index = 0;
    while component_index < component.len() {
        bytes[index] = component[component_index];
        index += 1;
        component_index += 1;
    }
    VfsPath::from_root_path(&bytes[..len]).map_err(|_| InitError::InvalidBootManifest)
}

pub fn init_from_boot_config(config: &'static BootRuntimeConfig) -> Result<(), InitError> {
    let build = stage_boot_config_runtime(config)?;
    commit_staged_boot_config_runtime(config, build);

    Ok(())
}

pub(super) fn stage_boot_config_runtime(
    config: &'static BootRuntimeConfig,
) -> Result<StagingBuild, InitError> {
    validate_boot_config_installable(config)?;
    let initial_index = initial_process_index(config)?;
    let initial_process = config.processes[initial_index].ok_or(InitError::InvalidBootManifest)?;
    let initial_context = load_boot_initial_context(initial_process)?;
    let (old_contexts, old_context_count) = snapshot_runtime_reap_targets();

    let result = {
        let staging = staging_runtime();
        build_boot_config_runtime(
            staging,
            config,
            initial_index,
            initial_process,
            initial_context,
        )
    };
    if result.is_err() {
        reclaim_detached_address_space(initial_process.name, initial_context.cr3);
        return result.map(|_| StagingBuild {
            initial_context,
            old_contexts,
            old_context_count,
        });
    }

    Ok(StagingBuild {
        initial_context,
        old_contexts,
        old_context_count,
    })
}

pub(super) fn commit_staged_boot_config_runtime(
    config: &'static BootRuntimeConfig,
    build: StagingBuild,
) {
    boot_manager().start_boot(config.generation_id);
    release_all_runtime_dma_mappings();
    commit_staging_runtime();
    install_runtime_interrupt_masks(config);
    print_boot_tables(runtime());

    if build.old_context_count > 0 {
        unsafe {
            gdt::switch_address_space(build.initial_context.cr3);
        }
        if reap_runtime_contexts(&build.old_contexts, build.old_context_count).is_err() {
            serial::write_str("Krust old runtime address-space reap incomplete\n");
        }
    }
}

fn build_boot_config_runtime(
    runtime: &mut RuntimeState,
    config: &'static BootRuntimeConfig,
    initial_index: usize,
    initial_process: BootProcessConfig,
    initial_context: ProcessContext,
) -> Result<(), InitError> {
    runtime.objects.reset();
    runtime.processes.reset();
    runtime.reset_capability_lifecycle(config);

    let mut endpoint_index = 0;
    while endpoint_index < config.endpoint_count {
        let endpoint = config.endpoints[endpoint_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.endpoint_ids[endpoint_index] = Some(runtime.objects.add_endpoint(endpoint.name)?);
        endpoint_index += 1;
    }
    if config.state_volume_count > 0 {
        runtime.state_vfs_request_endpoint = Some(
            runtime
                .objects
                .add_endpoint(STATE_VFS_REQUEST_ENDPOINT_NAME)?,
        );
        runtime.state_vfs_reply_endpoint = Some(
            runtime
                .objects
                .add_endpoint(STATE_VFS_REPLY_ENDPOINT_NAME)?,
        );
    }
    runtime.vertexfs_device_request_endpoint = Some(
        runtime
            .objects
            .add_endpoint(VERTEXFS_DEVICE_REQUEST_ENDPOINT_NAME)?,
    );
    runtime.vertexfs_device_reply_endpoint = Some(
        runtime
            .objects
            .add_endpoint(VERTEXFS_DEVICE_REPLY_ENDPOINT_NAME)?,
    );
    if boot_config_has_process(config, BLOCK_DRIVER_PROCESS_NAME)? {
        runtime.generation_metadata_block_request_endpoint = Some(
            runtime
                .objects
                .add_endpoint(GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME)?,
        );
        runtime.generation_metadata_block_reply_endpoint = Some(
            runtime
                .objects
                .add_endpoint(GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME)?,
        );
    }

    let mut store_index = 0;
    while store_index < config.store_object_count {
        let object = config.store_objects[store_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.store_object_ids[store_index] = Some(runtime.objects.add_store_object(
            object.id,
            object.base,
            object.length,
            object.hash,
        )?);
        store_index += 1;
    }
    let mut state_index = 0;
    while state_index < config.state_volume_count {
        let state = config.state_volumes[state_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.state_volume_ids[state_index] = Some(runtime.objects.add_state_volume(state)?);
        state_index += 1;
    }
    let mut network_index = 0;
    while network_index < config.network_port_count {
        let port = config.network_ports[network_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.network_port_ids[network_index] = Some(runtime.objects.add_network_port(port.id)?);
        network_index += 1;
    }

    let mut io_index = 0;
    while io_index < config.io_port_count {
        let port = config.io_ports[io_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.io_port_ids[io_index] = Some(runtime.objects.add_io_port(
            port.id,
            port.base,
            port.length,
        )?);
        io_index += 1;
    }

    let mut mmio_index = 0;
    while mmio_index < config.mmio_region_count {
        let region = config.mmio_regions[mmio_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.mmio_region_ids[mmio_index] = Some(runtime.objects.add_mmio_region(
            region.id,
            region.base,
            region.length,
        )?);
        mmio_index += 1;
    }

    let mut framebuffer_index = 0;
    while framebuffer_index < config.framebuffer_count {
        let framebuffer =
            config.framebuffers[framebuffer_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.framebuffer_ids[framebuffer_index] =
            Some(add_boot_framebuffer(runtime, framebuffer.id)?);
        framebuffer_index += 1;
    }

    let mut irq_index = 0;
    while irq_index < config.interrupt_line_count {
        let line = config.interrupt_lines[irq_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.interrupt_line_ids[irq_index] =
            Some(runtime.objects.add_interrupt_line(line.id, line.line)?);
        irq_index += 1;
    }

    let mut dma_index = 0;
    while dma_index < config.dma_region_count {
        let region = config.dma_regions[dma_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.dma_region_ids[dma_index] = Some(runtime.objects.add_dma_region(
            region.id,
            region.base,
            region.length,
        )?);
        dma_index += 1;
    }

    let mut pci_index = 0;
    while pci_index < config.pci_device_count {
        let device = config.pci_devices[pci_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.pci_device_ids[pci_index] =
            Some(runtime.objects.add_pci_device(device.id, device.kind)?);
        pci_index += 1;
    }

    let mut virtio_index = 0;
    while virtio_index < config.virtio_device_count {
        let device = config.virtio_devices[virtio_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.virtio_device_ids[virtio_index] = Some(
            runtime
                .objects
                .add_virtio_device(device.id, device.transport)?,
        );
        virtio_index += 1;
    }

    runtime.timer_id = Some(runtime.objects.add_timer("monotonic-timer")?);
    install_vfs_nodes(runtime)?;
    validate_process_mount_roots(runtime, config)?;

    let mut namespace_index = 0;
    while namespace_index < config.namespace_count {
        let namespace = config.namespaces[namespace_index].ok_or(InitError::InvalidBootManifest)?;
        let mut entries = [None; MAX_NAMESPACE_ENTRIES];
        let mut entry_index = 0;
        while entry_index < namespace.entry_count {
            let entry = namespace.entries[entry_index].ok_or(InitError::InvalidBootManifest)?;
            let object = namespace_entry_object_id(runtime, entry)?;
            entries[entry_index] = Some(NamespaceEntry {
                path: entry.path,
                object,
                rights: entry.rights,
            });
            entry_index += 1;
        }
        runtime.namespace_ids[namespace_index] = Some(runtime.objects.add_namespace(
            namespace.id,
            entries,
            namespace.entry_count,
        )?);
        namespace_index += 1;
    }

    let mut vfs_root_index = 0;
    while vfs_root_index < config.vfs_root_count {
        let root = config.vfs_roots[vfs_root_index].ok_or(InitError::InvalidBootManifest)?;
        runtime.vfs_root_ids[vfs_root_index] =
            Some(runtime.objects.add_vfs_root(root.id, root.root_path)?);
        vfs_root_index += 1;
    }

    runtime.secret_id = Some(
        runtime
            .objects
            .add_secret("secret:logd-token", NATIVE_SECRET_VALUE)?,
    );
    serial::write_str("Native secret object registered: secret:logd-token storage=in-memory\n");

    let initial_mount_root = VfsPath::from_boot_root_path(initial_process.mount_root)?;
    let initial_pid = runtime.processes.add_process(
        initial_process.name,
        initial_context,
        initial_process.image_base,
        initial_process.image_length,
        ProcessState::Running,
        CapabilitySpace::new(),
        initial_mount_root,
    )?;
    install_declared_process_mounts(runtime, initial_process, initial_pid, initial_mount_root)?;
    runtime.process_template_pids[initial_index] = Some(initial_pid);
    runtime.processes.set_current(initial_pid);

    grant_config_caps_to_process(runtime, config, initial_index, initial_pid)?;

    if let Some(module) = config.manifest_module {
        if !boot_bootstrap_authority_allows(
            config,
            initial_process.graph_node,
            "boot-module:krustboot-manifest",
            "initial-manifest",
            capability::RIGHT_READ,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let module_id = runtime
            .objects
            .add_boot_module(module.name, module.base, module.length)?;
        let cap = runtime
            .new_capability(
                module_id,
                capability::RIGHT_READ,
                initial_pid,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, initial_pid, 0, cap, true)?;
    }

    let process_control_id = runtime.objects.add_process_control("process-control")?;
    runtime.process_control_id = Some(process_control_id);
    let process_control_rights = capability::RIGHT_CONTROL
        | capability::RIGHT_ALLOCATE
        | capability::RIGHT_DELEGATE
        | capability::RIGHT_REVOKE
        | capability::RIGHT_INSPECT
        | capability::RIGHT_CREATE
        | capability::RIGHT_START
        | capability::RIGHT_KILL
        | capability::RIGHT_WAIT;
    if !boot_bootstrap_authority_allows(
        config,
        initial_process.graph_node,
        "process-control",
        "initial-process-control",
        process_control_rights,
    ) {
        return Err(InitError::InvalidBootManifest);
    }
    let cap = runtime
        .new_capability(
            process_control_id,
            process_control_rights,
            initial_pid,
            0,
            ProcessId::empty(),
        )
        .map_err(|_| InitError::CapabilityTableFull)?;
    grant_process_cap_by_pid(runtime, initial_pid, 2, cap, true)?;

    let timer_id = runtime.timer_id.ok_or(InitError::InvalidBootManifest)?;
    if !boot_bootstrap_authority_allows(
        config,
        initial_process.graph_node,
        "timer:monotonic-timer",
        "initial-restart-timer",
        capability::RIGHT_CONTROL,
    ) {
        return Err(InitError::InvalidBootManifest);
    }
    let cap = runtime
        .new_capability(
            timer_id,
            capability::RIGHT_CONTROL,
            initial_pid,
            0,
            ProcessId::empty(),
        )
        .map_err(|_| InitError::CapabilityTableFull)?;
    grant_process_cap_by_pid(runtime, initial_pid, INIT_TIMER_CAP_SLOT, cap, true)?;

    Ok(())
}

fn add_boot_framebuffer(
    runtime: &mut RuntimeState,
    name: &'static str,
) -> Result<KernelObjectId, InitError> {
    let framebuffer = limine::primary_framebuffer().ok_or(InitError::InvalidBootManifest)?;
    validate_boot_framebuffer(framebuffer)?;
    let length = framebuffer
        .pitch
        .checked_mul(framebuffer.height)
        .ok_or(InitError::InvalidBootManifest)?;
    let virtual_base = framebuffer.address as u64;
    let physical_base = framebuffer_physical_base(virtual_base, length)?;

    let id = runtime.objects.add_framebuffer(
        name,
        physical_base,
        length,
        framebuffer.width,
        framebuffer.height,
        framebuffer.pitch,
        framebuffer.bpp,
        framebuffer.red_mask_size,
        framebuffer.red_mask_shift,
        framebuffer.green_mask_size,
        framebuffer.green_mask_shift,
        framebuffer.blue_mask_size,
        framebuffer.blue_mask_shift,
    )?;
    serial::write_str("Boot framebuffer registered: framebuffer=");
    serial::write_str(name);
    serial::write_str(" physical=");
    serial::write_u64_hex(physical_base);
    serial::write_str(" virtual=");
    serial::write_u64_hex(virtual_base);
    serial::write_str(" width=");
    serial::write_u64_dec(framebuffer.width);
    serial::write_str(" height=");
    serial::write_u64_dec(framebuffer.height);
    serial::write_str(" pitch=");
    serial::write_u64_dec(framebuffer.pitch);
    serial::write_str(" bpp=");
    serial::write_u64_dec(framebuffer.bpp as u64);
    serial::write_str("\n");
    Ok(id)
}

fn validate_boot_framebuffer(framebuffer: &limine::Framebuffer) -> Result<(), InitError> {
    if framebuffer.address.is_null()
        || framebuffer.width == 0
        || framebuffer.height == 0
        || framebuffer.pitch == 0
        || framebuffer.bpp != 32
        || framebuffer.memory_model != 1
        || framebuffer.red_mask_size != 8
        || framebuffer.green_mask_size != 8
        || framebuffer.blue_mask_size != 8
    {
        return Err(InitError::InvalidBootManifest);
    }
    let bytes_per_pixel = (framebuffer.bpp as u64)
        .checked_div(8)
        .ok_or(InitError::InvalidBootManifest)?;
    let min_pitch = framebuffer
        .width
        .checked_mul(bytes_per_pixel)
        .ok_or(InitError::InvalidBootManifest)?;
    let length = framebuffer
        .pitch
        .checked_mul(framebuffer.height)
        .ok_or(InitError::InvalidBootManifest)?;
    if framebuffer.pitch < min_pitch || length == 0 || length > USER_DEVICE_MAPPING_STRIDE {
        return Err(InitError::InvalidBootManifest);
    }
    Ok(())
}

fn framebuffer_physical_base(virtual_base: u64, length: u64) -> Result<u64, InitError> {
    if let Some(hhdm_offset) = limine::hhdm_offset()
        && virtual_base >= hhdm_offset
    {
        let candidate = virtual_base
            .checked_sub(hhdm_offset)
            .ok_or(InitError::InvalidBootManifest)?;
        if framebuffer_memmap_covers(candidate, length)? {
            return Ok(candidate);
        }
    }

    let memory_map = limine::memory_map().ok_or(InitError::InvalidBootManifest)?;
    let mut index = 0;
    while index < memory_map.entry_count() {
        if let Some(entry) = memory_map.entry(index)
            && entry.entry_type == limine::MEMMAP_FRAMEBUFFER
            && entry.length >= length
        {
            return Ok(entry.base);
        }
        index += 1;
    }

    Err(InitError::InvalidBootManifest)
}

fn framebuffer_memmap_covers(base: u64, length: u64) -> Result<bool, InitError> {
    let end = base
        .checked_add(length)
        .ok_or(InitError::InvalidBootManifest)?;
    let memory_map = limine::memory_map().ok_or(InitError::InvalidBootManifest)?;
    let mut index = 0;
    while index < memory_map.entry_count() {
        if let Some(entry) = memory_map.entry(index)
            && entry.entry_type == limine::MEMMAP_FRAMEBUFFER
        {
            let entry_end = entry
                .base
                .checked_add(entry.length)
                .ok_or(InitError::InvalidBootManifest)?;
            if base >= entry.base && end <= entry_end {
                return Ok(true);
            }
        }
        index += 1;
    }
    Ok(false)
}

fn commit_staging_runtime() {
    unsafe {
        core::ptr::copy_nonoverlapping(
            INSTALL_STAGING_RUNTIME.0.get() as *const RuntimeState,
            RUNTIME.0.get(),
            1,
        );
    }
}

fn install_runtime_interrupt_masks(config: &BootRuntimeConfig) {
    timer::reset_legacy_irq_masks();
    let mut irq_index = 0;
    while irq_index < config.interrupt_line_count {
        let Some(line) = config.interrupt_lines[irq_index] else {
            return;
        };
        timer::enable_legacy_irq(line.line as u8);
        serial::write_str("Legacy IRQ unmasked: interrupt-line=");
        serial::write_str(line.id);
        serial::write_str(" line=");
        serial::write_u64_dec(line.line);
        serial::write_str("\n");
        irq_index += 1;
    }
}

pub(super) fn validate_boot_config_installable(
    config: &BootRuntimeConfig,
) -> Result<(), InitError> {
    validate_counted_config_entries(&config.processes, config.process_count)?;
    validate_counted_config_entries(&config.endpoints, config.endpoint_count)?;
    validate_counted_config_entries(&config.store_objects, config.store_object_count)?;
    validate_counted_config_entries(&config.state_volumes, config.state_volume_count)?;
    validate_counted_config_entries(&config.network_ports, config.network_port_count)?;
    validate_counted_config_entries(&config.io_ports, config.io_port_count)?;
    validate_counted_config_entries(&config.mmio_regions, config.mmio_region_count)?;
    validate_counted_config_entries(&config.framebuffers, config.framebuffer_count)?;
    validate_counted_config_entries(&config.interrupt_lines, config.interrupt_line_count)?;
    validate_counted_config_entries(&config.dma_regions, config.dma_region_count)?;
    validate_counted_config_entries(&config.pci_devices, config.pci_device_count)?;
    validate_counted_config_entries(&config.virtio_devices, config.virtio_device_count)?;
    validate_counted_config_entries(&config.namespaces, config.namespace_count)?;
    validate_counted_config_entries(&config.vfs_roots, config.vfs_root_count)?;
    validate_counted_config_entries(&config.graph_nodes, config.graph_node_count)?;
    validate_counted_config_entries(&config.graph_edges, config.graph_edge_count)?;
    validate_counted_config_entries(&config.grants, config.grant_count)?;
    validate_counted_config_entries(&config.policy_capabilities, config.policy_capability_count)?;
    validate_counted_config_entries(&config.policy_requirements, config.policy_requirement_count)?;
    validate_counted_config_entries(&config.policy_provides, config.policy_provide_count)?;
    validate_counted_config_entries(&config.policy_mounts, config.policy_mount_count)?;
    validate_counted_config_entries(&config.policy_state_paths, config.policy_state_path_count)?;
    validate_counted_config_entries(&config.policy_bootstraps, config.policy_bootstrap_count)?;

    if config.endpoint_count == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let log_endpoint = config.endpoints[0].ok_or(InitError::InvalidBootManifest)?;
    if log_endpoint.name != LOG_ENDPOINT_NAME {
        return Err(InitError::InvalidBootManifest);
    }
    let mut endpoint_index = 1;
    while endpoint_index < config.endpoint_count {
        let endpoint = config.endpoints[endpoint_index].ok_or(InitError::InvalidBootManifest)?;
        if endpoint.name == LOG_ENDPOINT_NAME {
            return Err(InitError::InvalidBootManifest);
        }
        endpoint_index += 1;
    }
    let initial_index = initial_process_index(config)?;
    validate_boot_config_state_volumes(config)?;

    let object_count = boot_config_object_count(config).ok_or(InitError::ObjectTableFull)?;
    if object_count > MAX_OBJECTS {
        serial::write_str("Krust boot config rejected: object budget exceeded objects=");
        serial::write_u64_dec(object_count as u64);
        serial::write_str(" max=");
        serial::write_u64_dec(MAX_OBJECTS as u64);
        serial::write_str("\n");
        return Err(InitError::ObjectTableFull);
    }
    validate_boot_config_hardware_authority(config)?;
    validate_boot_config_graph_store(config)?;
    validate_boot_config_policy(config)?;

    let mut namespace_index = 0;
    while namespace_index < config.namespace_count {
        let namespace = config.namespaces[namespace_index].ok_or(InitError::InvalidBootManifest)?;
        if namespace.entry_count > MAX_NAMESPACE_ENTRIES {
            return Err(InitError::InvalidBootManifest);
        }
        validate_counted_config_entries(&namespace.entries, namespace.entry_count)?;
        let mut entry_index = 0;
        while entry_index < namespace.entry_count {
            let entry = namespace.entries[entry_index].ok_or(InitError::InvalidBootManifest)?;
            if !namespace_entry_object_kind_allowed(entry.object_kind)
                || !boot_object_config_ref_valid(config, entry.object_kind, entry.object_index)
            {
                return Err(InitError::InvalidBootManifest);
            }
            entry_index += 1;
        }
        namespace_index += 1;
    }

    let mut vfs_root_index = 0;
    while vfs_root_index < config.vfs_root_count {
        let root = config.vfs_roots[vfs_root_index].ok_or(InitError::InvalidBootManifest)?;
        if !valid_vfs_root_path(root.root_path.as_bytes()) {
            return Err(InitError::InvalidBootManifest);
        }
        vfs_root_index += 1;
    }

    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index >= config.process_count {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(slot) = usize::try_from(grant.cap_slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= MAX_CAPS
            || !boot_object_config_ref_valid(config, grant.object_kind, grant.object_index)
        {
            return Err(InitError::InvalidBootManifest);
        }
        if grant.process_index == initial_index
            && initial_process_reserved_cap_slot(config, grant.cap_slot)
        {
            return Err(InitError::InvalidBootManifest);
        }

        let mut previous = 0;
        while previous < grant_index {
            let previous_grant = config.grants[previous].ok_or(InitError::InvalidBootManifest)?;
            if previous_grant.process_index == grant.process_index
                && previous_grant.cap_slot == grant.cap_slot
            {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        grant_index += 1;
    }

    Ok(())
}

fn initial_process_reserved_cap_slot(config: &BootRuntimeConfig, slot: u64) -> bool {
    slot == 2 || slot == INIT_TIMER_CAP_SLOT || (slot == 0 && config.manifest_module.is_some())
}

fn validate_boot_config_graph_store(config: &BootRuntimeConfig) -> Result<(), InitError> {
    if config.graph_node_count == 0
        || config.graph_store_hash[0] == 0
        || config.graph_store_source.is_empty()
    {
        return Err(InitError::InvalidBootManifest);
    }
    let mut generation_nodes = 0;
    let mut index = 0;
    while index < config.graph_node_count {
        let node = config.graph_nodes[index].ok_or(InitError::InvalidBootManifest)?;
        if node.kind == 0 || node.id.is_empty() {
            return Err(InitError::InvalidBootManifest);
        }
        if node.kind == GRAPH_NODE_GENERATION {
            generation_nodes += 1;
            if node.id != config.generation_id {
                return Err(InitError::InvalidBootManifest);
            }
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.graph_nodes[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == node.id {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }
    if generation_nodes != 1 {
        return Err(InitError::InvalidBootManifest);
    }

    index = 0;
    while index < config.graph_edge_count {
        let edge = config.graph_edges[index].ok_or(InitError::InvalidBootManifest)?;
        if edge.kind == 0
            || edge.id.is_empty()
            || edge.from_index >= config.graph_node_count
            || edge.to_index >= config.graph_node_count
            || (edge.kind == GRAPH_EDGE_CAPABILITY && edge.rights == 0)
        {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.graph_edges[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == edge.id {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_graph_has_node(config, GRAPH_NODE_SERVICE, process.graph_node) {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    Ok(())
}

fn boot_graph_has_node(config: &BootRuntimeConfig, kind: u16, id: &str) -> bool {
    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index]
            && node.kind == kind
            && node.id == id
        {
            return true;
        }
        index += 1;
    }
    false
}

fn validate_boot_config_policy(config: &BootRuntimeConfig) -> Result<(), InitError> {
    if config.policy_version != BOOT_POLICY_VERSION || config.policy_hash[0] == 0 {
        log_policy_denial(
            config,
            "<boot>",
            "<policy>",
            "policy-version",
            "unknown-or-empty",
        );
        return Err(InitError::InvalidBootManifest);
    }

    let mut index = 0;
    while index < config.policy_capability_count {
        let capability = config.policy_capabilities[index].ok_or(InitError::InvalidBootManifest)?;
        if capability.id.is_empty()
            || capability.provider.is_empty()
            || capability.rights == 0
            || !known_policy_rights(capability.rights)
            || !boot_policy_object_config_ref_valid(
                config,
                capability.object_kind,
                capability.object_index,
            )
        {
            log_policy_denial(
                config,
                capability.provider,
                capability.id,
                "capability-fact",
                "invalid-capability",
            );
            return Err(InitError::InvalidBootManifest);
        }
        let mut prior = 0;
        while prior < index {
            let existing =
                config.policy_capabilities[prior].ok_or(InitError::InvalidBootManifest)?;
            if existing.id == capability.id {
                log_policy_denial(
                    config,
                    capability.provider,
                    capability.id,
                    "capability-fact",
                    "duplicate-capability",
                );
                return Err(InitError::InvalidBootManifest);
            }
            prior += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_requirement_count {
        let requirement =
            config.policy_requirements[index].ok_or(InitError::InvalidBootManifest)?;
        let Some(capability) = boot_policy_capability_by_id(config, requirement.capability) else {
            log_policy_denial(
                config,
                requirement.service,
                requirement.capability,
                "requirement-fact",
                "unknown-capability",
            );
            return Err(InitError::InvalidBootManifest);
        };
        if !boot_config_has_service(config, requirement.service)?
            || requirement.rights == 0
            || !known_policy_rights(requirement.rights)
            || requirement.rights & !capability.rights != 0
        {
            log_policy_denial(
                config,
                requirement.service,
                requirement.capability,
                "requirement-fact",
                "excess-or-invalid-rights",
            );
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_provide_count {
        let provide = config.policy_provides[index].ok_or(InitError::InvalidBootManifest)?;
        let Some(capability) = boot_policy_capability_by_id(config, provide.capability) else {
            log_policy_denial(
                config,
                provide.service,
                provide.capability,
                "provide-fact",
                "unknown-capability",
            );
            return Err(InitError::InvalidBootManifest);
        };
        if !boot_config_has_service(config, provide.service)?
            || capability.provider != provide.service
        {
            log_policy_denial(
                config,
                provide.service,
                provide.capability,
                "provide-fact",
                "provider-mismatch",
            );
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_mount_count {
        let mount = config.policy_mounts[index].ok_or(InitError::InvalidBootManifest)?;
        if !valid_boot_policy_mount_fact(config, mount)? {
            let target = if mount.path.is_empty() {
                mount.mount_root
            } else {
                mount.path
            };
            log_policy_denial(config, mount.service, target, "mount-fact", "invalid-mount");
            return Err(InitError::InvalidBootManifest);
        }
        let mut prior = 0;
        while prior < index {
            let existing = config.policy_mounts[prior].ok_or(InitError::InvalidBootManifest)?;
            if existing.service == mount.service
                && existing.mount_root == mount.mount_root
                && existing.path == mount.path
                && existing.source == mount.source
                && existing.flags == mount.flags
            {
                log_policy_denial(
                    config,
                    mount.service,
                    mount.mount_root,
                    "mount-fact",
                    "duplicate",
                );
                return Err(InitError::InvalidBootManifest);
            }
            prior += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_state_path_count {
        let state_path = config.policy_state_paths[index].ok_or(InitError::InvalidBootManifest)?;
        if !valid_boot_policy_state_path_fact(config, state_path)? {
            log_policy_denial(
                config,
                state_path.service,
                state_path.root,
                "state-path-fact",
                "invalid-state-path",
            );
            return Err(InitError::InvalidBootManifest);
        }
        let mut prior = 0;
        while prior < index {
            let existing =
                config.policy_state_paths[prior].ok_or(InitError::InvalidBootManifest)?;
            if existing.service == state_path.service
                && existing.state == state_path.state
                && existing.root == state_path.root
            {
                log_policy_denial(
                    config,
                    state_path.service,
                    state_path.root,
                    "state-path-fact",
                    "duplicate",
                );
                return Err(InitError::InvalidBootManifest);
            }
            prior += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_bootstrap_count {
        let bootstrap = config.policy_bootstraps[index].ok_or(InitError::InvalidBootManifest)?;
        if !valid_boot_policy_bootstrap_fact(config, bootstrap)? {
            log_policy_denial(
                config,
                bootstrap.service,
                bootstrap.authority,
                bootstrap.rule,
                "invalid-bootstrap",
            );
            return Err(InitError::InvalidBootManifest);
        }
        let mut prior = 0;
        while prior < index {
            let existing = config.policy_bootstraps[prior].ok_or(InitError::InvalidBootManifest)?;
            if existing.service == bootstrap.service
                && existing.authority == bootstrap.authority
                && existing.rule == bootstrap.rule
            {
                log_policy_denial(
                    config,
                    bootstrap.service,
                    bootstrap.authority,
                    bootstrap.rule,
                    "duplicate",
                );
                return Err(InitError::InvalidBootManifest);
            }
            prior += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.grant_count {
        let grant = config.grants[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_grant_authorized_by_policy(config, grant)? {
            let source = config.processes[grant.process_index]
                .map(|process| process.graph_node)
                .unwrap_or("<invalid>");
            let target = boot_config_object_label(config, grant).unwrap_or("<invalid>");
            let rule = if boot_grant_covers_state_volume_path(config, grant)? {
                "state-path"
            } else {
                "grant-authorized"
            };
            log_policy_denial(config, source, target, rule, "no-policy-edge");
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_policy_mount_root_allows(config, process.graph_node, process.mount_root) {
            log_policy_denial(
                config,
                process.graph_node,
                process.mount_root,
                "mount-root",
                "no-policy-edge",
            );
            return Err(InitError::InvalidBootManifest);
        }
        let mut mount_index = 0;
        while mount_index < process.mount_count {
            let mount = process.mounts[mount_index].ok_or(InitError::InvalidBootManifest)?;
            if !boot_policy_mount_allows(config, process.graph_node, process.mount_root, mount) {
                log_policy_denial(
                    config,
                    process.graph_node,
                    mount.path,
                    "declared-mount",
                    "no-policy-edge",
                );
                return Err(InitError::InvalidBootManifest);
            }
            mount_index += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_mount_count {
        let mount = config.policy_mounts[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_policy_mount_matches_process(config, mount)? {
            let target = if mount.path.is_empty() {
                mount.mount_root
            } else {
                mount.path
            };
            log_policy_denial(
                config,
                mount.service,
                target,
                "mount-fact",
                "unused-policy-edge",
            );
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    index = 0;
    while index < config.policy_state_path_count {
        let state_path = config.policy_state_paths[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_policy_state_path_matches_grant(config, state_path)? {
            log_policy_denial(
                config,
                state_path.service,
                state_path.root,
                "state-path-fact",
                "unused-policy-edge",
            );
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    validate_bootstrap_authority_policy(config)?;

    index = 0;
    while index < config.policy_bootstrap_count {
        let bootstrap = config.policy_bootstraps[index].ok_or(InitError::InvalidBootManifest)?;
        if !boot_policy_bootstrap_matches_runtime(config, bootstrap)? {
            log_policy_denial(
                config,
                bootstrap.service,
                bootstrap.authority,
                bootstrap.rule,
                "unused-policy-edge",
            );
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }

    Ok(())
}

fn valid_boot_policy_mount_fact(
    config: &BootRuntimeConfig,
    mount: BootPolicyMountConfig,
) -> Result<bool, InitError> {
    if !boot_config_has_service(config, mount.service)?
        || !valid_vfs_root_path(mount.mount_root.as_bytes())
    {
        return Ok(false);
    }
    if mount.path.is_empty() && mount.source.is_empty() {
        return Ok(mount.flags == 0);
    }
    Ok(!mount.path.is_empty()
        && !mount.source.is_empty()
        && mount.path != "/"
        && valid_vfs_root_path(mount.path.as_bytes())
        && valid_vfs_root_path(mount.source.as_bytes())
        && mount.flags & !known_boot_process_mount_flags() == 0
        && mount.flags & BOOT_PROCESS_MOUNT_BIND != 0)
}

fn boot_policy_mount_root_allows(
    config: &BootRuntimeConfig,
    service: &str,
    mount_root: &str,
) -> bool {
    let mut index = 0;
    while index < config.policy_mount_count {
        if let Some(mount) = config.policy_mounts[index]
            && mount.service == service
            && mount.mount_root == mount_root
            && mount.path.is_empty()
            && mount.source.is_empty()
            && mount.flags == 0
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_mount_allows(
    config: &BootRuntimeConfig,
    service: &str,
    mount_root: &str,
    process_mount: BootProcessMountConfig,
) -> bool {
    let mut index = 0;
    while index < config.policy_mount_count {
        if let Some(mount) = config.policy_mounts[index]
            && mount.service == service
            && mount.mount_root == mount_root
            && mount.path == process_mount.path
            && mount.source == process_mount.source
            && mount.flags == process_mount.flags
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_mount_matches_process(
    config: &BootRuntimeConfig,
    mount: BootPolicyMountConfig,
) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.graph_node == mount.service && process.mount_root == mount.mount_root {
            if mount.path.is_empty() && mount.source.is_empty() && mount.flags == 0 {
                return Ok(true);
            }
            let mut mount_index = 0;
            while mount_index < process.mount_count {
                let process_mount =
                    process.mounts[mount_index].ok_or(InitError::InvalidBootManifest)?;
                if mount.path == process_mount.path
                    && mount.source == process_mount.source
                    && mount.flags == process_mount.flags
                {
                    return Ok(true);
                }
                mount_index += 1;
            }
        }
        index += 1;
    }
    Ok(false)
}

fn valid_boot_policy_state_path_fact(
    config: &BootRuntimeConfig,
    state_path: BootPolicyStatePathConfig,
) -> Result<bool, InitError> {
    let Some(state) = boot_state_volume_by_id(config, state_path.state)? else {
        return Ok(false);
    };
    Ok(boot_config_has_service(config, state_path.service)?
        && !state_path.root.is_empty()
        && valid_vfs_root_path(state_path.root.as_bytes())
        && state_path.rights != 0
        && known_policy_rights(state_path.rights)
        && (state.sharing_policy != "owner-only" || state.owner == state_path.service))
}

fn boot_state_path_policy_allows_grant(
    config: &BootRuntimeConfig,
    service: &str,
    grant: BootGrantConfig,
) -> Result<bool, InitError> {
    if grant.object_kind != BOOT_OBJECT_VFS_ROOT {
        return Ok(true);
    }
    let root = config.vfs_roots[grant.object_index].ok_or(InitError::InvalidBootManifest)?;
    let mut index = 0;
    while index < config.state_volume_count {
        let state = config.state_volumes[index].ok_or(InitError::InvalidBootManifest)?;
        if boot_state_volume_covered_by_root_path(root.root_path, state)? {
            if state.sharing_policy == "owner-only" && state.owner != service {
                return Ok(false);
            }
            if !boot_policy_state_path_allows(
                config,
                service,
                state.id,
                root.root_path,
                grant.rights,
            ) {
                return Ok(false);
            }
        }
        index += 1;
    }
    Ok(true)
}

fn boot_policy_state_path_allows(
    config: &BootRuntimeConfig,
    service: &str,
    state_id: &str,
    root: &str,
    rights: u64,
) -> bool {
    let mut index = 0;
    while index < config.policy_state_path_count {
        if let Some(state_path) = config.policy_state_paths[index]
            && state_path.service == service
            && state_path.state == state_id
            && state_path.root == root
            && rights & !state_path.rights == 0
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_state_path_matches_grant(
    config: &BootRuntimeConfig,
    state_path: BootPolicyStatePathConfig,
) -> Result<bool, InitError> {
    let Some(state) = boot_state_volume_by_id(config, state_path.state)? else {
        return Ok(false);
    };
    let mut matched_rights = 0;
    let mut index = 0;
    while index < config.grant_count {
        let grant = config.grants[index].ok_or(InitError::InvalidBootManifest)?;
        if grant.object_kind == BOOT_OBJECT_VFS_ROOT {
            let process =
                config.processes[grant.process_index].ok_or(InitError::InvalidBootManifest)?;
            if process.graph_node == state_path.service {
                let root =
                    config.vfs_roots[grant.object_index].ok_or(InitError::InvalidBootManifest)?;
                if root.root_path == state_path.root
                    && boot_state_volume_covered_by_root_path(root.root_path, state)?
                {
                    matched_rights |= grant.rights;
                }
            }
        }
        index += 1;
    }
    Ok(matched_rights != 0 && state_path.rights & !matched_rights == 0)
}

fn boot_grant_covers_state_volume_path(
    config: &BootRuntimeConfig,
    grant: BootGrantConfig,
) -> Result<bool, InitError> {
    if grant.object_kind != BOOT_OBJECT_VFS_ROOT {
        return Ok(false);
    }
    let root = config.vfs_roots[grant.object_index].ok_or(InitError::InvalidBootManifest)?;
    let mut index = 0;
    while index < config.state_volume_count {
        let state = config.state_volumes[index].ok_or(InitError::InvalidBootManifest)?;
        if boot_state_volume_covered_by_root_path(root.root_path, state)? {
            return Ok(true);
        }
        index += 1;
    }
    Ok(false)
}

fn boot_state_volume_covered_by_root_path(
    root: &str,
    state: BootStateVolumeConfig,
) -> Result<bool, InitError> {
    if root == "/state" {
        return Ok(true);
    }
    let Some(rest) = root.strip_prefix("/state/") else {
        return Ok(false);
    };
    let Some(component) = rest.split('/').next() else {
        return Err(InitError::InvalidBootManifest);
    };
    Ok(component == state_volume_mount_component(state.id)?)
}

fn boot_state_volume_by_id(
    config: &BootRuntimeConfig,
    state_id: &str,
) -> Result<Option<BootStateVolumeConfig>, InitError> {
    let mut index = 0;
    while index < config.state_volume_count {
        let state = config.state_volumes[index].ok_or(InitError::InvalidBootManifest)?;
        if state.id == state_id {
            return Ok(Some(state));
        }
        index += 1;
    }
    Ok(None)
}

fn valid_boot_policy_bootstrap_fact(
    config: &BootRuntimeConfig,
    bootstrap: BootPolicyBootstrapConfig,
) -> Result<bool, InitError> {
    Ok(boot_config_has_service(config, bootstrap.service)?
        && !bootstrap.authority.is_empty()
        && !bootstrap.rule.is_empty()
        && bootstrap.rights != 0
        && known_bootstrap_rights(bootstrap.rights))
}

fn validate_bootstrap_authority_policy(config: &BootRuntimeConfig) -> Result<(), InitError> {
    let initial =
        config.processes[initial_process_index(config)?].ok_or(InitError::InvalidBootManifest)?;
    if config.manifest_module.is_some() {
        require_bootstrap_authority(
            config,
            initial.graph_node,
            "boot-module:krustboot-manifest",
            "initial-manifest",
            capability::RIGHT_READ,
        )?;
    }
    require_bootstrap_authority(
        config,
        initial.graph_node,
        "process-control",
        "initial-process-control",
        initial_process_control_rights(),
    )?;
    require_bootstrap_authority(
        config,
        initial.graph_node,
        "timer:monotonic-timer",
        "initial-restart-timer",
        capability::RIGHT_CONTROL,
    )?;
    if boot_config_has_process(config, "logd")? {
        require_bootstrap_authority(
            config,
            "svc:logd",
            "secret:logd-token",
            "native-secret",
            capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA,
        )?;
    }
    if config.state_volume_count > 0
        && let Some(service) = boot_process_graph_node_by_name(config, VERTEX_STATE_PROCESS_NAME)?
    {
        require_bootstrap_authority(
            config,
            service,
            "endpoint:state-vfs-request",
            "state-vfs-request",
            capability::RIGHT_RECEIVE,
        )?;
        require_bootstrap_authority(
            config,
            service,
            "endpoint:state-vfs-reply",
            "state-vfs-reply",
            capability::RIGHT_SEND,
        )?;
    }
    if let Some(service) = boot_process_graph_node_by_name(config, BLOCK_DRIVER_PROCESS_NAME)? {
        require_bootstrap_authority(
            config,
            service,
            "endpoint:vertexfs-device-request",
            "vertexfs-device-request",
            capability::RIGHT_RECEIVE,
        )?;
        require_bootstrap_authority(
            config,
            service,
            "endpoint:vertexfs-device-reply",
            "vertexfs-device-reply",
            capability::RIGHT_SEND,
        )?;
        require_bootstrap_authority(
            config,
            service,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_RECEIVE,
        )?;
        require_bootstrap_authority(
            config,
            service,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_SEND,
        )?;
    }
    if boot_config_has_process(config, BLOCK_DRIVER_PROCESS_NAME)?
        && let Some(service) =
            boot_process_graph_node_by_name(config, GENERATION_MANAGER_PROCESS_NAME)?
    {
        require_bootstrap_authority(
            config,
            service,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_SEND,
        )?;
        require_bootstrap_authority(
            config,
            service,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_RECEIVE,
        )?;
    }
    Ok(())
}

fn require_bootstrap_authority(
    config: &BootRuntimeConfig,
    service: &str,
    authority: &str,
    rule: &str,
    rights: u64,
) -> Result<(), InitError> {
    if boot_bootstrap_authority_allows(config, service, authority, rule, rights) {
        return Ok(());
    }
    log_policy_denial(
        config,
        service,
        authority,
        rule,
        "missing-bootstrap-authority",
    );
    Err(InitError::InvalidBootManifest)
}

fn boot_bootstrap_authority_allows(
    config: &BootRuntimeConfig,
    service: &str,
    authority: &str,
    rule: &str,
    rights: u64,
) -> bool {
    let mut index = 0;
    while index < config.policy_bootstrap_count {
        if let Some(bootstrap) = config.policy_bootstraps[index]
            && bootstrap.service == service
            && bootstrap.authority == authority
            && bootstrap.rule == rule
            && rights & !bootstrap.rights == 0
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_bootstrap_matches_runtime(
    config: &BootRuntimeConfig,
    bootstrap: BootPolicyBootstrapConfig,
) -> Result<bool, InitError> {
    if bootstrap.authority == "boot-module:krustboot-manifest" {
        return Ok(bootstrap.rule == "initial-manifest"
            && bootstrap.rights == capability::RIGHT_READ
            && boot_service_is_initial(config, bootstrap.service)?);
    }
    if bootstrap.authority == "process-control" {
        return Ok(bootstrap.rule == "initial-process-control"
            && bootstrap.rights == initial_process_control_rights()
            && boot_service_is_initial(config, bootstrap.service)?);
    }
    if bootstrap.authority == "timer:monotonic-timer" {
        return Ok(bootstrap.rule == "initial-restart-timer"
            && bootstrap.rights == capability::RIGHT_CONTROL
            && boot_service_is_initial(config, bootstrap.service)?);
    }
    if bootstrap.authority == "secret:logd-token" {
        let Some(capability) = boot_policy_capability_by_id(config, "secret:logd-token") else {
            return Ok(false);
        };
        return Ok(bootstrap.rule == "native-secret"
            && bootstrap.rights == (capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA)
            && boot_service_process_name(config, bootstrap.service)? == Some("logd")
            && capability.object_kind == BOOT_OBJECT_SECRET
            && capability.object_index == 0
            && capability.rights & capability::RIGHT_READ != 0
            && boot_policy_requirement_allows(
                config,
                bootstrap.service,
                "secret:logd-token",
                capability::RIGHT_READ,
            ));
    }
    if let Some(endpoint) = bootstrap.authority.strip_prefix("endpoint:") {
        if !boot_config_has_endpoint(config, endpoint)? {
            return Ok(false);
        }
        if boot_policy_bootstrap_matches_internal_endpoint(config, bootstrap, endpoint)? {
            return Ok(true);
        }
        return boot_policy_bootstrap_matches_builtin_grant(config, bootstrap, endpoint);
    }
    Ok(false)
}

fn boot_policy_bootstrap_matches_builtin_grant(
    config: &BootRuntimeConfig,
    bootstrap: BootPolicyBootstrapConfig,
    endpoint: &str,
) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.grant_count {
        let grant = config.grants[index].ok_or(InitError::InvalidBootManifest)?;
        if grant.object_kind == BOOT_OBJECT_ENDPOINT && grant.rights == bootstrap.rights {
            let process =
                config.processes[grant.process_index].ok_or(InitError::InvalidBootManifest)?;
            let grant_endpoint =
                config.endpoints[grant.object_index].ok_or(InitError::InvalidBootManifest)?;
            if process.graph_node == bootstrap.service
                && grant_endpoint.name == endpoint
                && boot_builtin_endpoint_rule_for_grant(process, grant_endpoint.name, grant)
                    == Some(bootstrap.rule)
            {
                return Ok(true);
            }
        }
        index += 1;
    }
    Ok(false)
}

fn boot_policy_bootstrap_matches_internal_endpoint(
    config: &BootRuntimeConfig,
    bootstrap: BootPolicyBootstrapConfig,
    endpoint: &str,
) -> Result<bool, InitError> {
    let Some(process_name) = boot_service_process_name(config, bootstrap.service)? else {
        return Ok(false);
    };
    let expected = match (process_name, endpoint, bootstrap.rule) {
        (VERTEX_STATE_PROCESS_NAME, STATE_VFS_REQUEST_ENDPOINT_NAME, "state-vfs-request") => {
            capability::RIGHT_RECEIVE
        }
        (VERTEX_STATE_PROCESS_NAME, STATE_VFS_REPLY_ENDPOINT_NAME, "state-vfs-reply") => {
            capability::RIGHT_SEND
        }
        (
            BLOCK_DRIVER_PROCESS_NAME,
            VERTEXFS_DEVICE_REQUEST_ENDPOINT_NAME,
            "vertexfs-device-request",
        ) => capability::RIGHT_RECEIVE,
        (
            BLOCK_DRIVER_PROCESS_NAME,
            VERTEXFS_DEVICE_REPLY_ENDPOINT_NAME,
            "vertexfs-device-reply",
        ) => capability::RIGHT_SEND,
        (
            BLOCK_DRIVER_PROCESS_NAME,
            GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME,
            "generation-metadata-block-request",
        ) => capability::RIGHT_RECEIVE,
        (
            BLOCK_DRIVER_PROCESS_NAME,
            GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME,
            "generation-metadata-block-reply",
        ) => capability::RIGHT_SEND,
        (
            GENERATION_MANAGER_PROCESS_NAME,
            GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME,
            "generation-metadata-block-request",
        ) => capability::RIGHT_SEND,
        (
            GENERATION_MANAGER_PROCESS_NAME,
            GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME,
            "generation-metadata-block-reply",
        ) => capability::RIGHT_RECEIVE,
        _ => return Ok(false),
    };
    Ok(bootstrap.rights == expected)
}

fn boot_policy_bootstrap_allows_endpoint(
    config: &BootRuntimeConfig,
    service: &str,
    endpoint: &str,
    rule: &str,
    rights: u64,
) -> bool {
    let mut index = 0;
    while index < config.policy_bootstrap_count {
        if let Some(bootstrap) = config.policy_bootstraps[index]
            && bootstrap.service == service
            && bootstrap.rule == rule
            && rights & !bootstrap.rights == 0
            && boot_authority_matches_endpoint(bootstrap.authority, endpoint)
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_authority_matches_endpoint(authority: &str, endpoint: &str) -> bool {
    authority.strip_prefix("endpoint:") == Some(endpoint)
}

fn boot_builtin_endpoint_rule_for_grant(
    process: BootProcessConfig,
    endpoint_name: &str,
    grant: BootGrantConfig,
) -> Option<&'static str> {
    if endpoint_name == LOG_ENDPOINT_NAME && grant.rights == capability::RIGHT_SEND {
        return Some("serial-log");
    }
    if endpoint_name == "readiness" {
        if process.initial && grant.rights == capability::RIGHT_RECEIVE {
            return Some("readiness-receive");
        }
        if !process.initial && grant.rights == capability::RIGHT_SEND {
            return Some("readiness-send");
        }
    }
    if process.initial && grant.rights == capability::RIGHT_SEND {
        return Some("init-endpoint-delegation");
    }
    None
}

fn boot_service_is_initial(config: &BootRuntimeConfig, service: &str) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.graph_node == service {
            return Ok(process.initial);
        }
        index += 1;
    }
    Ok(false)
}

fn boot_service_process_name(
    config: &BootRuntimeConfig,
    service: &str,
) -> Result<Option<&'static str>, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.graph_node == service {
            return Ok(Some(process.name));
        }
        index += 1;
    }
    Ok(None)
}

fn boot_process_graph_node_by_name(
    config: &BootRuntimeConfig,
    name: &str,
) -> Result<Option<&'static str>, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.name == name {
            return Ok(Some(process.graph_node));
        }
        index += 1;
    }
    Ok(None)
}

fn boot_config_has_endpoint(config: &BootRuntimeConfig, endpoint: &str) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.endpoint_count {
        let candidate = config.endpoints[index].ok_or(InitError::InvalidBootManifest)?;
        if candidate.name == endpoint {
            return Ok(true);
        }
        index += 1;
    }
    match endpoint {
        STATE_VFS_REQUEST_ENDPOINT_NAME | STATE_VFS_REPLY_ENDPOINT_NAME => {
            Ok(config.state_volume_count > 0)
        }
        VERTEXFS_DEVICE_REQUEST_ENDPOINT_NAME | VERTEXFS_DEVICE_REPLY_ENDPOINT_NAME => Ok(true),
        GENERATION_METADATA_BLOCK_REQUEST_ENDPOINT_NAME
        | GENERATION_METADATA_BLOCK_REPLY_ENDPOINT_NAME => {
            boot_config_has_process(config, BLOCK_DRIVER_PROCESS_NAME)
        }
        _ => Ok(false),
    }
}

fn initial_process_control_rights() -> u64 {
    capability::RIGHT_CONTROL
        | capability::RIGHT_ALLOCATE
        | capability::RIGHT_DELEGATE
        | capability::RIGHT_REVOKE
        | capability::RIGHT_INSPECT
        | capability::RIGHT_CREATE
        | capability::RIGHT_START
        | capability::RIGHT_KILL
        | capability::RIGHT_WAIT
}

fn boot_grant_authorized_by_policy(
    config: &BootRuntimeConfig,
    grant: BootGrantConfig,
) -> Result<bool, InitError> {
    let process = config.processes[grant.process_index].ok_or(InitError::InvalidBootManifest)?;
    if boot_builtin_grant_authorized(config, process, grant)? {
        return Ok(true);
    }

    let mut index = 0;
    while index < config.policy_capability_count {
        let capability = config.policy_capabilities[index].ok_or(InitError::InvalidBootManifest)?;
        if capability.object_kind == grant.object_kind
            && capability.object_index == grant.object_index
            && grant.rights & !capability.rights == 0
        {
            if grant.object_kind == BOOT_OBJECT_ENDPOINT
                && grant.rights == capability::RIGHT_RECEIVE
            {
                if boot_policy_provides(config, process.graph_node, capability.id)
                    && capability.provider == process.graph_node
                {
                    return Ok(true);
                }
            } else if boot_policy_requirement_allows(
                config,
                process.graph_node,
                capability.id,
                grant.rights,
            ) {
                return boot_state_path_policy_allows_grant(config, process.graph_node, grant);
            }
        }
        index += 1;
    }
    Ok(false)
}

fn boot_builtin_grant_authorized(
    config: &BootRuntimeConfig,
    process: BootProcessConfig,
    grant: BootGrantConfig,
) -> Result<bool, InitError> {
    if grant.object_kind != BOOT_OBJECT_ENDPOINT {
        return Ok(false);
    }
    let endpoint = config.endpoints[grant.object_index].ok_or(InitError::InvalidBootManifest)?;
    let Some(rule) = boot_builtin_endpoint_rule_for_grant(process, endpoint.name, grant) else {
        return Ok(false);
    };
    Ok(boot_policy_bootstrap_allows_endpoint(
        config,
        process.graph_node,
        endpoint.name,
        rule,
        grant.rights,
    ))
}

fn boot_policy_requirement_allows(
    config: &BootRuntimeConfig,
    service: &str,
    capability: &str,
    rights: u64,
) -> bool {
    let mut index = 0;
    while index < config.policy_requirement_count {
        if let Some(requirement) = config.policy_requirements[index]
            && requirement.service == service
            && requirement.capability == capability
            && rights & !requirement.rights == 0
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_provides(config: &BootRuntimeConfig, service: &str, capability: &str) -> bool {
    let mut index = 0;
    while index < config.policy_provide_count {
        if let Some(provide) = config.policy_provides[index]
            && provide.service == service
            && provide.capability == capability
        {
            return true;
        }
        index += 1;
    }
    false
}

fn boot_policy_capability_by_id(
    config: &BootRuntimeConfig,
    capability: &str,
) -> Option<BootPolicyCapabilityConfig> {
    let mut index = 0;
    while index < config.policy_capability_count {
        if let Some(candidate) = config.policy_capabilities[index]
            && candidate.id == capability
        {
            return Some(candidate);
        }
        index += 1;
    }
    None
}

fn boot_config_has_service(config: &BootRuntimeConfig, service: &str) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.graph_node == service {
            return Ok(true);
        }
        index += 1;
    }
    Ok(false)
}

fn known_policy_rights(rights: u64) -> bool {
    rights
        & !(capability::RIGHT_SEND
            | capability::RIGHT_RECEIVE
            | capability::RIGHT_READ
            | capability::RIGHT_WRITE
            | capability::RIGHT_SNAPSHOT
            | capability::RIGHT_RESTORE
            | capability::RIGHT_CONTROL
            | capability::RIGHT_BIND
            | capability::RIGHT_LISTEN
            | capability::RIGHT_MAP
            | capability::RIGHT_RESOLVE
            | capability::RIGHT_CREATE
            | capability::RIGHT_UNLINK
            | capability::RIGHT_RENAME
            | capability::RIGHT_MOUNT)
        == 0
}

fn known_bootstrap_rights(rights: u64) -> bool {
    rights
        & !(capability::RIGHT_SEND
            | capability::RIGHT_RECEIVE
            | capability::RIGHT_READ
            | capability::RIGHT_WRITE
            | capability::RIGHT_SNAPSHOT
            | capability::RIGHT_RESTORE
            | capability::RIGHT_CONTROL
            | capability::RIGHT_BIND
            | capability::RIGHT_LISTEN
            | capability::RIGHT_MAP
            | capability::RIGHT_RESOLVE
            | capability::RIGHT_CREATE
            | capability::RIGHT_UNLINK
            | capability::RIGHT_RENAME
            | capability::RIGHT_MOUNT
            | capability::RIGHT_ALLOCATE
            | capability::RIGHT_DELEGATE
            | capability::RIGHT_REVOKE
            | capability::RIGHT_INSPECT
            | capability::RIGHT_START
            | capability::RIGHT_KILL
            | capability::RIGHT_WAIT
            | capability::RIGHT_INSPECT_METADATA)
        == 0
}

fn boot_policy_object_config_ref_valid(
    config: &BootRuntimeConfig,
    object_kind: u16,
    object_index: usize,
) -> bool {
    match object_kind {
        BOOT_OBJECT_SECRET => object_index == 0,
        _ => boot_object_config_ref_valid(config, object_kind, object_index),
    }
}

fn boot_config_object_label(
    config: &BootRuntimeConfig,
    grant: BootGrantConfig,
) -> Option<&'static str> {
    match grant.object_kind {
        BOOT_OBJECT_ENDPOINT => config.endpoints[grant.object_index].map(|object| object.name),
        BOOT_OBJECT_STORE => config.store_objects[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_TIMER => Some("monotonic-timer"),
        BOOT_OBJECT_NETWORK_PORT => {
            config.network_ports[grant.object_index].map(|object| object.id)
        }
        BOOT_OBJECT_IO_PORT_RANGE => config.io_ports[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_MMIO_REGION => config.mmio_regions[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_FRAMEBUFFER => config.framebuffers[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_INTERRUPT_LINE => {
            config.interrupt_lines[grant.object_index].map(|object| object.id)
        }
        BOOT_OBJECT_DMA_REGION => config.dma_regions[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_PCI_DEVICE => config.pci_devices[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_VIRTIO_DEVICE => {
            config.virtio_devices[grant.object_index].map(|object| object.id)
        }
        BOOT_OBJECT_NAMESPACE => config.namespaces[grant.object_index].map(|object| object.id),
        BOOT_OBJECT_VFS_ROOT => config.vfs_roots[grant.object_index].map(|object| object.id),
        _ => None,
    }
}

fn log_policy_denial(
    config: &BootRuntimeConfig,
    source: &str,
    target: &str,
    rule: &str,
    reason: &str,
) {
    record_policy_denial(
        config.generation_id,
        &config.policy_hash,
        source,
        target,
        rule,
        reason,
    );
    serial::write_str("native policy validation rejected: source=");
    serial::write_str(source);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" rule=");
    serial::write_str(rule);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str("\n");
}

fn boot_config_has_process(config: &BootRuntimeConfig, name: &str) -> Result<bool, InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.name == name {
            return Ok(true);
        }
        index += 1;
    }
    Ok(false)
}

fn validate_counted_config_entries<T: Copy, const N: usize>(
    entries: &[Option<T>; N],
    count: usize,
) -> Result<(), InitError> {
    if count > N {
        return Err(InitError::InvalidBootManifest);
    }
    let mut index = 0;
    while index < count {
        if entries[index].is_none() {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(())
}

fn validate_boot_config_state_volumes(config: &BootRuntimeConfig) -> Result<(), InitError> {
    if BUILTIN_VFS_MOUNTS
        .checked_add(config.state_volume_count)
        .is_none_or(|count| count > MAX_VFS_MOUNTS)
    {
        return Err(InitError::ObjectTableFull);
    }
    let mut index = 0;
    while index < config.state_volume_count {
        let state = config.state_volumes[index].ok_or(InitError::InvalidBootManifest)?;
        let component = state_volume_mount_component(state.id)?;
        if state.owner.is_empty()
            || state.schema_version.is_empty()
            || state.storage_class.is_empty()
            || state.migration_policy.is_empty()
            || state.retention_policy.is_empty()
            || state.sharing_policy.is_empty()
            || !matches!(
                state.storage_class,
                "vertexdisk-v1" | "hosted-local-directory"
            )
            || !matches!(
                state.migration_policy,
                "preserve" | "migrate" | "fork" | "discard"
            )
            || !matches!(
                state.retention_policy,
                "retain-while-referenced" | "retain-forever"
            )
            || !matches!(state.sharing_policy, "owner-only" | "explicit")
        {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.state_volumes[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.id == state.id || state_volume_mount_component(prior.id)? == component {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }
    Ok(())
}

fn boot_config_object_count(config: &BootRuntimeConfig) -> Option<usize> {
    let mut count = 0usize;
    count = count.checked_add(config.endpoint_count)?;
    if config.state_volume_count > 0 {
        count = count.checked_add(2)?; // kernel-owned state VFS request/reply endpoints
    }
    count = count.checked_add(config.store_object_count)?;
    count = count.checked_add(config.state_volume_count)?;
    count = count.checked_add(config.network_port_count)?;
    count = count.checked_add(config.io_port_count)?;
    count = count.checked_add(config.mmio_region_count)?;
    count = count.checked_add(config.framebuffer_count)?;
    count = count.checked_add(config.interrupt_line_count)?;
    count = count.checked_add(config.dma_region_count)?;
    count = count.checked_add(config.pci_device_count)?;
    count = count.checked_add(config.virtio_device_count)?;
    count = count.checked_add(config.namespace_count)?;
    count = count.checked_add(config.vfs_root_count)?;
    count = count.checked_add(BUILTIN_VFS_MOUNTS)?;
    count = count.checked_add(config.state_volume_count)?;
    count = count.checked_add(1)?; // monotonic timer
    count = count.checked_add(1)?; // logd secret
    count = count.checked_add(1)?; // process-control
    if config.manifest_module.is_some() {
        count = count.checked_add(1)?;
    }
    Some(count)
}

fn boot_object_config_ref_valid(
    config: &BootRuntimeConfig,
    object_kind: u16,
    object_index: usize,
) -> bool {
    match object_kind {
        BOOT_OBJECT_ENDPOINT => object_index < config.endpoint_count,
        BOOT_OBJECT_STORE => object_index < config.store_object_count,
        BOOT_OBJECT_STATE => false,
        BOOT_OBJECT_TIMER => object_index == 0,
        BOOT_OBJECT_NETWORK_PORT => object_index < config.network_port_count,
        BOOT_OBJECT_IO_PORT_RANGE => object_index < config.io_port_count,
        BOOT_OBJECT_MMIO_REGION => object_index < config.mmio_region_count,
        BOOT_OBJECT_FRAMEBUFFER => object_index < config.framebuffer_count,
        BOOT_OBJECT_INTERRUPT_LINE => object_index < config.interrupt_line_count,
        BOOT_OBJECT_DMA_REGION => object_index < config.dma_region_count,
        BOOT_OBJECT_PCI_DEVICE => object_index < config.pci_device_count,
        BOOT_OBJECT_VIRTIO_DEVICE => object_index < config.virtio_device_count,
        BOOT_OBJECT_NAMESPACE => object_index < config.namespace_count,
        BOOT_OBJECT_VFS_ROOT => object_index < config.vfs_root_count,
        _ => false,
    }
}

fn validate_boot_config_hardware_authority(config: &BootRuntimeConfig) -> Result<(), InitError> {
    let mut index = 0;
    while index < config.io_port_count {
        let range = config.io_ports[index].ok_or(InitError::InvalidBootManifest)?;
        validate_io_boot_range(range.base, range.length)?;
        let mut previous = 0;
        while previous < index {
            let prior = config.io_ports[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(range.base, range.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.mmio_region_count {
        let region = config.mmio_regions[index].ok_or(InitError::InvalidBootManifest)?;
        validate_device_boot_range(region.base, region.length, false)?;
        let mut previous = 0;
        while previous < index {
            let prior = config.mmio_regions[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(region.base, region.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.interrupt_line_count {
        let line = config.interrupt_lines[index].ok_or(InitError::InvalidBootManifest)?;
        if line.line > 15 {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.interrupt_lines[previous].ok_or(InitError::InvalidBootManifest)?;
            if prior.line == line.line {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    index = 0;
    while index < config.dma_region_count {
        let region = config.dma_regions[index].ok_or(InitError::InvalidBootManifest)?;
        validate_device_boot_range(region.base, region.length, true)?;
        if region.length % memory::FRAME_SIZE != 0 || region.length > USER_DEVICE_MAPPING_STRIDE {
            return Err(InitError::InvalidBootManifest);
        }
        let mut previous = 0;
        while previous < index {
            let prior = config.dma_regions[previous].ok_or(InitError::InvalidBootManifest)?;
            if boot_ranges_overlap(region.base, region.length, prior.base, prior.length)? {
                return Err(InitError::InvalidBootManifest);
            }
            previous += 1;
        }
        index += 1;
    }

    Ok(())
}

fn validate_io_boot_range(base: u64, length: u64) -> Result<(), InitError> {
    if length == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let Some(last) = base.checked_add(length - 1) else {
        return Err(InitError::InvalidBootManifest);
    };
    if last > u16::MAX as u64 {
        return Err(InitError::InvalidBootManifest);
    }
    Ok(())
}

fn validate_device_boot_range(
    base: u64,
    length: u64,
    page_aligned_base: bool,
) -> Result<(), InitError> {
    if length == 0 || length > USER_DEVICE_MAPPING_STRIDE {
        return Err(InitError::InvalidBootManifest);
    }
    base.checked_add(length - 1)
        .ok_or(InitError::InvalidBootManifest)?;
    if page_aligned_base && base % memory::FRAME_SIZE != 0 {
        return Err(InitError::InvalidBootManifest);
    }
    Ok(())
}

fn boot_ranges_overlap(
    base: u64,
    length: u64,
    other_base: u64,
    other_length: u64,
) -> Result<bool, InitError> {
    if length == 0 || other_length == 0 {
        return Ok(false);
    }
    let end = base
        .checked_add(length)
        .ok_or(InitError::InvalidBootManifest)?;
    let other_end = other_base
        .checked_add(other_length)
        .ok_or(InitError::InvalidBootManifest)?;
    Ok(base < other_end && other_base < end)
}

fn load_boot_initial_context(process: BootProcessConfig) -> Result<ProcessContext, InitError> {
    load_process_context(process.name, process.image_base, process.image_length)
        .map_err(|_| InitError::InvalidBootManifest)
}

fn snapshot_runtime_reap_targets() -> ([Option<RuntimeReapTarget>; MAX_PROCESSES], usize) {
    let mut targets = [None; MAX_PROCESSES];
    let mut count = 0;
    let runtime = runtime();
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && !process.context_reaped
            && process.context.cr3 != 0
        {
            targets[count] = Some(RuntimeReapTarget {
                pid: process.pid,
                name: process.name,
                cr3: process.context.cr3,
            });
            count += 1;
        }
        index += 1;
    }
    (targets, count)
}

pub(super) fn reap_runtime_contexts(
    targets: &[Option<RuntimeReapTarget>; MAX_PROCESSES],
    count: usize,
) -> Result<(), IpcError> {
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    let allocator = frame_allocator()?;
    let mut index = 0;
    while index < count {
        let target = targets[index].ok_or(IpcError::BadCapability)?;
        let stats = paging::reclaim_user_address_space(hhdm_offset, target.cr3, allocator)
            .map_err(|_| IpcError::BadCapability)?;
        serial::write_str("Krust old runtime address space reaped: proc=");
        serial::write_str(target.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(target.pid.raw());
        serial::write_str(" user_frames=");
        serial::write_u64_dec(stats.user_leaf_frames);
        serial::write_str(" page_tables=");
        serial::write_u64_dec(stats.page_table_frames);
        serial::write_str(" device_mappings=");
        serial::write_u64_dec(stats.device_mappings);
        serial::write_str("\n");
        index += 1;
    }
    Ok(())
}

pub fn install_frame_allocator(allocator: *mut memory::FrameAllocator) {
    unsafe {
        *FRAME_ALLOCATOR.0.get() = Some(allocator);
    }
}

pub(super) fn validate_config_caps_for_process(
    runtime: &RuntimeState,
    config: &BootRuntimeConfig,
    config_process_index: usize,
) -> Result<(), InitError> {
    let process = config.processes[config_process_index].ok_or(InitError::InvalidBootManifest)?;
    let mut occupied_slots = [false; MAX_CAPS];
    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index != config_process_index {
            grant_index += 1;
            continue;
        }

        grant_object_id(runtime, grant)?;
        let Ok(slot) = usize::try_from(grant.cap_slot) else {
            return Err(InitError::CapabilityTableFull);
        };
        if slot >= MAX_CAPS {
            return Err(InitError::CapabilityTableFull);
        }
        if occupied_slots[slot] {
            return Err(InitError::InvalidBootManifest);
        }
        occupied_slots[slot] = true;
        grant_index += 1;
    }

    if process.name == "logd" {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "secret:logd-token",
            "native-secret",
            capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        runtime.secret_id.ok_or(InitError::InvalidBootManifest)?;
        let secret_slot = 6usize;
        if secret_slot >= MAX_CAPS || occupied_slots[secret_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == VERTEX_STATE_PROCESS_NAME && runtime.state_vfs_reply_endpoint.is_some() {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:state-vfs-request",
            "state-vfs-request",
            capability::RIGHT_RECEIVE,
        ) || !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:state-vfs-reply",
            "state-vfs-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(request_slot) = usize::try_from(VERTEX_STATE_VFS_REQUEST_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(VERTEX_STATE_VFS_REPLY_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME && runtime.vertexfs_device_reply_endpoint.is_some()
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:vertexfs-device-request",
            "vertexfs-device-request",
            capability::RIGHT_RECEIVE,
        ) || !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:vertexfs-device-reply",
            "vertexfs-device-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(request_slot) = usize::try_from(BLOCK_DRIVER_VERTEXFS_REQUEST_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(BLOCK_DRIVER_VERTEXFS_REPLY_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && runtime.generation_metadata_block_reply_endpoint.is_some()
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_RECEIVE,
        ) || !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(request_slot) = usize::try_from(BLOCK_DRIVER_GENERATION_METADATA_REQUEST_CAP_SLOT)
        else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(BLOCK_DRIVER_GENERATION_METADATA_REPLY_CAP_SLOT)
        else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == GENERATION_MANAGER_PROCESS_NAME
        && runtime.generation_metadata_block_reply_endpoint.is_some()
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_SEND,
        ) || !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_RECEIVE,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let Ok(request_slot) = usize::try_from(GENERATION_MANAGER_METADATA_REQUEST_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        let Ok(reply_slot) = usize::try_from(GENERATION_MANAGER_METADATA_REPLY_CAP_SLOT) else {
            return Err(InitError::CapabilityTableFull);
        };
        if request_slot >= MAX_CAPS || occupied_slots[request_slot] {
            return Err(InitError::InvalidBootManifest);
        }
        if reply_slot >= MAX_CAPS || occupied_slots[reply_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }

    Ok(())
}

pub(super) fn grant_config_caps_to_process(
    runtime: &mut RuntimeState,
    config: &BootRuntimeConfig,
    config_process_index: usize,
    owner: ProcessId,
) -> Result<(), InitError> {
    let mut grant_index = 0;
    while grant_index < config.grant_count {
        let grant = config.grants[grant_index].ok_or(InitError::InvalidBootManifest)?;
        if grant.process_index != config_process_index {
            grant_index += 1;
            continue;
        }
        let object = grant_object_id(runtime, grant)?;
        let cap = runtime
            .new_capability(object, grant.rights, owner, 0, ProcessId::empty())
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, grant.cap_slot, cap, true)?;
        grant_index += 1;
    }

    let Some(process) = config.processes[config_process_index] else {
        return Err(InitError::InvalidBootManifest);
    };
    if process.name == "logd" {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "secret:logd-token",
            "native-secret",
            capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let secret_id = runtime.secret_id.ok_or(InitError::InvalidBootManifest)?;
        let cap = runtime
            .new_capability(
                secret_id,
                capability::RIGHT_READ | capability::RIGHT_INSPECT_METADATA,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, 6, cap, true)?;
        serial::write_str(
            "Native secret grant: process=logd secret=secret:logd-token rights=read|inspect-metadata\n",
        );
    }
    if process.name == VERTEX_STATE_PROCESS_NAME
        && let Some(request_endpoint) = runtime.state_vfs_request_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:state-vfs-request",
            "state-vfs-request",
            capability::RIGHT_RECEIVE,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, VERTEX_STATE_VFS_REQUEST_CAP_SLOT, cap, true)?;
        serial::write_str(
            "Native VFS state request grant: process=vertex-state endpoint=state-vfs-request rights=receive\n",
        );
    }
    if process.name == VERTEX_STATE_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.state_vfs_reply_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:state-vfs-reply",
            "state-vfs-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(runtime, owner, VERTEX_STATE_VFS_REPLY_CAP_SLOT, cap, true)?;
        serial::write_str(
            "Native VFS state reply grant: process=vertex-state endpoint=state-vfs-reply rights=send\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.vertexfs_device_request_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:vertexfs-device-request",
            "vertexfs-device-request",
            capability::RIGHT_RECEIVE,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_VERTEXFS_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native VertexFS device request grant: process=block-driver endpoint=vertexfs-device-request rights=receive\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.vertexfs_device_reply_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:vertexfs-device-reply",
            "vertexfs-device-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_VERTEXFS_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native VertexFS device reply grant: process=block-driver endpoint=vertexfs-device-reply rights=send\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.generation_metadata_block_request_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_RECEIVE,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_GENERATION_METADATA_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block request grant: process=block-driver endpoint=generation-metadata-block-request rights=receive\n",
        );
    }
    if process.name == BLOCK_DRIVER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.generation_metadata_block_reply_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            BLOCK_DRIVER_GENERATION_METADATA_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block reply grant: process=block-driver endpoint=generation-metadata-block-reply rights=send\n",
        );
    }
    if process.name == GENERATION_MANAGER_PROCESS_NAME
        && let Some(request_endpoint) = runtime.generation_metadata_block_request_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-request",
            "generation-metadata-block-request",
            capability::RIGHT_SEND,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                request_endpoint,
                capability::RIGHT_SEND,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            GENERATION_MANAGER_METADATA_REQUEST_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block request grant: process=gen-manager endpoint=generation-metadata-block-request rights=send\n",
        );
    }
    if process.name == GENERATION_MANAGER_PROCESS_NAME
        && let Some(reply_endpoint) = runtime.generation_metadata_block_reply_endpoint
    {
        if !boot_bootstrap_authority_allows(
            config,
            process.graph_node,
            "endpoint:generation-metadata-block-reply",
            "generation-metadata-block-reply",
            capability::RIGHT_RECEIVE,
        ) {
            return Err(InitError::InvalidBootManifest);
        }
        let cap = runtime
            .new_capability(
                reply_endpoint,
                capability::RIGHT_RECEIVE,
                owner,
                0,
                ProcessId::empty(),
            )
            .map_err(|_| InitError::CapabilityTableFull)?;
        grant_process_cap_by_pid(
            runtime,
            owner,
            GENERATION_MANAGER_METADATA_REPLY_CAP_SLOT,
            cap,
            true,
        )?;
        serial::write_str(
            "Native generation metadata block reply grant: process=gen-manager endpoint=generation-metadata-block-reply rights=receive\n",
        );
    }

    Ok(())
}

fn install_vfs_nodes(runtime: &mut RuntimeState) -> Result<(), InitError> {
    let vertexfs_image = vertexfs_boot_image()?;
    let vertexfs = parse_vertexfs_image(vertexfs_image)?;
    runtime.load_vertexfs_image(vertexfs_image)?;
    let root = runtime.add_vfs_node(
        "/",
        None,
        VfsNodeKind::Directory,
        VfsBacking::None,
        "rootfs",
    )?;
    runtime.add_vfs_mount(
        "mount:rootfs",
        root,
        VfsPath::from_boot_root_path("/")?,
        "rootfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let store_root = runtime.add_vfs_node(
        "store",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "storefs",
    )?;
    runtime.add_vfs_mount(
        "mount:storefs",
        store_root,
        VfsPath::from_boot_root_path("/store")?,
        "storefs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let state_root = runtime.add_vfs_node(
        "state",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "state:volatile",
    )?;
    runtime.add_vfs_mount(
        "mount:state-volatile",
        state_root,
        VfsPath::from_boot_root_path("/state")?,
        "state:volatile",
        VFS_MOUNT_VOLATILE,
        false,
        ProcessId::empty(),
    )?;
    let dev_root = runtime.add_vfs_node(
        "dev",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "devfs",
    )?;
    runtime.add_vfs_mount(
        "mount:devfs",
        dev_root,
        VfsPath::from_boot_root_path("/dev")?,
        "devfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let proc_root = runtime.add_vfs_node(
        "proc",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "procfs",
    )?;
    runtime.add_vfs_mount(
        "mount:procfs",
        proc_root,
        VfsPath::from_boot_root_path("/proc")?,
        "procfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let fs_root = runtime.add_vfs_node(
        "fs",
        Some(root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "vertexfs",
    )?;
    runtime.add_vfs_mount(
        "mount:vertexfs-v1",
        fs_root,
        VfsPath::from_boot_root_path("/fs")?,
        "vertexfs",
        0,
        false,
        ProcessId::empty(),
    )?;
    let vertexfs_readme = runtime.add_vertexfs_file(
        "readme",
        vertexfs.readme.payload,
        Some(vertexfs.readme.inode),
    )?;
    runtime.add_vfs_node(
        "readme",
        Some(fs_root),
        VfsNodeKind::RegularFile,
        VfsBacking::VertexFsFile(vertexfs_readme),
        "vertexfs",
    )?;
    let fs_app = runtime.add_vfs_node(
        "app",
        Some(fs_root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "vertexfs",
    )?;
    let vertexfs_app_a =
        runtime.add_vertexfs_file("a", vertexfs.app_a.payload, Some(vertexfs.app_a.inode))?;
    runtime.add_vfs_node(
        "a",
        Some(fs_app),
        VfsNodeKind::RegularFile,
        VfsBacking::VertexFsFile(vertexfs_app_a),
        "vertexfs",
    )?;
    let vertexfs_format = vertexfs_format_label(vertexfs_image)?;
    let vertexfs_feature = vertexfs_feature_label(vertexfs_image)?;
    serial::write_str("VertexFS ");
    serial::write_str(vertexfs_format);
    serial::write_str(" superblock accepted: generation=");
    serial::write_ascii_bytes(vertexfs.generation);
    serial::write_str(" feature_flags=");
    serial::write_str(vertexfs_feature);
    serial::write_str("\n");
    serial::write_str("VertexFS ");
    serial::write_str(vertexfs_format);
    serial::write_str(" mounted: path=/fs source=vertexfs\n");
    serial::write_str("VertexFS ");
    serial::write_str(vertexfs_format);
    serial::write_str(" directory record verified: path=/fs/app\n");
    serial::write_str("VertexFS ");
    serial::write_str(vertexfs_format);
    serial::write_str(" declared file mounted: path=/fs/app/a\n");
    if vertexfs.journal_replayed {
        serial::write_str("VertexFS ");
        serial::write_str(vertexfs_format);
        serial::write_str(" journal replayed: inode=4 outcome=new\n");
    }
    let state_a = runtime.add_vfs_memory_file("a", b"state:a=0\n")?;
    runtime.add_vfs_node(
        "a",
        Some(state_root),
        VfsNodeKind::RegularFile,
        VfsBacking::MemoryFile(state_a),
        "state:volatile",
    )?;
    let state_sub = runtime.add_vfs_node(
        "sub",
        Some(state_root),
        VfsNodeKind::Directory,
        VfsBacking::None,
        "state:volatile",
    )?;
    let state_sub_a = runtime.add_vfs_memory_file("a", b"state:sub:a=0\n")?;
    runtime.add_vfs_node(
        "a",
        Some(state_sub),
        VfsNodeKind::RegularFile,
        VfsBacking::MemoryFile(state_sub_a),
        "state:volatile",
    )?;
    let service_report_node = runtime.add_vfs_node(
        "service-report",
        Some(state_root),
        VfsNodeKind::RegularFile,
        VfsBacking::FsServiceReport,
        "servicefs",
    )?;
    runtime.add_vfs_mount(
        "mount:servicefs",
        service_report_node,
        VfsPath::from_boot_root_path("/state/service-report")?,
        "servicefs",
        VFS_MOUNT_READ_ONLY,
        false,
        ProcessId::empty(),
    )?;
    serial::write_str(
        "VFS filesystem service file mounted: path=/state/service-report source=servicefs\n",
    );
    serial::write_str(
        "M90 typed VFS mount source: path=/state/service-report source=servicefs source_kind=servicefs source_id=",
    );
    serial::write_u64_dec(service_report_node.raw());
    serial::write_str(" flags=read-only\n");
    let mut state_index = 0;
    while state_index < runtime.state_volume_ids.len() {
        if let Some(object_id) = runtime.state_volume_ids[state_index] {
            let state = runtime
                .objects
                .get_state_volume(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            let node_name = state_volume_vfs_name(state.name)?;
            let root_path = state_volume_vfs_path(state.name)?;
            if runtime.vfs_node_by_path(root_path.as_bytes()).is_some() {
                return Err(InitError::InvalidBootManifest);
            }
            let root_node = runtime.add_vfs_node_with_name(
                node_name,
                Some(state_root),
                VfsNodeKind::Directory,
                VfsBacking::StateVolume(object_id),
                STATEFS_SOURCE,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_VALUE_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeValue(object_id),
                STATEFS_SOURCE,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_CONTROL_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeControl(object_id),
                STATEFS_SOURCE,
            )?;
            runtime.add_vfs_mount(
                state.name,
                root_node,
                root_path,
                STATEFS_SOURCE,
                0,
                false,
                ProcessId::empty(),
            )?;
            serial::write_str("VFS state volume mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str(" source=vertex-state\n");
            serial::write_str("VFS state volume value file mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str("/value source=vertex-state\n");
            serial::write_str("VFS state volume control file mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" path=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str("/control source=vertex-state\n");
            serial::write_str("M94 statefs backend mounted: state=");
            serial::write_str(state.name);
            serial::write_str(" backend=");
            serial::write_str(state.storage_class);
            serial::write_str(" root=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str(" owner=");
            serial::write_str(state.owner);
            serial::write_str(" schema=");
            serial::write_str(state.schema_version);
            serial::write_str(" migration=");
            serial::write_str(state.migration_policy);
            serial::write_str(" retention=");
            serial::write_str(state.retention_policy);
            serial::write_str(" sharing=");
            serial::write_str(state.sharing_policy);
            serial::write_str("\n");
            serial::write_str("M94 statefs authority source: state=");
            serial::write_str(state.name);
            serial::write_str(" source=graph-state-policy root=");
            serial::write_ascii_bytes(root_path.as_bytes());
            serial::write_str("\n");
        }
        state_index += 1;
    }
    runtime.add_vfs_node(
        "inspect",
        Some(proc_root),
        VfsNodeKind::SyntheticNode,
        VfsBacking::Synthetic(VFS_SYNTHETIC_INSPECT_BYTES),
        "procfs",
    )?;
    runtime.add_vfs_node(
        "log-stream",
        Some(proc_root),
        VfsNodeKind::Pipe,
        VfsBacking::Pipe,
        "pipefs",
    )?;

    let mut index = 0;
    while index < runtime.store_object_ids.len() {
        if let Some(object_id) = runtime.store_object_ids[index] {
            let store = runtime
                .objects
                .get_store_object(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            runtime.add_vfs_node(
                store.name,
                Some(store_root),
                VfsNodeKind::RegularFile,
                VfsBacking::StoreObject(object_id),
                "storefs",
            )?;
            serial::write_str("VFS node registered: file=");
            serial::write_str(store.name);
            serial::write_str(" backing=store-object\n");
        }
        index += 1;
    }

    index = 0;
    while index < runtime.virtio_device_ids.len() {
        if let Some(object_id) = runtime.virtio_device_ids[index] {
            let device = runtime
                .objects
                .get_virtio_device(object_id)
                .ok_or(InitError::InvalidBootManifest)?;
            runtime.add_vfs_node(
                device.name,
                Some(dev_root),
                VfsNodeKind::DeviceNode,
                VfsBacking::Device(object_id),
                "devfs",
            )?;
            serial::write_str("VFS node registered: device=");
            serial::write_str(device.name);
            serial::write_str(" backing=virtio-device\n");
        }
        index += 1;
    }
    Ok(())
}

fn vertexfs_boot_image() -> Result<&'static [u8], InitError> {
    let Some(modules) = limine::modules() else {
        serial::write_str("Krust VertexFS image missing: limine modules unavailable\n");
        return Err(InitError::InvalidBootManifest);
    };

    let mut found = None;
    let mut index = 0;
    while index < modules.module_count() {
        if let Some(module) = modules.module(index)
            && (c_string_eq_bytes(module.string, VERTEXFS_MODULE_STRING)
                || c_string_eq_bytes(module.string, VERTEXFS_MODULE_STRING_V1)
                || c_string_eq_bytes(module.string, VERTEXFS_MODULE_STRING_V2))
        {
            if found.is_some() {
                return reject_vertexfs_boot_image("duplicate module");
            }
            found = Some(module);
        }
        index += 1;
    }

    let Some(module) = found else {
        serial::write_str("Krust VertexFS image missing\n");
        return Err(InitError::InvalidBootManifest);
    };
    if module.address.is_null() {
        return reject_vertexfs_boot_image("null module");
    }
    let Ok(size) = usize::try_from(module.size) else {
        return reject_vertexfs_boot_image("size overflow");
    };
    if size != VERTEXFS_IMAGE_BYTES {
        serial::write_str("Krust VertexFS image rejected: size=");
        serial::write_u64_dec(module.size);
        serial::write_str(" expected=");
        serial::write_u64_dec(VERTEXFS_IMAGE_BYTES as u64);
        serial::write_str("\n");
        return Err(InitError::InvalidBootManifest);
    }

    serial::write_str("VertexFS image module accepted: bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");
    Ok(unsafe { core::slice::from_raw_parts(module.address, size) })
}

fn reject_vertexfs_boot_image<T>(reason: &str) -> Result<T, InitError> {
    serial::write_str("Krust VertexFS image rejected: ");
    serial::write_str(reason);
    serial::write_str("\n");
    Err(InitError::InvalidBootManifest)
}

fn c_string_eq_bytes(value: *const u8, expected: &[u8]) -> bool {
    if value.is_null() {
        return false;
    }
    let mut index = 0;
    while index < expected.len() {
        if unsafe { value.add(index).read() } != expected[index] {
            return false;
        }
        index += 1;
    }
    unsafe { value.add(expected.len()).read() == 0 }
}

fn validate_process_mount_roots(
    runtime: &RuntimeState,
    config: &BootRuntimeConfig,
) -> Result<(), InitError> {
    let mut index = 0;
    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        let node = runtime
            .vfs_node_by_path(process.mount_root.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(node.kind, VfsNodeKind::Directory) {
            return Err(InitError::InvalidBootManifest);
        }
        index += 1;
    }
    Ok(())
}

pub(super) fn install_declared_process_mounts(
    runtime: &mut RuntimeState,
    process: BootProcessConfig,
    pid: ProcessId,
    mount_root: VfsPath,
) -> Result<u64, InitError> {
    let mut installed = 0;
    let mut index = 0;
    while index < process.mount_count {
        let mount = process.mounts[index].ok_or(InitError::InvalidBootManifest)?;
        let destination = resolve_vfs_path_under_root(mount_root, mount.path.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let source = resolve_vfs_path_under_root(mount_root, mount.source.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let (parent_path, _) = split_vfs_parent_child(destination.as_bytes())
            .map_err(|_| InitError::InvalidBootManifest)?;
        let parent = runtime
            .vfs_node_by_path(parent_path)
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(parent.kind, VfsNodeKind::Directory)
            || runtime.vfs_node_by_path(destination.as_bytes()).is_some()
            || runtime
                .objects
                .get_vfs_mount_by_exact_path(destination.as_bytes())
                .is_some()
        {
            return Err(InitError::InvalidBootManifest);
        }
        let source_node = runtime
            .vfs_node_by_path(source.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?;
        if !matches!(source_node.kind, VfsNodeKind::Directory) {
            return Err(InitError::InvalidBootManifest);
        }
        let source_mount_flags = runtime
            .objects
            .get_vfs_mount_by_path(source.as_bytes())
            .ok_or(InitError::InvalidBootManifest)?
            .flags;
        let flags = boot_process_mount_flags_to_vfs(mount.flags)?
            | (source_mount_flags & VFS_MOUNT_READ_ONLY);
        runtime.add_vfs_mount(
            "mount:declared-bind",
            source_node.id,
            destination,
            source_node.mount_source,
            flags,
            false,
            pid,
        )?;
        serial::write_str("Krust declared mount snapshot restored: proc=");
        serial::write_str(process.name);
        serial::write_str(" path=");
        serial::write_ascii_bytes(mount.path.as_bytes());
        serial::write_str(" canonical=");
        serial::write_ascii_bytes(destination.as_bytes());
        serial::write_str(" source=");
        serial::write_ascii_bytes(mount.source.as_bytes());
        serial::write_str(" canonical_source=");
        serial::write_ascii_bytes(source.as_bytes());
        serial::write_str(" flags=");
        serial_write_vfs_mount_flags(flags);
        serial::write_str("\n");
        installed += 1;
        index += 1;
    }
    Ok(installed)
}

fn boot_process_mount_flags_to_vfs(flags: u16) -> Result<u64, InitError> {
    if flags & !known_boot_process_mount_flags() != 0 || flags & BOOT_PROCESS_MOUNT_BIND == 0 {
        return Err(InitError::InvalidBootManifest);
    }
    let mut vfs_flags = VFS_MOUNT_BIND;
    if flags & BOOT_PROCESS_MOUNT_READ_ONLY != 0 {
        vfs_flags |= VFS_MOUNT_READ_ONLY;
    }
    Ok(vfs_flags)
}

pub(super) fn grant_object_id(
    runtime: &RuntimeState,
    grant: BootGrantConfig,
) -> Result<KernelObjectId, InitError> {
    match grant.object_kind {
        BOOT_OBJECT_ENDPOINT => {
            runtime.endpoint_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_STORE => {
            runtime.store_object_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_STATE => Err(InitError::InvalidBootManifest),
        BOOT_OBJECT_TIMER if grant.object_index == 0 => {
            runtime.timer_id.ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_NETWORK_PORT => {
            runtime.network_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_IO_PORT_RANGE => {
            runtime.io_port_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_MMIO_REGION => {
            runtime.mmio_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_FRAMEBUFFER => {
            runtime.framebuffer_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_INTERRUPT_LINE => {
            runtime.interrupt_line_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_DMA_REGION => {
            runtime.dma_region_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_PCI_DEVICE => {
            runtime.pci_device_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_VIRTIO_DEVICE => {
            runtime.virtio_device_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_NAMESPACE => {
            runtime.namespace_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        BOOT_OBJECT_VFS_ROOT => {
            runtime.vfs_root_ids[grant.object_index].ok_or(InitError::InvalidBootManifest)
        }
        _ => Err(InitError::InvalidBootManifest),
    }
}

fn namespace_entry_object_id(
    runtime: &RuntimeState,
    entry: BootNamespaceEntryConfig,
) -> Result<KernelObjectId, InitError> {
    if !namespace_entry_object_kind_allowed(entry.object_kind) {
        return Err(InitError::InvalidBootManifest);
    }
    grant_object_id(
        runtime,
        BootGrantConfig {
            process_index: 0,
            cap_slot: 0,
            object_kind: entry.object_kind,
            object_index: entry.object_index,
            rights: entry.rights,
        },
    )
}

fn namespace_entry_object_kind_allowed(object_kind: u16) -> bool {
    matches!(
        object_kind,
        BOOT_OBJECT_ENDPOINT | BOOT_OBJECT_STORE | BOOT_OBJECT_TIMER | BOOT_OBJECT_NETWORK_PORT
    )
}

fn grant_process_cap_by_pid(
    runtime: &mut RuntimeState,
    pid: ProcessId,
    slot: u64,
    cap: Capability,
    persist_for_restart: bool,
) -> Result<(), InitError> {
    let Some(process) = runtime.processes.process_mut(pid) else {
        return Err(InitError::InvalidBootManifest);
    };
    let mut caps = process.caps;
    let mut initial_caps = process.initial_caps;
    caps.grant(slot, cap)?;
    if persist_for_restart {
        initial_caps.grant(slot, cap)?;
    }
    process.caps = caps;
    if persist_for_restart {
        process.initial_caps = initial_caps;
    }
    Ok(())
}

fn initial_process_index(config: &BootRuntimeConfig) -> Result<usize, InitError> {
    let mut found = None;
    let mut index = 0;

    while index < config.process_count {
        let process = config.processes[index].ok_or(InitError::InvalidBootManifest)?;
        if process.initial {
            if found.is_some() {
                return Err(InitError::InvalidBootManifest);
            }
            found = Some(index);
        }
        index += 1;
    }

    found.ok_or(InitError::InvalidBootManifest)
}

pub fn initial_process_context() -> Option<ProcessContext> {
    runtime()
        .processes
        .current_process()
        .map(|process| process.context)
}
