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
        runtime.state_volume_ids[state_index] = Some(runtime.objects.add_state_volume(state.id)?);
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
    validate_counted_config_entries(&config.interrupt_lines, config.interrupt_line_count)?;
    validate_counted_config_entries(&config.dma_regions, config.dma_region_count)?;
    validate_counted_config_entries(&config.pci_devices, config.pci_device_count)?;
    validate_counted_config_entries(&config.virtio_devices, config.virtio_device_count)?;
    validate_counted_config_entries(&config.namespaces, config.namespace_count)?;
    validate_counted_config_entries(&config.vfs_roots, config.vfs_root_count)?;
    validate_counted_config_entries(&config.graph_nodes, config.graph_node_count)?;
    validate_counted_config_entries(&config.graph_edges, config.graph_edge_count)?;
    validate_counted_config_entries(&config.grants, config.grant_count)?;

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
        runtime.secret_id.ok_or(InitError::InvalidBootManifest)?;
        let secret_slot = 6usize;
        if secret_slot >= MAX_CAPS || occupied_slots[secret_slot] {
            return Err(InitError::InvalidBootManifest);
        }
    }
    if process.name == VERTEX_STATE_PROCESS_NAME && runtime.state_vfs_reply_endpoint.is_some() {
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
    serial::write_str("VertexFS v1 superblock accepted: generation=");
    serial::write_ascii_bytes(vertexfs.generation);
    serial::write_str(" feature_flags=metadata-v1\n");
    serial::write_str("VertexFS v1 mounted: path=/fs source=vertexfs\n");
    serial::write_str("VertexFS v1 directory record verified: path=/fs/app\n");
    serial::write_str("VertexFS v1 declared file mounted: path=/fs/app/a\n");
    if vertexfs.journal_replayed {
        serial::write_str("VertexFS v1 journal replayed: inode=4 outcome=new\n");
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
    runtime.add_vfs_node(
        "service-report",
        Some(state_root),
        VfsNodeKind::RegularFile,
        VfsBacking::FsServiceReport,
        "servicefs",
    )?;
    serial::write_str(
        "VFS filesystem service file mounted: path=/state/service-report source=servicefs\n",
    );
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
                state.name,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_VALUE_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeValue(object_id),
                state.name,
            )?;
            runtime.add_vfs_node(
                STATE_VOLUME_CONTROL_FILE_NAME,
                Some(root_node),
                VfsNodeKind::RegularFile,
                VfsBacking::StateVolumeControl(object_id),
                state.name,
            )?;
            runtime.add_vfs_mount(
                state.name,
                root_node,
                root_path,
                state.name,
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
        serial::write_str("Krust VertexFS v1 image missing: limine modules unavailable\n");
        return Err(InitError::InvalidBootManifest);
    };

    let mut found = None;
    let mut index = 0;
    while index < modules.module_count() {
        if let Some(module) = modules.module(index)
            && c_string_eq_bytes(module.string, VERTEXFS_MODULE_STRING)
        {
            if found.is_some() {
                return reject_vertexfs_boot_image("duplicate module");
            }
            found = Some(module);
        }
        index += 1;
    }

    let Some(module) = found else {
        serial::write_str("Krust VertexFS v1 image missing\n");
        return Err(InitError::InvalidBootManifest);
    };
    if module.address.is_null() {
        return reject_vertexfs_boot_image("null module");
    }
    let Ok(size) = usize::try_from(module.size) else {
        return reject_vertexfs_boot_image("size overflow");
    };
    if size != VERTEXFS_IMAGE_BYTES {
        serial::write_str("Krust VertexFS v1 image rejected: size=");
        serial::write_u64_dec(module.size);
        serial::write_str(" expected=");
        serial::write_u64_dec(VERTEXFS_IMAGE_BYTES as u64);
        serial::write_str("\n");
        return Err(InitError::InvalidBootManifest);
    }

    serial::write_str("VertexFS v1 image module accepted: bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");
    Ok(unsafe { core::slice::from_raw_parts(module.address, size) })
}

fn reject_vertexfs_boot_image<T>(reason: &str) -> Result<T, InitError> {
    serial::write_str("Krust VertexFS v1 image rejected: ");
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
