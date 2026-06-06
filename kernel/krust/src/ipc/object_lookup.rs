use super::*;

pub(super) fn endpoint_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<KernelObjectId, IpcError> {
    endpoint_cap_from_slot(cap_slot, required_right).map(|cap| cap.object)
}

pub(super) fn endpoint_cap_from_slot(
    cap_slot: u64,
    required_right: u64,
) -> Result<Capability, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;

    match runtime().objects.get_endpoint(cap.object) {
        Some(_) => Ok(cap),
        None => Err(IpcError::BadCapability),
    }
}

pub(super) fn serial_log_endpoint_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<IpcEndpoint, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    let runtime = runtime();
    let log_endpoint_id = runtime.endpoint_ids[0].ok_or(IpcError::BadCapability)?;
    if cap.object != log_endpoint_id {
        return Err(IpcError::BadCapability);
    }
    let endpoint = runtime
        .objects
        .get_endpoint(cap.object)
        .ok_or(IpcError::BadCapability)?;
    if endpoint.name != LOG_ENDPOINT_NAME {
        return Err(IpcError::BadCapability);
    }
    Ok(endpoint)
}

pub(super) fn boot_module_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<BootModuleObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_boot_module(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn timer_from_cap(cap_slot: u64, required_right: u64) -> Result<TimerObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_timer(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn network_port_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<NetworkPortObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_network_port(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn io_port_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<IoPortRangeObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_io_port(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn mmio_region_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<MmioRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_mmio_region(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn interrupt_line_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<InterruptLineObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_interrupt_line(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn dma_region_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<DmaRegionObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_dma_region(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn virtio_device_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<VirtioDeviceObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_virtio_device(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn namespace_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<NamespaceObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_namespace(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn process_control_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<ProcessControlObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_process_control(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn secret_from_cap(
    cap_slot: u64,
    required_right: u64,
) -> Result<SecretObject, IpcError> {
    let cap = lookup_capability(cap_slot, required_right)?;
    runtime()
        .objects
        .get_secret(cap.object)
        .ok_or(IpcError::BadCapability)
}

pub(super) fn lookup_capability(
    cap_slot: u64,
    required_right: u64,
) -> Result<Capability, IpcError> {
    let runtime = runtime();
    let process = runtime
        .processes
        .current_process()
        .ok_or(IpcError::BadCapability)?;
    let cap = process
        .caps
        .lookup(cap_slot)
        .ok_or(IpcError::BadCapability)?;

    if cap.revoked
        || runtime.cap_id_revoked(cap.id)
        || capability_has_revoked_ancestor(runtime, cap)
        || cap.generation_id != runtime.generation_id
    {
        return Err(IpcError::BadCapability);
    }

    if required_right != 0 && cap.rights & required_right != required_right {
        return Err(IpcError::BadCapability);
    }

    Ok(cap)
}

pub(super) fn port_in_range(range: IoPortRangeObject, port: u64) -> bool {
    port >= range.base
        && port
            .checked_sub(range.base)
            .map(|offset| offset < range.length)
            .unwrap_or(false)
}

pub(super) fn port_span_in_range(range: IoPortRangeObject, port: u64, width: u64) -> bool {
    if width == 0 {
        return false;
    }
    let Some(last_port) = port.checked_add(width - 1) else {
        return false;
    };
    if last_port > u16::MAX as u64 {
        return false;
    }
    port_in_range(range, port) && port_in_range(range, last_port)
}

pub(super) fn capability_has_revoked_ancestor(runtime: &RuntimeState, cap: Capability) -> bool {
    let mut parent = cap.parent_cap_id;
    while parent != 0 {
        if runtime.cap_id_revoked(parent) {
            return true;
        }
        parent = find_cap_parent(runtime, parent).unwrap_or(0);
    }
    false
}

pub(super) fn find_cap_parent(runtime: &RuntimeState, cap_id: u64) -> Option<u64> {
    if let Some(parent) = runtime.cap_parent_from_lineage(cap_id) {
        return Some(parent);
    }

    let mut process_index = 0;
    while process_index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[process_index] {
            if let Some(parent) = find_cap_parent_in_space(process.caps, cap_id) {
                return Some(parent);
            }
            if let Some(parent) = find_cap_parent_in_space(process.initial_caps, cap_id) {
                return Some(parent);
            }
        }
        process_index += 1;
    }
    None
}

pub(super) fn find_cap_parent_in_space(space: CapabilitySpace, cap_id: u64) -> Option<u64> {
    let mut slot = 0;
    while slot < MAX_CAPS {
        if let Some(cap) = space.caps[slot]
            && cap.id == cap_id
        {
            return Some(cap.parent_cap_id);
        }
        slot += 1;
    }
    None
}

pub(super) fn restart_policy_label(policy: u16) -> &'static str {
    match policy {
        1 => "on-failure",
        2 => "always",
        _ => "none",
    }
}

pub(super) fn current_process_label() -> &'static str {
    runtime()
        .processes
        .current_process()
        .map(|process| process.name)
        .unwrap_or("<none>")
}

pub(super) fn runtime() -> &'static mut RuntimeState {
    unsafe { &mut *RUNTIME.0.get() }
}

pub(super) fn staging_runtime() -> &'static mut RuntimeState {
    unsafe { &mut *INSTALL_STAGING_RUNTIME.0.get() }
}

pub(super) fn frame_allocator() -> Result<&'static mut memory::FrameAllocator, IpcError> {
    let allocator = unsafe { *FRAME_ALLOCATOR.0.get() }.ok_or(IpcError::BadCapability)?;
    unsafe { allocator.as_mut().ok_or(IpcError::BadCapability) }
}
