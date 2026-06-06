use super::*;

pub fn cap_derive(parent_slot: u64, new_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let parent = lookup_capability(parent_slot, 0)?;
    if rights_mask == 0 || rights_mask & !parent.rights != 0 {
        return Err(IpcError::BadCapability);
    }

    let process_name = current_process_name();
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
        if !process.caps.can_grant(new_slot) {
            return Err(IpcError::BadCapability);
        }
    }
    let cap = runtime.new_capability(parent.object, rights_mask, owner, parent.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(new_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability derive accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" parent=");
    serial::write_u64_dec(parent_slot);
    serial::write_str(" new=");
    serial::write_u64_dec(new_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_drop(slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let dropped = {
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::BadCapability);
        };
        process.caps.clear(slot)?
    };
    release_unreferenced_derived_vfs_root(runtime, dropped.object);

    serial::write_str("Capability drop accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_revoke(slot: u64) -> Result<(), IpcError> {
    let cap = lookup_capability(slot, 0)?;
    let process_name = current_process_name();
    {
        let runtime = runtime();
        runtime.revoke_cap_id(cap.id)?;
        release_unreferenced_derived_vfs_roots(runtime);
    }
    let canceled = cancel_unauthorized_blocked_receivers(STATUS_BAD_CAPABILITY);
    if canceled > 0 {
        serial::write_str("Capability revoke canceled blocked receives: count=");
        serial::write_u64_dec(canceled as u64);
        serial::write_str("\n");
    }

    serial::write_str("Capability revoke accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str("\n");
    Ok(())
}

pub(super) fn release_unreferenced_derived_vfs_root(
    runtime: &mut RuntimeState,
    object: KernelObjectId,
) {
    if object_reachable_by_cap(runtime, object) {
        return;
    }
    if runtime.objects.remove_derived_vfs_root(object) {
        log_derived_vfs_root_released(object);
    }
}

pub(super) fn release_unreferenced_derived_vfs_roots(runtime: &mut RuntimeState) {
    let mut index = 0;
    while index < runtime.objects.count {
        if let Some(KernelObject::VfsRoot(root)) = runtime.objects.objects[index]
            && root.derived
            && !object_reachable_by_cap(runtime, root.id)
        {
            let object = root.id;
            if runtime.objects.remove_derived_vfs_root(object) {
                log_derived_vfs_root_released(object);
                continue;
            }
        }
        index += 1;
    }
}

fn log_derived_vfs_root_released(object: KernelObjectId) {
    serial::write_str("Derived VFS root released: object=");
    serial::write_u64_dec(object.raw());
    serial::write_str("\n");
}

pub fn cap_inspect(slot: u64) -> Result<u64, IpcError> {
    let cap = lookup_capability(slot, 0)?;
    serial::write_str("Capability inspect: proc=");
    serial::write_str(current_process_name());
    serial::write_str(" slot=");
    serial::write_u64_dec(slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str(" object_id=");
    serial::write_u64_dec(cap.object.raw());
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(cap.parent_cap_id);
    serial::write_str(" owner_process=");
    serial::write_u64_dec(cap.owner_process.raw());
    serial::write_str(" delegated_by=");
    serial::write_u64_dec(cap.delegated_by.raw());
    serial::write_str(" generation=");
    serial::write_str(cap.generation_id);
    serial::write_str(" revoked=");
    serial::write_str(if cap.revoked { "yes" } else { "no" });
    serial::write_str("\n");
    Ok(cap.parent_cap_id)
}

pub fn cap_copy(source_slot: u64, target_slot: u64, rights_mask: u64) -> Result<(), IpcError> {
    let source = lookup_capability(source_slot, 0)?;
    if rights_mask == 0 || rights_mask & !source.rights != 0 {
        return Err(IpcError::BadCapability);
    }

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
    let copied = runtime.new_capability(source.object, rights_mask, owner, source.id, owner)?;
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    process
        .caps
        .grant(target_slot, copied)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability copy accepted: proc=");
    serial::write_str(process.name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(copied.id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(source.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> Result<(), IpcError> {
    let process_name = current_process_name();
    let runtime = runtime();
    let Some(process) = runtime.processes.current_process_mut() else {
        return Err(IpcError::BadCapability);
    };
    if !process.caps.can_grant(target_slot) {
        return Err(IpcError::BadCapability);
    }
    let cap = process.caps.clear(source_slot)?;
    process
        .caps
        .grant(target_slot, cap)
        .map_err(|_| IpcError::BadCapability)?;

    serial::write_str("Capability move accepted: proc=");
    serial::write_str(process_name);
    serial::write_str(" source=");
    serial::write_u64_dec(source_slot);
    serial::write_str(" target=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap.id);
    serial::write_str("\n");
    Ok(())
}

pub fn cap_transfer(
    control_slot: u64,
    target_pid: u64,
    packed_transfer: u64,
) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(control_slot, capability::RIGHT_DELEGATE)?;
    let cap_slot = packed_transfer & 0xffff;
    let target_slot = (packed_transfer >> 16) & 0xffff;
    let rights_mask = packed_transfer >> 32;
    let cap = lookup_capability(cap_slot, 0)?;
    if rights_mask == 0 || rights_mask & !cap.rights != 0 {
        return Err(IpcError::BadCapability);
    }
    let target_pid = ProcessId::new(target_pid);

    let (caller, target, transferred_id, parent_cap_id, target_pid_raw) = {
        let runtime = runtime();
        let caller = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let delegated_by = runtime
            .processes
            .current_process()
            .map(|process| process.pid)
            .ok_or(IpcError::BadCapability)?;
        let (target_name, persist_for_restart) = {
            let Some(target_process) = runtime.processes.process(target_pid) else {
                return Err(IpcError::BadCapability);
            };
            if !target_process.caps.can_grant(target_slot) {
                return Err(IpcError::BadCapability);
            }
            if target_process.state == ProcessState::Declared
                && !target_process.initial_caps.can_grant(target_slot)
            {
                return Err(IpcError::BadCapability);
            }
            (
                target_process.name,
                target_process.state == ProcessState::Declared,
            )
        };
        let transferred =
            runtime.new_capability(cap.object, rights_mask, target_pid, cap.id, delegated_by)?;
        let transferred_id = transferred.id;
        let Some(target_process) = runtime.processes.process_mut(target_pid) else {
            return Err(IpcError::BadCapability);
        };
        let mut next_caps = target_process.caps;
        let mut next_initial_caps = target_process.initial_caps;
        next_caps
            .grant(target_slot, transferred)
            .map_err(|_| IpcError::BadCapability)?;
        if persist_for_restart {
            next_initial_caps
                .grant(target_slot, transferred)
                .map_err(|_| IpcError::BadCapability)?;
        }
        target_process.caps = next_caps;
        if persist_for_restart {
            target_process.initial_caps = next_initial_caps;
        }
        (
            caller,
            target_name,
            transferred_id,
            cap.id,
            target_pid.raw(),
        )
    };

    serial::write_str("Capability transfer accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" target_pid=");
    serial::write_u64_dec(target_pid_raw);
    serial::write_str(" slot=");
    serial::write_u64_dec(target_slot);
    serial::write_str(" rights=");
    print_rights(rights_mask);
    serial::write_str(" cap_id=");
    serial::write_u64_dec(transferred_id);
    serial::write_str(" parent_cap_id=");
    serial::write_u64_dec(parent_cap_id);
    serial::write_str("\n");
    Ok(())
}
