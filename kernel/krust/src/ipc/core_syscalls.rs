use super::vfs_transactions::{
    wake_blocked_vertexfs_sync_reply, wake_blocked_vfs_pipe_read, wake_blocked_vfs_state_reply,
};
use super::*;

pub fn send(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let endpoint_id = match endpoint_from_cap(cap_slot, capability::RIGHT_SEND) {
        Ok(endpoint_id) => endpoint_id,
        Err(error) => {
            print_negative("send");
            return Err(error);
        }
    };

    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    let sender = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);

    let endpoint = runtime()
        .objects
        .get_endpoint_mut(endpoint_id)
        .ok_or(IpcError::BadCapability)?;

    endpoint.enqueue(sender, &message, len)?;

    if serial::trace_enabled() {
        serial::write_str("IPC send accepted: endpoint=");
        serial::write_u64_dec(endpoint.id.raw());
        serial::write_str(" bytes=");
        serial::write_u64_dec(len as u64);
        serial::write_str("\n");
    }

    wake_blocked_receiver(endpoint_id);
    wake_blocked_vfs_state_reply(endpoint_id);
    wake_blocked_vertexfs_sync_reply(endpoint_id);

    Ok(())
}

pub fn receive(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    receive_with_timeout(cap_slot, destination, max_len, None, frame)
}

pub fn receive_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_ms: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    receive_with_timeout(
        cap_slot,
        destination,
        max_len,
        Some(deadline_after_ms(timeout_ms)),
        frame,
    )
}

fn receive_with_timeout(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let endpoint_cap = match endpoint_cap_from_slot(cap_slot, capability::RIGHT_RECEIVE) {
        Ok(endpoint_cap) => endpoint_cap,
        Err(error) => {
            print_negative("receive");
            return Err(error);
        }
    };
    let endpoint_id = endpoint_cap.object;

    usercopy::validate_user_buffer(
        UserPtr::new(destination as u64),
        max_len,
        paging::UserAccess::Write,
    )
    .map_err(|_| IpcError::InvalidUserBuffer)?;

    let current_pid = runtime()
        .processes
        .current_process()
        .map(|process| process.pid)
        .unwrap_or_else(ProcessId::empty);
    let queued_message = {
        let endpoint = runtime()
            .objects
            .get_endpoint_mut(endpoint_id)
            .ok_or(IpcError::BadCapability)?;
        endpoint.dequeue_for(current_pid)
    };

    let Some(message) = queued_message else {
        if block_current_on_endpoint(
            endpoint_id,
            endpoint_cap.id,
            destination as u64,
            max_len,
            timeout_tsc,
            frame,
        ) {
            return Ok(());
        }

        return Err(IpcError::Empty);
    };

    let copy_len = min(message.len, max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &message.bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    record_ready_lifecycle(endpoint_id, current_pid, message);

    if serial::trace_enabled() {
        serial::write_str("IPC receive delivered: endpoint=");
        serial::write_u64_dec(endpoint_id.raw());
        serial::write_str(" bytes=");
        serial::write_u64_dec(copy_len as u64);
        serial::write_str("\n");
    }

    frame.rax = copy_len as u64;
    Ok(())
}

pub(super) fn record_ready_lifecycle(
    endpoint: KernelObjectId,
    receiver: ProcessId,
    message: IpcMessage,
) {
    let Some(ready_service_name) = ready_service_name(&message) else {
        return;
    };

    let service = {
        let runtime = runtime();
        let Some(endpoint) = runtime.objects.get_endpoint(endpoint) else {
            return;
        };
        if endpoint.name != "readiness" || receiver.raw() != 1 {
            return;
        }

        let Some(process) = runtime.processes.process(message.sender) else {
            return;
        };
        process.name
    };

    if ready_service_name != service.as_bytes() {
        return;
    }

    runtime().record_service_lifecycle(service, ServiceLifecycleState::Ready, None);
}

fn ready_service_name(message: &IpcMessage) -> Option<&[u8]> {
    if message.len < READY_ENVELOPE_LEN {
        return None;
    }

    let protocol = u16::from_le_bytes([message.bytes[0], message.bytes[1]]);
    let message_type = u16::from_le_bytes([message.bytes[2], message.bytes[3]]);
    if protocol != PROTOCOL_HEALTH_V0 || message_type != MESSAGE_READY {
        return None;
    }

    let payload_len = u32::from_le_bytes([
        message.bytes[4],
        message.bytes[5],
        message.bytes[6],
        message.bytes[7],
    ]) as usize;
    if payload_len > message.len - READY_ENVELOPE_LEN {
        return None;
    }

    Some(&message.bytes[READY_ENVELOPE_LEN..READY_ENVELOPE_LEN + payload_len])
}

pub fn read_boot_module(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    if max_len > MAX_BOOT_READ_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let module = boot_module_from_cap(cap_slot, capability::RIGHT_READ)?;
    let Ok(module_len) = usize::try_from(module.length) else {
        return Err(IpcError::MessageTooLarge);
    };
    if module_len > max_len {
        return Err(IpcError::MessageTooLarge);
    }
    let copy_len = module_len;

    let bytes = unsafe { core::slice::from_raw_parts(module.base as *const u8, copy_len) };
    usercopy::copy_to_user(UserPtr::new(destination as u64), &bytes[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Boot module read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" module=");
    serial::write_str(module.name);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    Ok(copy_len)
}

pub fn log_write(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }

    let _endpoint = serial_log_endpoint_from_cap(cap_slot, capability::RIGHT_SEND)?;
    let mut message = [0u8; MAX_MESSAGE_BYTES];
    usercopy::copy_from_user(&mut message, UserPtr::new(source as u64), len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    if !serial::interactive_quiet() {
        serial::write_ascii_bytes(&message[..len]);
        serial::write_str("\n");
    }
    wake_blocked_vfs_pipe_read(&message[..len]);
    Ok(())
}

pub fn endpoint_create(control_slot: u64, cap_slot: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_ALLOCATE)?;
    let process_name = current_process_name();
    let runtime = runtime();
    let (owner, quota, cap_slot_available) = {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        (process.pid, process.quota, process.caps.can_grant(cap_slot))
    };
    if quota.used_endpoints >= quota.max_endpoints {
        serial::write_str("Endpoint create rejected: proc=");
        serial::write_str(process_name);
        serial::write_str(" quota=max_endpoints\n");
        return Err(IpcError::BadCapability);
    }
    if !cap_slot_available {
        serial::write_str("Endpoint create rejected: proc=");
        serial::write_str(process_name);
        serial::write_str(" target cap slot unavailable\n");
        return Err(IpcError::BadCapability);
    }
    if !runtime.can_allocate_capability() {
        serial::write_str("Endpoint create rejected: cap lineage full\n");
        return Err(IpcError::BadCapability);
    }

    let endpoint_id = runtime
        .objects
        .add_endpoint_owned("dynamic-endpoint", owner)
        .map_err(|_| {
            serial::write_str("Endpoint create rejected: object arena full\n");
            IpcError::BadCapability
        })?;
    let cap = match runtime.new_capability(
        endpoint_id,
        capability::RIGHT_SEND | capability::RIGHT_RECEIVE,
        owner,
        0,
        owner,
    ) {
        Ok(cap) => cap,
        Err(error) => {
            let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
            return Err(error);
        }
    };
    let quota_after = {
        let Some(process) = runtime.processes.current_process_mut() else {
            runtime.rollback_last_capability(cap);
            let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
            return Err(IpcError::BadCapability);
        };
        if process.caps.grant(cap_slot, cap).is_err() {
            None
        } else {
            process.quota.used_endpoints = process.quota.used_endpoints.saturating_add(1);
            Some((process.quota.used_endpoints, process.quota.max_endpoints))
        }
    };
    let Some((used_endpoints, max_endpoints)) = quota_after else {
        runtime.rollback_last_capability(cap);
        let _ = runtime.objects.remove_owned_endpoint(endpoint_id, owner);
        return Err(IpcError::BadCapability);
    };

    serial::write_str("Endpoint create accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(cap_slot);
    serial::write_str(" endpoint_id=");
    serial::write_u64_dec(endpoint_id.raw());
    serial::write_str(" quota=");
    serial::write_u64_dec(used_endpoints);
    serial::write_str("/");
    serial::write_u64_dec(max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn quota_delegate(
    control_slot: u64,
    target_pid: u64,
    max_endpoints: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_DELEGATE)?;
    let target_pid = ProcessId::new(target_pid);
    let runtime = runtime();
    let (caller_name, caller_quota) = runtime
        .processes
        .current_process()
        .map(|process| (process.name, process.quota))
        .ok_or(IpcError::BadCapability)?;
    if max_endpoints > caller_quota.max_endpoints {
        serial::write_str("Quota delegate rejected: requested exceeds parent quota\n");
        return Err(IpcError::BadCapability);
    }
    let Some(target) = runtime.processes.process_mut(target_pid) else {
        return Err(IpcError::BadCapability);
    };
    let persist_for_restart = target.state == ProcessState::Declared;
    target.quota.max_endpoints = max_endpoints;
    if persist_for_restart {
        target.quota.used_endpoints = 0;
        target.initial_quota.max_endpoints = max_endpoints;
        target.initial_quota.used_endpoints = 0;
    }

    serial::write_str("Quota delegate accepted: proc=");
    serial::write_str(caller_name);
    serial::write_str(" target=");
    serial::write_str(target.name);
    serial::write_str(" target_pid=");
    serial::write_u64_dec(target_pid.raw());
    serial::write_str(" max_endpoints=");
    serial::write_u64_dec(max_endpoints);
    serial::write_str("\n");
    Ok(())
}

pub fn secret_read(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    let secret = secret_from_cap(cap_slot, capability::RIGHT_READ)?;
    let copy_len = min(secret.value.len(), max_len);
    usercopy::copy_to_user(UserPtr::new(destination as u64), &secret.value[..copy_len])
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Secret read accepted: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" secret=");
    serial::write_str(secret.name);
    serial::write_str(" bytes=<redacted>\n");
    serial::write_str("vertex-inspect security event: secret metadata access secret=");
    serial::write_str(secret.name);
    serial::write_str(" proc=");
    serial::write_str(current_process_name());
    serial::write_str("\n");
    Ok(copy_len)
}

pub fn namespace_resolve(
    cap_slot: u64,
    path: *const u8,
    path_len: usize,
    target_slot: u64,
) -> Result<(), IpcError> {
    if path_len > 128 {
        return Err(IpcError::MessageTooLarge);
    }
    let namespace = namespace_from_cap(cap_slot, capability::RIGHT_RESOLVE)?;
    let mut path_bytes = [0u8; 128];
    usercopy::copy_from_user(&mut path_bytes, UserPtr::new(path as u64), path_len)
        .map_err(|_| IpcError::InvalidUserBuffer)?;
    let Some(entry) = namespace.resolve(&path_bytes[..path_len]) else {
        serial::write_str("Namespace resolve rejected: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" namespace=");
        serial::write_str(namespace.name);
        serial::write_str(" path=");
        serial::write_ascii_bytes(&path_bytes[..path_len]);
        serial::write_str("\n");
        return Err(IpcError::BadCapability);
    };

    let namespace_cap = lookup_capability(cap_slot, capability::RIGHT_RESOLVE)?;
    let runtime = runtime();
    let owner = runtime
        .processes
        .current_process()
        .map(|process| process.pid)
        .ok_or(IpcError::BadCapability)?;
    {
        let Some(process) = runtime.processes.current_process() else {
            return Err(IpcError::BadCapability);
        };
        if !process.caps.can_grant(target_slot) {
            return Err(IpcError::BadCapability);
        }
    }
    let cap = runtime.new_capability(entry.object, entry.rights, owner, namespace_cap.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Namespace resolve accepted: proc=");
    serial::write_str(process.name);
    serial::write_str(" namespace=");
    serial::write_str(namespace.name);
    serial::write_str(" path=");
    serial::write_ascii_bytes(&path_bytes[..path_len]);
    serial::write_str(" target_cap[");
    serial::write_u64_dec(target_slot);
    serial::write_str("] rights=");
    print_rights(entry.rights);
    serial::write_str("\n");
    Ok(())
}

pub fn runtime_inspect(
    cap_slot: u64,
    destination: *mut u8,
    max_len: usize,
) -> Result<usize, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_INSPECT)?;
    let caller = current_process_name();
    let report = inspect_report();
    report.clear();

    {
        let runtime = runtime();
        build_inspect_report(runtime, report);
    }

    if report.is_truncated() || report.len() > max_len {
        return Err(IpcError::MessageTooLarge);
    }

    usercopy::copy_to_user(UserPtr::new(destination as u64), report.as_slice())
        .map_err(|_| IpcError::InvalidUserBuffer)?;

    serial::write_str("Runtime inspect accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" bytes=");
    serial::write_u64_dec(report.len() as u64);
    serial::write_str("\n");
    Ok(report.len())
}
