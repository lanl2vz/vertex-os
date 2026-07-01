use super::*;

pub(super) fn build_inspect_report(runtime: &RuntimeState, report: &mut InspectReport) {
    report.push_str("native-runtime-report v=1\n");
    report.push_str("generation=");
    report.push_str(runtime.generation_id);
    report.push_byte(b'\n');
    write_generation_manager_report(report);
    write_graph_store_report(runtime, report);
    write_state_policy_report(runtime, report);
    report.push_str("processes=");
    report.push_u64_dec(runtime.processes.count as u64);
    report.push_byte(b'\n');
    report.push_str("objects=");
    report.push_u64_dec(runtime.objects.live_count() as u64);
    report.push_byte(b'\n');
    report.push_str("vfs_nodes=");
    report.push_u64_dec(runtime.vfs_node_count as u64);
    report.push_str(" file_handles=");
    report.push_u64_dec(runtime_file_handle_count(runtime));
    report.push_byte(b'\n');
    report.push_str("caps=");
    report.push_u64_dec(runtime_cap_count(runtime));
    report.push_byte(b'\n');
    report.push_str("objects_unreachable=");
    report.push_u64_dec(unreachable_object_count(runtime));
    report.push_byte(b'\n');
    write_unreachable_object_report(runtime, report);
    write_vfs_report(runtime, report);
    if let Some(stats) = frame_allocator_stats() {
        report.push_str("frames total=");
        report.push_u64_dec(stats.total_frames);
        report.push_str(" allocated=");
        report.push_u64_dec(stats.allocated_frames);
        report.push_str(" free=");
        report.push_u64_dec(stats.free_frames);
        report.push_str(" reserved=0 reclaimed=");
        report.push_u64_dec(stats.reclaimed_frames);
        report.push_str(" high_water=");
        report.push_u64_dec(stats.high_water_frames);
        report.push_str(" failed_allocations=");
        report.push_u64_dec(stats.failed_allocations);
        report.push_str(" recycled=");
        report.push_u64_dec(stats.recycled_frames as u64);
        report.push_str(" ledger_entries=");
        report.push_u64_dec(stats.ledger_entries as u64);
        report.push_str(" owner_kernel=");
        report.push_u64_dec(stats.kernel_frames);
        report.push_str(" owner_page_table=");
        report.push_u64_dec(stats.page_table_frames);
        report.push_str(" owner_process=");
        report.push_u64_dec(stats.process_memory_frames);
        report.push_str(" owner_dma=");
        report.push_u64_dec(stats.dma_frames);
        report.push_str(" owner_scratch=");
        report.push_u64_dec(stats.scratch_frames);
        report.push_byte(b'\n');
    }

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            report.push_str("process[");
            report.push_u64_dec(index as u64);
            report.push_str("] name=");
            report.push_str(process.name);
            report.push_str(" pid=");
            report.push_u64_dec(process.pid.raw());
            report.push_str(" state=");
            report.push_str(process.state.label());
            report.push_str(" restart_policy=");
            report.push_str(restart_policy_label(
                process_config_for_pid(runtime, process.pid)
                    .map(|process| process.restart_policy)
                    .unwrap_or(0),
            ));
            report.push_str(" mount_root=");
            report.push_bytes(process.mount_root.as_bytes());
            report.push_str(" context_reaped=");
            if process.context_reaped {
                report.push_str("yes");
            } else {
                report.push_str("no");
            }
            report.push_str(" cr3=");
            report.push_u64_dec(process.context.cr3);
            report.push_str(" generation=");
            report.push_str(runtime.generation_id);
            report.push_str(" graph_node=");
            report.push_str(process_graph_node(runtime, process.name));
            report.push_byte(b'\n');

            write_capability_space_report(runtime, report, process, "current", process.caps);
            write_capability_space_report(
                runtime,
                report,
                process,
                "initial",
                process.initial_caps,
            );
        }
        index += 1;
    }

    let mut interrupt_report_index = 0;
    let mut object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::InterruptLine(_)) = runtime.objects.objects[object_index] {
            interrupt_report_index += 1;
        }
        object_index += 1;
    }

    report.push_str("interrupt_lines=");
    report.push_u64_dec(interrupt_report_index);
    report.push_byte(b'\n');
    interrupt_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::InterruptLine(line)) = runtime.objects.objects[object_index] {
            report.push_str("interrupt-line[");
            report.push_u64_dec(interrupt_report_index);
            report.push_str("] name=");
            report.push_str(line.name);
            report.push_str(" line=");
            report.push_u64_dec(line.line);
            report.push_str(" owner=");
            report.push_str(interrupt_owner_name(runtime, line.id));
            report.push_str(" pending=");
            report.push_u64_dec(line.pending_count);
            report.push_str(" delivered=");
            report.push_u64_dec(line.delivered_count);
            report.push_str(" waiters=");
            report.push_u64_dec(interrupt_waiter_count(runtime, line.id));
            report.push_str(" spurious=");
            report.push_u64_dec(line.spurious_count);
            report.push_byte(b'\n');
            interrupt_report_index += 1;
        }
        object_index += 1;
    }

    let mut dma_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::DmaRegion(_)) = runtime.objects.objects[object_index] {
            dma_report_index += 1;
        }
        object_index += 1;
    }

    report.push_str("dma_regions=");
    report.push_u64_dec(dma_report_index);
    report.push_byte(b'\n');
    dma_report_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::DmaRegion(region)) = runtime.objects.objects[object_index] {
            report.push_str("dma-region[");
            report.push_u64_dec(dma_report_index);
            report.push_str("] name=");
            report.push_str(region.name);
            report.push_str(" base=");
            report.push_u64_dec(region.base);
            report.push_str(" length=");
            report.push_u64_dec(region.length);
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, region.mapped_by));
            report.push_str(" mapped=");
            report.push_str(if region.mapped_by == ProcessId::empty() {
                "no"
            } else {
                "yes"
            });
            report.push_str(" map_count=");
            report.push_u64_dec(region.map_count);
            report.push_str(" release_count=");
            report.push_u64_dec(region.release_count);
            report.push_byte(b'\n');
            dma_report_index += 1;
        }
        object_index += 1;
    }

    write_virtio_runtime_report(runtime, report);

    report.push_str("service_lifecycle_events=");
    report.push_u64_dec(runtime.service_lifecycle_event_count as u64);
    report.push_byte(b'\n');
    let mut event_index = 0;
    while event_index < runtime.service_lifecycle_event_count {
        if let Some(event) = runtime.service_lifecycle_events[event_index] {
            report.push_str("service-lifecycle[");
            report.push_u64_dec(event_index as u64);
            report.push_str("] generation=");
            report.push_str(runtime.generation_id);
            report.push_str(" service=");
            report.push_str(event.service);
            report.push_str(" state=");
            report.push_str(event.state.label());
            if event.has_status {
                report.push_str(" status=");
                report.push_u64_dec(event.status);
            }
            report.push_byte(b'\n');
        }
        event_index += 1;
    }
}

fn write_generation_manager_report(report: &mut InspectReport) {
    let manager = boot_manager_state();
    report.push_str("generation-manager v=1 selected=");
    push_generation_field(report, manager.selected_generation);
    report.push_str(" previous=");
    push_generation_field(report, manager.previous_generation);
    report.push_str(" known_good=");
    push_generation_field(report, manager.known_good_generation);
    report.push_str(" last_failed=");
    push_generation_field(report, manager.last_failed_generation);
    report.push_str(" transaction=");
    report.push_str(manager.last_transaction_state);
    report.push_str(" target=");
    push_generation_field(report, manager.last_transaction_target);
    report.push_str(" tx_counter=");
    report.push_u64_dec(manager.transaction_counter);
    report.push_str(" failure_reason=");
    push_generation_field(report, manager.last_failure_reason);
    report.push_str(" failure_service=");
    push_generation_field(report, manager.last_failure_service);
    report.push_str(" failure_dependency=");
    push_generation_field(report, manager.last_failure_dependency);
    report.push_str(" failure_policy=");
    push_generation_field(report, manager.last_failure_policy);
    report.push_byte(b'\n');
}

fn push_generation_field(report: &mut InspectReport, value: &str) {
    if value.is_empty() {
        report.push_str("<none>");
    } else {
        report.push_str(value);
    }
}

fn write_graph_store_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let Some(config) = runtime.active_config else {
        report.push_str("graph-store v=1 status=unavailable\n");
        return;
    };
    report.push_str("graph-store v=1 generation=");
    report.push_str(config.generation_id);
    report.push_str(" hash=");
    report.push_bytes(&config.graph_store_hash);
    report.push_str(" checksum=");
    report.push_u64_dec(config.graph_store_checksum as u64);
    report.push_str(" nodes=");
    report.push_u64_dec(config.graph_node_count as u64);
    report.push_str(" edges=");
    report.push_u64_dec(config.graph_edge_count as u64);
    report.push_str(" source=");
    report.push_str(config.graph_store_source);
    report.push_byte(b'\n');

    report.push_str("graph-store-object-counts generation=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_GENERATION));
    report.push_str(" services=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_SERVICE));
    report.push_str(" endpoints=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_ENDPOINT));
    report.push_str(" store_objects=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_STORE_OBJECT));
    report.push_str(" configs=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_CONFIG));
    report.push_str(" state=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_STATE_VOLUME));
    report.push_str(" devices=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_DEVICE));
    report.push_str(" namespaces=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_NAMESPACE));
    report.push_str(" vfs_roots=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_VFS_ROOT));
    report.push_str(" secrets=");
    report.push_u64_dec(graph_node_kind_count(config, GRAPH_NODE_SECRET));
    report.push_byte(b'\n');

    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index] {
            report.push_str("graph-node[");
            report.push_u64_dec(index as u64);
            report.push_str("] kind=");
            report.push_str(graph_node_kind_label(node.kind));
            report.push_str(" id=");
            report.push_str(node.id);
            report.push_str(" object_kind=");
            report.push_str(boot_object_kind_label(node.object_kind));
            report.push_str(" label=");
            report.push_str(node.label);
            report.push_byte(b'\n');
        }
        index += 1;
    }

    index = 0;
    while index < config.graph_edge_count {
        if let Some(edge) = config.graph_edges[index] {
            report.push_str("graph-edge[");
            report.push_u64_dec(index as u64);
            report.push_str("] kind=");
            report.push_str(graph_edge_kind_label(edge.kind));
            report.push_str(" id=");
            report.push_str(edge.id);
            report.push_str(" from=");
            report.push_str(graph_node_id(config, edge.from_index));
            report.push_str(" to=");
            report.push_str(graph_node_id(config, edge.to_index));
            report.push_str(" rights=");
            write_rights_report(report, edge.rights);
            report.push_byte(b'\n');
        }
        index += 1;
    }
}

fn graph_node_kind_count(config: &BootRuntimeConfig, kind: u16) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < config.graph_node_count {
        if let Some(node) = config.graph_nodes[index]
            && node.kind == kind
        {
            count += 1;
        }
        index += 1;
    }
    count
}

fn graph_node_id(config: &BootRuntimeConfig, index: usize) -> &'static str {
    if index < config.graph_node_count
        && let Some(node) = config.graph_nodes[index]
    {
        return node.id;
    }
    "<invalid>"
}

fn graph_node_kind_label(kind: u16) -> &'static str {
    match kind {
        GRAPH_NODE_GENERATION => "generation",
        GRAPH_NODE_SERVICE => "service",
        GRAPH_NODE_ENDPOINT => "endpoint",
        GRAPH_NODE_STORE_OBJECT => "store-object",
        GRAPH_NODE_CONFIG => "config",
        GRAPH_NODE_STATE_VOLUME => "state-volume",
        GRAPH_NODE_DEVICE => "device",
        GRAPH_NODE_NAMESPACE => "namespace",
        GRAPH_NODE_VFS_ROOT => "vfs-root",
        GRAPH_NODE_TIMER => "timer",
        GRAPH_NODE_SECRET => "secret",
        _ => "unknown",
    }
}

fn graph_edge_kind_label(kind: u16) -> &'static str {
    match kind {
        GRAPH_EDGE_ACTIVATION => "activation",
        GRAPH_EDGE_CAPABILITY => "capability",
        GRAPH_EDGE_MOUNT => "mount",
        _ => "unknown",
    }
}

fn boot_object_kind_label(kind: u16) -> &'static str {
    match kind {
        0 => "none",
        BOOT_OBJECT_ENDPOINT => "endpoint",
        BOOT_OBJECT_STORE => "store",
        BOOT_OBJECT_STATE => "state",
        BOOT_OBJECT_TIMER => "timer",
        BOOT_OBJECT_NETWORK_PORT => "network-port",
        BOOT_OBJECT_IO_PORT_RANGE => "io-port",
        BOOT_OBJECT_MMIO_REGION => "mmio-region",
        BOOT_OBJECT_FRAMEBUFFER => "framebuffer",
        BOOT_OBJECT_INTERRUPT_LINE => "interrupt-line",
        BOOT_OBJECT_DMA_REGION => "dma-region",
        BOOT_OBJECT_PCI_DEVICE => "pci-device",
        BOOT_OBJECT_VIRTIO_DEVICE => "virtio-device",
        BOOT_OBJECT_NAMESPACE => "namespace",
        BOOT_OBJECT_VFS_ROOT => "vfs-root",
        _ => "unknown",
    }
}

fn write_virtio_runtime_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let rng = unsafe { *VIRTIO_RNG_STATE.0.get() };
    let net = unsafe { *VIRTIO_NET_STATE.0.get() };

    report.push_str("virtio_runtime_devices=2\n");
    report.push_str("virtio-runtime[0] device=");
    report.push_str(VIRTIO_RNG_DEVICE_ID);
    report.push_str(" initialized=");
    write_yes_no(report, rng.initialized);
    report.push_str(" owner=");
    report.push_str(process_name_by_pid(runtime, rng.owner));
    report.push_str(" resets=");
    report.push_u64_dec(rng.reset_count);
    report.push_str(" last_error=");
    report.push_str(rng.last_error);
    report.push_str(" io_base=");
    report.push_u64_dec(rng.io_base as u64);
    report.push_byte(b'\n');
    write_virtio_queue_report(report, "virtio-queue[0]", "rng", &rng.queue);

    report.push_str("virtio-runtime[1] device=");
    report.push_str(VIRTIO_NET_DEVICE_ID);
    report.push_str(" initialized=");
    write_yes_no(report, net.initialized);
    report.push_str(" owner=");
    report.push_str(process_name_by_pid(runtime, net.owner));
    report.push_str(" resets=");
    report.push_u64_dec(net.reset_count);
    report.push_str(" last_error=");
    report.push_str(net.last_error);
    report.push_str(" io_base=");
    report.push_u64_dec(net.io_base as u64);
    report.push_str(" rx_posted=");
    write_yes_no(report, net.rx_posted);
    report.push_byte(b'\n');
    write_virtio_queue_report(report, "virtio-queue[1]", "net-rx", &net.rx);
    write_virtio_queue_report(report, "virtio-queue[2]", "net-tx", &net.tx);

    let mut device_count = 0;
    let mut object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(_)) = runtime.objects.objects[object_index] {
            device_count += 1;
        }
        object_index += 1;
    }
    report.push_str("virtio_driver_devices=");
    report.push_u64_dec(device_count);
    report.push_byte(b'\n');

    let mut device_index = 0;
    object_index = 0;
    while object_index < runtime.objects.count {
        if let Some(KernelObject::VirtioDevice(device)) = runtime.objects.objects[object_index] {
            report.push_str("virtio-device-runtime[");
            report.push_u64_dec(device_index);
            report.push_str("] device=");
            report.push_str(device.name);
            report.push_str(" transport=");
            report.push_str(device.transport);
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, device.owner));
            report.push_str(" queue_size=");
            report.push_u64_dec(device.queue_size as u64);
            report.push_str(" avail_idx=");
            report.push_u64_dec(device.avail_idx as u64);
            report.push_str(" used_idx=");
            report.push_u64_dec(device.used_idx as u64);
            report.push_str(" submissions=");
            report.push_u64_dec(device.submissions);
            report.push_str(" completions=");
            report.push_u64_dec(device.completions);
            report.push_str(" timeouts=");
            report.push_u64_dec(device.timeouts);
            report.push_str(" resets=");
            report.push_u64_dec(device.reset_count);
            report.push_str(" last_error=");
            report.push_str(device.last_error);
            report.push_byte(b'\n');
            device_index += 1;
        }
        object_index += 1;
    }
}

fn write_virtio_queue_report(
    report: &mut InspectReport,
    slot: &'static str,
    name: &'static str,
    queue: &VirtioQueueState,
) {
    report.push_str(slot);
    report.push_str(" name=");
    report.push_str(name);
    report.push_str(" queue_size=");
    report.push_u64_dec(queue.queue_size as u64);
    report.push_str(" avail_idx=");
    report.push_u64_dec(queue.avail_idx as u64);
    report.push_str(" used_idx=");
    report.push_u64_dec(queue.used_idx as u64);
    report.push_str(" submissions=");
    report.push_u64_dec(queue.submissions);
    report.push_str(" completions=");
    report.push_u64_dec(queue.completions);
    report.push_str(" interrupt_waits=");
    report.push_u64_dec(queue.interrupt_waits);
    report.push_str(" timeouts=");
    report.push_u64_dec(queue.timeouts);
    report.push_str(" last_error=");
    report.push_str(queue.last_error);
    report.push_str(" dma_physical=");
    report.push_u64_dec(queue.dma_physical);
    report.push_str(" dma_virtual=");
    report.push_u64_dec(queue.dma_virtual);
    report.push_byte(b'\n');
}

fn write_yes_no(report: &mut InspectReport, value: bool) {
    if value {
        report.push_str("yes");
    } else {
        report.push_str("no");
    }
}

fn write_unreachable_object_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let mut leak_index = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(object) = runtime.objects.objects[index]
            && !object_reachable_by_cap(runtime, object.id())
            && !object_reachable_by_config(runtime, object.id())
            && !object_reachable_by_owner(runtime, object)
        {
            report.push_str("object-unreachable[");
            report.push_u64_dec(leak_index);
            report.push_str("] ");
            write_capability_object_report(runtime, report, object.id());
            report.push_byte(b'\n');
            leak_index += 1;
        }
        index += 1;
    }
}

fn frame_allocator_stats() -> Option<memory::AllocatorStats> {
    let allocator = unsafe { *FRAME_ALLOCATOR.0.get() }?;
    unsafe { allocator.as_ref().map(|allocator| allocator.stats()) }
}

fn runtime_cap_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            count += cap_count_in_space(process.caps);
            count += cap_count_in_space(process.initial_caps);
        }
        process_index += 1;
    }
    count
}

fn write_state_policy_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let Some(config) = runtime.active_config else {
        report.push_str("state-policy v=1 status=unavailable\n");
        return;
    };
    let manager = boot_manager_state();
    let mut index = 0;
    while index < config.state_volume_count {
        if let Some(state) = config.state_volumes[index] {
            report.push_str("state-policy[");
            report.push_u64_dec(index as u64);
            report.push_str("] id=");
            report.push_str(state.id);
            report.push_str(" owner=");
            report.push_str(state.owner);
            report.push_str(" schema=");
            report.push_str(state.schema_version);
            report.push_str(" storage=");
            report.push_str(state.storage_class);
            report.push_str(" migration=");
            report.push_str(state.migration_policy);
            report.push_str(" retention=");
            report.push_str(state.retention_policy);
            report.push_str(" sharing=");
            report.push_str(state.sharing_policy);
            report.push_str(" generation=");
            report.push_str(config.generation_id);
            report.push_byte(b'\n');
            report.push_str("state-health[");
            report.push_u64_dec(index as u64);
            report.push_str("] id=");
            report.push_str(state.id);
            report.push_str(" owner=");
            report.push_str(state.owner);
            report.push_str(" schema=");
            report.push_str(state.schema_version);
            report.push_str(" generation=");
            report.push_str(config.generation_id);
            report.push_str(" migration_status=");
            if manager.last_state_migration_state == state.id {
                report.push_str(manager.last_state_migration_status);
                report.push_str(" last_error=");
                if manager.last_state_migration_error.is_empty() {
                    report.push_str("none");
                } else {
                    report.push_str(manager.last_state_migration_error);
                }
            } else {
                report.push_str("clean last_error=none");
            }
            report.push_str(" retention=");
            report.push_str(state.retention_policy);
            report.push_byte(b'\n');
        }
        index += 1;
    }
}

fn runtime_file_handle_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            let mut handle_index = 0;
            while handle_index < process.file_handles.len() {
                if process.file_handles[handle_index].handle.is_some() {
                    count += 1;
                }
                handle_index += 1;
            }
        }
        process_index += 1;
    }
    count
}

fn write_vfs_report(runtime: &RuntimeState, report: &mut InspectReport) {
    let mut mount_index = 0;
    while mount_index < runtime.objects.count {
        if let Some(KernelObject::VfsMount(mount)) = runtime.objects.objects[mount_index] {
            report.push_str("vfs-mount[");
            report.push_u64_dec(mount_index as u64);
            report.push_str("] id=");
            report.push_u64_dec(mount.id.raw());
            report.push_str(" name=");
            report.push_str(mount.name);
            report.push_str(" root=");
            report.push_bytes(mount.root_path.as_bytes());
            report.push_str(" root_vnode=");
            report.push_u64_dec(mount.root_node.raw());
            report.push_str(" source=");
            report.push_str(mount.source);
            report.push_str(" flags=");
            write_vfs_mount_flags(report, mount.flags);
            report.push_str(" dynamic=");
            report.push_str(if mount.dynamic { "yes" } else { "no" });
            report.push_str(" owner=");
            if mount.owner == ProcessId::empty() {
                report.push_str("system");
            } else {
                report.push_str(process_name_by_pid(runtime, mount.owner));
                report.push_str(":");
                report.push_u64_dec(mount.owner.raw());
            }
            report.push_byte(b'\n');
        }
        mount_index += 1;
    }

    let mut index = 0;
    while index < runtime.vfs_node_count {
        if let Some(node) = runtime.vfs_nodes[index] {
            report.push_str("vfs-node[");
            report.push_u64_dec(index as u64);
            report.push_str("] id=");
            report.push_u64_dec(node.id.raw());
            report.push_str(" kind=");
            match node.kind {
                VfsNodeKind::RegularFile => report.push_str("regular"),
                VfsNodeKind::Directory => report.push_str("directory"),
                VfsNodeKind::DeviceNode => report.push_str("device"),
                VfsNodeKind::Pipe => report.push_str("pipe"),
                VfsNodeKind::SyntheticNode => report.push_str("synthetic"),
            }
            report.push_str(" name=");
            report.push_bytes(node.name.as_bytes());
            report.push_str(" mount=");
            report.push_str(node.mount_source);
            if let Some(parent) = node.parent {
                report.push_str(" parent=");
                report.push_u64_dec(parent.raw());
            }
            match node.backing {
                VfsBacking::None => report.push_str(" backing=none"),
                VfsBacking::StoreObject(object) => {
                    report.push_str(" backing=store-object object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolume(object) => {
                    report.push_str(" backing=state-volume object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolumeValue(object) => {
                    report.push_str(" backing=state-volume-value object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::StateVolumeControl(object) => {
                    report.push_str(" backing=state-volume-control object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::MemoryFile(file) => {
                    report.push_str(" backing=memory-file index=");
                    report.push_u64_dec(file as u64);
                }
                VfsBacking::VertexFsFile(file) => {
                    report.push_str(" backing=vertexfs-file index=");
                    report.push_u64_dec(file as u64);
                    if file < runtime.vertexfs_file_count {
                        report.push_str(" dirty=");
                        write_yes_no(report, runtime.vertexfs_files[file].dirty);
                        report.push_str(" checksum=");
                        report.push_u64_dec(runtime.vertexfs_files[file].checksum as u64);
                    }
                }
                VfsBacking::Device(object) => {
                    report.push_str(" backing=device object_id=");
                    report.push_u64_dec(object.raw());
                }
                VfsBacking::Synthetic(_) => report.push_str(" backing=synthetic"),
                VfsBacking::FsServiceReport => report.push_str(" backing=fs-service"),
                VfsBacking::Pipe => report.push_str(" backing=pipe"),
            }
            report.push_byte(b'\n');
        }
        index += 1;
    }

    let mut process_index = 0;
    let mut handle_row = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            let mut handle_index = 0;
            while handle_index < process.file_handles.len() {
                if let Some(handle) = process.file_handles[handle_index].handle
                    && let Some(description) = runtime.file_description(handle.description)
                {
                    report.push_str("vfs-handle[");
                    report.push_u64_dec(handle_row);
                    report.push_str("] owner=");
                    report.push_str(process.name);
                    report.push_str(" slot=");
                    report.push_u64_dec((handle_index + 1) as u64);
                    report.push_str(" description=");
                    report.push_u64_dec(description.id.raw());
                    report.push_str(" vnode=");
                    report.push_u64_dec(description.node.raw());
                    report.push_str(" rights=");
                    write_rights_report(report, description.rights);
                    report.push_str(" flags=");
                    report.push_u64_dec(description.flags);
                    report.push_str(" offset=");
                    report.push_u64_dec(description.offset);
                    report.push_str(" refs=");
                    report.push_u64_dec(description.ref_count);
                    report.push_byte(b'\n');
                    handle_row += 1;
                }
                handle_index += 1;
            }
        }
        process_index += 1;
    }

    let mut lock_index = 0;
    while lock_index < runtime.vfs_locks.len() {
        if let Some(lock) = runtime.vfs_locks[lock_index] {
            report.push_str("vfs-lock[");
            report.push_u64_dec(lock_index as u64);
            report.push_str("] owner=");
            if let Some(process) = runtime.processes.process(lock.owner) {
                report.push_str(process.name);
            } else {
                report.push_str("<dead>");
            }
            report.push_str(" vnode=");
            report.push_u64_dec(lock.node.raw());
            report.push_str(" description=");
            report.push_u64_dec(lock.description.raw());
            report.push_str(" mode=");
            match lock.mode {
                VfsLockMode::Shared => report.push_str("shared"),
                VfsLockMode::Exclusive => report.push_str("exclusive"),
            }
            report.push_str(" range=");
            report.push_u64_dec(lock.start);
            report.push_str("+");
            report.push_u64_dec(lock.len);
            report.push_byte(b'\n');
        }
        lock_index += 1;
    }

    report.push_str("vfs-pipe buffered=");
    report.push_u64_dec(runtime.vfs_pipe.len as u64);
    report.push_byte(b'\n');

    let mut event_index = 0;
    while event_index < runtime.vfs_event_count {
        if let Some(event) = runtime.vfs_events[event_index] {
            report.push_str("vfs-event[");
            report.push_u64_dec(event_index as u64);
            report.push_str("] parent=");
            report.push_u64_dec(event.parent.raw());
            report.push_str(" kind=");
            report.push_u64_dec(event.kind);
            report.push_str(" version=");
            report.push_u64_dec(event.metadata_version);
            report.push_str(" name=");
            report.push_bytes(event.name.as_bytes());
            report.push_byte(b'\n');
        }
        event_index += 1;
    }
}

fn write_vfs_mount_flags(report: &mut InspectReport, flags: u64) {
    let mut wrote = false;
    if flags & VFS_MOUNT_VOLATILE != 0 {
        report.push_str("volatile");
        wrote = true;
    }
    if flags & VFS_MOUNT_BIND != 0 {
        if wrote {
            report.push_str("|");
        }
        report.push_str("bind");
        wrote = true;
    }
    if flags & VFS_MOUNT_READ_ONLY != 0 {
        if wrote {
            report.push_str("|");
        }
        report.push_str("read-only");
        wrote = true;
    }
    if !wrote {
        report.push_str("none");
    }
}

fn cap_count_in_space(space: CapabilitySpace) -> u64 {
    let mut count = 0;
    let mut slot = 0;
    while slot < MAX_CAPS {
        if space.caps[slot].is_some() {
            count += 1;
        }
        slot += 1;
    }
    count
}

fn unreachable_object_count(runtime: &RuntimeState) -> u64 {
    let mut count = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(object) = runtime.objects.objects[index]
            && !object_reachable_by_cap(runtime, object.id())
            && !object_reachable_by_config(runtime, object.id())
            && !object_reachable_by_owner(runtime, object)
        {
            count += 1;
        }
        index += 1;
    }
    count
}

pub(super) fn object_reachable_by_cap(runtime: &RuntimeState, object_id: KernelObjectId) -> bool {
    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index]
            && process.state != ProcessState::Empty
            && ((process.state != ProcessState::Exited
                && cap_space_reaches_live_object(runtime, process.caps, object_id))
                || cap_space_reaches_live_object(runtime, process.initial_caps, object_id))
        {
            return true;
        }
        process_index += 1;
    }
    false
}

fn object_reachable_by_config(runtime: &RuntimeState, object_id: KernelObjectId) -> bool {
    id_list_contains(&runtime.endpoint_ids, object_id)
        || id_list_contains(&runtime.store_object_ids, object_id)
        || id_list_contains(&runtime.state_volume_ids, object_id)
        || id_list_contains(&runtime.network_port_ids, object_id)
        || id_list_contains(&runtime.io_port_ids, object_id)
        || id_list_contains(&runtime.mmio_region_ids, object_id)
        || id_list_contains(&runtime.framebuffer_ids, object_id)
        || id_list_contains(&runtime.interrupt_line_ids, object_id)
        || id_list_contains(&runtime.dma_region_ids, object_id)
        || id_list_contains(&runtime.pci_device_ids, object_id)
        || id_list_contains(&runtime.virtio_device_ids, object_id)
        || id_list_contains(&runtime.namespace_ids, object_id)
        || id_list_contains(&runtime.vfs_root_ids, object_id)
        || id_list_contains(&runtime.vfs_mount_ids, object_id)
        || runtime.timer_id == Some(object_id)
        || runtime.process_control_id == Some(object_id)
        || runtime.secret_id == Some(object_id)
        || runtime.state_vfs_request_endpoint == Some(object_id)
        || runtime.state_vfs_reply_endpoint == Some(object_id)
        || runtime.vertexfs_device_request_endpoint == Some(object_id)
        || runtime.vertexfs_device_reply_endpoint == Some(object_id)
}

fn object_reachable_by_owner(runtime: &RuntimeState, object: KernelObject) -> bool {
    match object {
        KernelObject::IpcEndpoint(endpoint) if endpoint.owner != ProcessId::empty() => runtime
            .processes
            .process(endpoint.owner)
            .map(|process| {
                process.state != ProcessState::Empty && process.state != ProcessState::Exited
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn id_list_contains(ids: &[Option<KernelObjectId>], object_id: KernelObjectId) -> bool {
    let mut index = 0;
    while index < ids.len() {
        if ids[index] == Some(object_id) {
            return true;
        }
        index += 1;
    }
    false
}

fn cap_space_reaches_live_object(
    runtime: &RuntimeState,
    space: CapabilitySpace,
    object_id: KernelObjectId,
) -> bool {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.object == object_id
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
        {
            return true;
        }
        slot += 1;
    }
    false
}

fn write_capability_space_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    process: Process,
    space_name: &str,
    space: CapabilitySpace,
) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot] {
            report.push_str("space=");
            report.push_str(space_name);
            report.push_str(" proc=");
            report.push_str(process.name);
            report.push_str(" cap[");
            report.push_u64_dec(slot as u64);
            report.push_str("] ");
            write_capability_object_report(runtime, report, cap.object);
            report.push_str(" rights=");
            write_rights_report(report, cap.rights);
            report.push_str(" cap_id=");
            report.push_u64_dec(cap.id);
            report.push_str(" parent_cap_id=");
            report.push_u64_dec(cap.parent_cap_id);
            report.push_str(" generation=");
            report.push_str(cap.generation_id);
            report.push_str(" graph_from=");
            report.push_str(process_graph_node(runtime, process.name));
            report.push_str(" graph_target=");
            write_capability_graph_target(runtime, report, cap.object);
            report.push_str(" graph_edge=");
            write_capability_graph_edge(runtime, report, process.name, slot, cap);
            report.push_str(" owner_pid=");
            report.push_u64_dec(cap.owner_process.raw());
            report.push_str(" owner=");
            report.push_str(process_name_by_pid(runtime, cap.owner_process));
            report.push_str(" delegated_by_pid=");
            report.push_u64_dec(cap.delegated_by.raw());
            report.push_str(" delegated_by=");
            report.push_str(process_name_by_pid(runtime, cap.delegated_by));
            report.push_str(" revoked=");
            report.push_str(if cap.revoked || runtime.cap_id_revoked(cap.id) {
                "yes"
            } else {
                "no"
            });
            report.push_byte(b'\n');
        }
        slot += 1;
    }
}

fn process_graph_node(runtime: &RuntimeState, process_name: &str) -> &'static str {
    let Some(config) = runtime.active_config else {
        return "<unknown>";
    };
    let mut index = 0;
    while index < config.process_count {
        if let Some(process) = config.processes[index]
            && process.name == process_name
        {
            return process.graph_node;
        }
        index += 1;
    }
    "<unknown>"
}

fn write_capability_graph_target(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    object: KernelObjectId,
) {
    if let Some(target) = graph_node_for_object(runtime, object) {
        report.push_str(target);
    } else {
        report.push_str("<unknown>");
    }
}

fn write_capability_graph_edge(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    process_name: &str,
    slot: usize,
    cap: Capability,
) {
    if let Some(index) = boot_grant_index_for_cap(runtime, process_name, slot, cap.object) {
        report.push_str("grant:");
        report.push_u64_dec(index as u64);
        return;
    }
    if let Some(target) = graph_node_for_object(runtime, cap.object)
        && target == "secret:logd-token"
    {
        report.push_str("grant:secret-logd-token");
        return;
    }
    report.push_str("runtime-derived");
}

fn boot_grant_index_for_cap(
    runtime: &RuntimeState,
    process_name: &str,
    slot: usize,
    object: KernelObjectId,
) -> Option<usize> {
    let config = runtime.active_config?;
    let mut process_index = 0;
    while process_index < config.process_count {
        let process = config.processes[process_index]?;
        if process.name == process_name {
            let mut grant_index = 0;
            while grant_index < config.grant_count {
                let grant = config.grants[grant_index]?;
                if grant.process_index == process_index
                    && grant.cap_slot == slot as u64
                    && grant_object_id(runtime, grant).ok() == Some(object)
                {
                    return Some(grant_index);
                }
                grant_index += 1;
            }
        }
        process_index += 1;
    }
    None
}

fn graph_node_for_object(runtime: &RuntimeState, object: KernelObjectId) -> Option<&'static str> {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    return Some(endpoint.name);
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    return Some(store.name);
                }
                KernelObject::StateVolume(state) if state.id == object => {
                    return Some(state.name);
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    return Some(timer.name);
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    return Some(port.name);
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    return Some(port.name);
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    return Some(region.name);
                }
                KernelObject::Framebuffer(framebuffer) if framebuffer.id == object => {
                    return Some(framebuffer.name);
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    return Some(line.name);
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    return Some(region.name);
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    return Some(device.name);
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    return Some(device.name);
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    return Some(namespace.name);
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    return Some(root.name);
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    return Some(secret.name);
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

pub(super) fn process_name_by_pid(runtime: &RuntimeState, pid: ProcessId) -> &'static str {
    if pid == ProcessId::empty() {
        return "kernel";
    }

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && process.pid == pid
        {
            return process.name;
        }
        index += 1;
    }

    "<unknown>"
}

fn write_capability_object_report(
    runtime: &RuntimeState,
    report: &mut InspectReport,
    object: KernelObjectId,
) {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    report.push_str("endpoint=");
                    report.push_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    report.push_str("boot-module=");
                    report.push_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    if store.name.starts_with("config:") {
                        report.push_str("config=");
                    } else {
                        report.push_str("store-object=");
                    }
                    report.push_str(store.name);
                    return;
                }
                KernelObject::StateVolume(state) if state.id == object => {
                    report.push_str("state-volume=");
                    report.push_str(state.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    report.push_str("timer=");
                    report.push_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    report.push_str("network-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    report.push_str("io-port=");
                    report.push_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    report.push_str("mmio-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::Framebuffer(framebuffer) if framebuffer.id == object => {
                    report.push_str("framebuffer=");
                    report.push_str(framebuffer.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    report.push_str("interrupt-line=");
                    report.push_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    report.push_str("dma-region=");
                    report.push_str(region.name);
                    return;
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    report.push_str("pci-device=");
                    report.push_str(device.name);
                    report.push_str(" kind=");
                    report.push_str(device.kind);
                    return;
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    report.push_str("virtio-device=");
                    report.push_str(device.name);
                    report.push_str(" transport=");
                    report.push_str(device.transport);
                    return;
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    report.push_str("namespace=");
                    report.push_str(namespace.name);
                    return;
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    report.push_str("vfs-root=");
                    report.push_str(root.name);
                    report.push_str(" root=");
                    report.push_bytes(root.root_path.as_bytes());
                    return;
                }
                KernelObject::VfsMount(mount) if mount.id == object => {
                    report.push_str("vfs-mount=");
                    report.push_str(mount.name);
                    report.push_str(" root=");
                    report.push_bytes(mount.root_path.as_bytes());
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    report.push_str("process-control=");
                    report.push_str(process_control.name);
                    return;
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    report.push_str("secret=");
                    report.push_str(secret.name);
                    report.push_str(" value=<redacted>");
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    report.push_str("object=");
    report.push_u64_dec(object.raw());
}

fn write_rights_report(report: &mut InspectReport, rights: u64) {
    let mut wrote = false;
    wrote = write_right_report(report, rights, capability::RIGHT_READ, "read", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_SEND, "send", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_SNAPSHOT,
        "snapshot",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_MAP, "map", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_ALLOCATE,
        "allocate",
        wrote,
    );
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_DELEGATE,
        "delegate",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_INSPECT, "inspect", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_CREATE, "create", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_UNLINK, "unlink", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_RENAME, "rename", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_MOUNT, "mount", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_START, "start", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_KILL, "kill", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_WAIT, "wait", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_DERIVE, "derive", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_SEAL, "seal", wrote);
    wrote = write_right_report(report, rights, capability::RIGHT_UNSEAL, "unseal", wrote);
    wrote = write_right_report(
        report,
        rights,
        capability::RIGHT_INSPECT_METADATA,
        "inspect-metadata",
        wrote,
    );
    wrote = write_right_report(report, rights, capability::RIGHT_RESOLVE, "resolve", wrote);

    if !wrote {
        report.push_str("none");
    }
}

fn write_right_report(
    report: &mut InspectReport,
    rights: u64,
    right: u64,
    label: &str,
    wrote: bool,
) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        report.push_byte(b'|');
    }
    report.push_str(label);
    true
}

pub(super) fn print_boot_tables(runtime: &RuntimeState) {
    serial::write_str("Process table entries: ");
    serial::write_u64_dec(runtime.processes.count as u64);
    serial::write_str("\n");

    serial::write_str("Endpoint table entries: ");
    serial::write_u64_dec(runtime.objects.endpoint_count() as u64);
    serial::write_str("\n");

    print_endpoint_labels(runtime);

    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            print_process_state(index, &process);
            print_process_caps(&process);
        }
        index += 1;
    }
}

fn print_endpoint_labels(runtime: &RuntimeState) {
    let mut printed = 0;
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::IpcEndpoint(endpoint)) = runtime.objects.objects[index] {
            serial::write_str("endpoint[");
            serial::write_u64_dec(printed as u64);
            serial::write_str("] id=");
            serial::write_u64_dec(endpoint.id.raw());
            serial::write_str(" name=");
            serial::write_str(endpoint.name);
            serial::write_str("\n");
            printed += 1;
        }
        index += 1;
    }
}

pub(super) fn print_process_by_pid(runtime: &RuntimeState, pid: ProcessId) {
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && process.pid == pid
        {
            print_process_state(index, &process);
            print_process_caps(&process);
            return;
        }
        index += 1;
    }
}

fn print_process_caps(process: &Process) {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = process.caps.caps[slot] {
            serial::write_str("proc=");
            serial::write_str(process.name);
            serial::write_str(" cap[");
            serial::write_u64_dec(slot as u64);
            serial::write_str("] ");
            print_capability_object(cap.object);
            serial::write_str(" rights=");
            print_rights(cap.rights);
            serial::write_str("\n");
        }
        slot += 1;
    }
}

fn print_capability_object(object: KernelObjectId) {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.objects.count {
        if let Some(entry) = runtime.objects.objects[index] {
            match entry {
                KernelObject::IpcEndpoint(endpoint) if endpoint.id == object => {
                    serial::write_str("endpoint=");
                    serial::write_str(endpoint.name);
                    return;
                }
                KernelObject::BootModule(module) if module.id == object => {
                    serial::write_str("boot-module=");
                    serial::write_str(module.name);
                    return;
                }
                KernelObject::StoreObject(store) if store.id == object => {
                    if store.name.starts_with("config:") {
                        serial::write_str("config=");
                    } else {
                        serial::write_str("store-object=");
                    }
                    serial::write_str(store.name);
                    return;
                }
                KernelObject::Timer(timer) if timer.id == object => {
                    serial::write_str("timer=");
                    serial::write_str(timer.name);
                    return;
                }
                KernelObject::NetworkPort(port) if port.id == object => {
                    serial::write_str("network-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::IoPortRange(port) if port.id == object => {
                    serial::write_str("io-port=");
                    serial::write_str(port.name);
                    return;
                }
                KernelObject::MmioRegion(region) if region.id == object => {
                    serial::write_str("mmio-region=");
                    serial::write_str(region.name);
                    return;
                }
                KernelObject::Framebuffer(framebuffer) if framebuffer.id == object => {
                    serial::write_str("framebuffer=");
                    serial::write_str(framebuffer.name);
                    return;
                }
                KernelObject::InterruptLine(line) if line.id == object => {
                    serial::write_str("interrupt-line=");
                    serial::write_str(line.name);
                    return;
                }
                KernelObject::DmaRegion(region) if region.id == object => {
                    serial::write_str("dma-region=");
                    serial::write_str(region.name);
                    serial::write_str(" base=");
                    serial::write_u64_hex(region.base);
                    serial::write_str(" length=");
                    serial::write_u64_hex(region.length);
                    return;
                }
                KernelObject::PciDevice(device) if device.id == object => {
                    serial::write_str("pci-device=");
                    serial::write_str(device.name);
                    serial::write_str(" kind=");
                    serial::write_str(device.kind);
                    return;
                }
                KernelObject::VirtioDevice(device) if device.id == object => {
                    serial::write_str("virtio-device=");
                    serial::write_str(device.name);
                    serial::write_str(" transport=");
                    serial::write_str(device.transport);
                    return;
                }
                KernelObject::Namespace(namespace) if namespace.id == object => {
                    serial::write_str("namespace=");
                    serial::write_str(namespace.name);
                    return;
                }
                KernelObject::VfsRoot(root) if root.id == object => {
                    serial::write_str("vfs-root=");
                    serial::write_str(root.name);
                    serial::write_str(" root=");
                    serial::write_ascii_bytes(root.root_path.as_bytes());
                    return;
                }
                KernelObject::VfsMount(mount) if mount.id == object => {
                    serial::write_str("vfs-mount=");
                    serial::write_str(mount.name);
                    serial::write_str(" root=");
                    serial::write_ascii_bytes(mount.root_path.as_bytes());
                    return;
                }
                KernelObject::ProcessControl(process_control) if process_control.id == object => {
                    serial::write_str("process-control=");
                    serial::write_str(process_control.name);
                    return;
                }
                KernelObject::Secret(secret) if secret.id == object => {
                    serial::write_str("secret=");
                    serial::write_str(secret.name);
                    serial::write_str(" value=<redacted>");
                    return;
                }
                _ => {}
            }
        }
        index += 1;
    }

    serial::write_str("object=");
    serial::write_u64_dec(object.raw());
}

fn print_process_state(index: usize, process: &Process) {
    serial::write_str("process[");
    serial::write_u64_dec(index as u64);
    serial::write_str("] id=");
    serial::write_u64_dec(process.pid.raw());
    serial::write_str(" name=");
    serial::write_str(process.name);
    serial::write_str(" state=");
    serial::write_str(process.state.label());
    serial::write_str(" quota_caps=");
    serial::write_u64_dec(process.quota.max_caps);
    serial::write_str(" quota_endpoints=");
    serial::write_u64_dec(process.quota.max_endpoints);
    serial::write_str(" quota_memory_pages=");
    serial::write_u64_dec(process.quota.max_memory_pages);
    serial::write_str(" quota_child_processes=");
    serial::write_u64_dec(process.quota.max_child_processes);
    serial::write_str(" quota_ipc_bytes=");
    serial::write_u64_dec(process.quota.max_ipc_bytes);
    serial::write_str(" mount_root=");
    serial::write_ascii_bytes(process.mount_root.as_bytes());
    serial::write_str("\n");
}

pub(super) fn print_rights(rights: u64) {
    let mut wrote = false;
    wrote = print_right(rights, capability::RIGHT_READ, "read", wrote);
    wrote = print_right(rights, capability::RIGHT_WRITE, "write", wrote);
    wrote = print_right(rights, capability::RIGHT_SEND, "send", wrote);
    wrote = print_right(rights, capability::RIGHT_RECEIVE, "receive", wrote);
    wrote = print_right(rights, capability::RIGHT_CONTROL, "control", wrote);
    wrote = print_right(rights, capability::RIGHT_SNAPSHOT, "snapshot", wrote);
    wrote = print_right(rights, capability::RIGHT_RESTORE, "restore", wrote);
    wrote = print_right(rights, capability::RIGHT_MAP, "map", wrote);
    wrote = print_right(rights, capability::RIGHT_BIND, "bind", wrote);
    wrote = print_right(rights, capability::RIGHT_LISTEN, "listen", wrote);
    wrote = print_right(rights, capability::RIGHT_ALLOCATE, "allocate", wrote);
    wrote = print_right(rights, capability::RIGHT_DELEGATE, "delegate", wrote);
    wrote = print_right(rights, capability::RIGHT_REVOKE, "revoke", wrote);
    wrote = print_right(rights, capability::RIGHT_INSPECT, "inspect", wrote);
    wrote = print_right(rights, capability::RIGHT_CREATE, "create", wrote);
    wrote = print_right(rights, capability::RIGHT_UNLINK, "unlink", wrote);
    wrote = print_right(rights, capability::RIGHT_RENAME, "rename", wrote);
    wrote = print_right(rights, capability::RIGHT_MOUNT, "mount", wrote);
    wrote = print_right(rights, capability::RIGHT_START, "start", wrote);
    wrote = print_right(rights, capability::RIGHT_KILL, "kill", wrote);
    wrote = print_right(rights, capability::RIGHT_WAIT, "wait", wrote);
    wrote = print_right(rights, capability::RIGHT_DERIVE, "derive", wrote);
    wrote = print_right(rights, capability::RIGHT_SEAL, "seal", wrote);
    wrote = print_right(rights, capability::RIGHT_UNSEAL, "unseal", wrote);
    wrote = print_right(
        rights,
        capability::RIGHT_INSPECT_METADATA,
        "inspect-metadata",
        wrote,
    );
    wrote = print_right(rights, capability::RIGHT_RESOLVE, "resolve", wrote);

    if !wrote {
        serial::write_str("none");
    }
}

fn print_right(rights: u64, right: u64, label: &str, wrote: bool) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        serial::write_str("|");
    }
    serial::write_str(label);
    true
}

pub(super) fn print_negative(operation: &str) {
    serial::write_str("IPC negative test: ");
    serial::write_str(current_process_label());
    serial::write_str(" ");
    serial::write_str(operation);
    serial::write_str(" rejected: bad capability\n");
}

pub(super) fn inspect_report() -> &'static mut InspectReport {
    unsafe { &mut *INSPECT_REPORT.0.get() }
}
