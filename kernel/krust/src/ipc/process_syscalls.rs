use super::vfs_transactions::{abort_vertexfs_sync_transactions, abort_vfs_state_transactions};
use super::*;

pub fn exit_current_process(status: u64, frame: &mut SyscallFrame) -> ScheduleResult {
    let initial_exited = {
        let runtime = runtime();
        runtime
            .processes
            .current_process()
            .map(|process| process.pid.raw() == 1)
            .unwrap_or(true)
    };

    let (lifecycle_event, exiting_pid, exiting_name) = {
        let runtime = runtime();

        if let Some(process) = runtime.processes.current_process_mut() {
            let pid = process.pid;
            let name = process.name;
            let event = if process.pid.raw() == 1 {
                None
            } else {
                let lifecycle_state = if status == 0 {
                    ServiceLifecycleState::Exited
                } else {
                    ServiceLifecycleState::Failed
                };
                Some((process.name, lifecycle_state))
            };
            process.state = ProcessState::Exited;
            process.has_saved_frame = false;
            process.exit_status = status;
            process.has_exited = true;
            process.clear_file_handles();
            runtime.release_process_file_descriptions(pid);
            (event, Some(pid), Some(name))
        } else {
            (None, None, None)
        }
    };
    if let Some(pid) = exiting_pid {
        let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    }
    release_unreferenced_derived_vfs_roots(runtime());
    if let Some((service, lifecycle_state)) = lifecycle_event {
        runtime().record_service_lifecycle(service, lifecycle_state, Some(status));
    }
    if exiting_name == Some(VERTEX_STATE_PROCESS_NAME) {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if exiting_name == Some(BLOCK_DRIVER_PROCESS_NAME) {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    if initial_exited && status != 0 {
        return ScheduleResult::Halt { ok: false };
    }

    if schedule_next_ready(frame) {
        if let Some(pid) = exiting_pid {
            let _ = reap_process_context(pid);
        }
        ScheduleResult::Switched
    } else {
        let ok = runtime().processes.all_exited_successfully();
        if ok {
            let generation_id = runtime().generation_id;
            boot_manager().mark_known_good(generation_id);
        }
        ScheduleResult::Halt { ok }
    }
}

pub fn yield_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready_excluding_current(frame) {
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        serial::write_str("Scheduler yield: proc=");
        serial::write_str(current);
        serial::write_str(" no other ready process\n");
        ScheduleResult::Continue
    }
}

pub fn preempt_current_process(frame: &mut SyscallFrame) -> ScheduleResult {
    wake_timed_processes(read_tsc());
    let current = {
        let runtime = runtime();
        if runtime
            .processes
            .next_ready_index_round_robin(false)
            .is_none()
        {
            return ScheduleResult::Continue;
        }
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        if process.state != ProcessState::Running {
            return ScheduleResult::Continue;
        }

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::Ready;
        process.name
    };

    if schedule_next_ready_no_wait_excluding_current(frame) {
        serial::write_str("Scheduler preempted process without explicit yield: from=");
        serial::write_str(current);
        serial::write_str(" to=");
        serial::write_str(current_process_name());
        serial::write_str("\n");
        ScheduleResult::Switched
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }
        ScheduleResult::Continue
    }
}

pub fn wake_timed_from_interrupt() {
    wake_timed_processes(read_tsc());
}

pub fn fault_current_process(
    reason: &str,
    address: u64,
    error_code: u64,
    frame: &mut SyscallFrame,
) -> ScheduleResult {
    let (name, initial_faulted, faulted_pid) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return ScheduleResult::Halt { ok: false };
        };
        let initial = process.pid.raw() == 1;
        let name = process.name;
        let pid = process.pid;
        process.state = ProcessState::Exited;
        process.has_saved_frame = false;
        process.exit_status = STATUS_PROCESS_FAULT;
        process.has_exited = true;
        process.clear_file_handles();
        runtime.release_process_file_descriptions(pid);
        (name, initial, pid)
    };
    let _ = cancel_blocked_receivers_for_endpoint_owner(faulted_pid, STATUS_BAD_CAPABILITY);
    release_unreferenced_derived_vfs_roots(runtime());
    if !initial_faulted {
        runtime().record_service_lifecycle(
            name,
            ServiceLifecycleState::Failed,
            Some(STATUS_PROCESS_FAULT),
        );
    }
    if name == VERTEX_STATE_PROCESS_NAME {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if name == BLOCK_DRIVER_PROCESS_NAME {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    serial::write_str("User process fault contained: proc=");
    serial::write_str(name);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str(" address=");
    serial::write_u64_hex(address);
    serial::write_str(" error=");
    serial::write_u64_hex(error_code);
    serial::write_str("\n");

    if initial_faulted {
        return ScheduleResult::Halt { ok: false };
    }

    if schedule_next_ready(frame) {
        let _ = reap_process_context(faulted_pid);
        ScheduleResult::Switched
    } else {
        ScheduleResult::Halt {
            ok: runtime().processes.all_exited_successfully(),
        }
    }
}

fn recycle_exited_process_template(config_process_index: usize) -> Result<(), IpcError> {
    let existing = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if config_process_index >= config.process_count {
            return Err(IpcError::BadCapability);
        }
        runtime.process_template_pids[config_process_index]
    };

    let Some(pid) = existing else {
        return Ok(());
    };

    let state = {
        let runtime = runtime();
        runtime
            .processes
            .process(pid)
            .map(|process| process.state)
            .ok_or(IpcError::BadCapability)?
    };
    if state != ProcessState::Exited {
        return Err(IpcError::BadCapability);
    }

    reap_process_context(pid)?;
    let runtime = runtime();
    runtime
        .processes
        .remove_process(pid)
        .map_err(|_| IpcError::BadCapability)?;
    runtime.process_template_pids[config_process_index] = None;

    serial::write_str("Krust process table slot recycled: pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" template=");
    serial::write_u64_dec(config_process_index as u64);
    serial::write_str("\n");
    Ok(())
}

pub(super) fn process_config_for_pid(
    runtime: &RuntimeState,
    pid: ProcessId,
) -> Option<BootProcessConfig> {
    let config = runtime.active_config?;
    let mut index = 0;
    while index < config.process_count {
        if runtime.process_template_pids[index] == Some(pid) {
            return config.processes[index];
        }
        index += 1;
    }
    None
}

pub(super) fn load_process_context(
    name: &'static str,
    image_base: u64,
    image_length: u64,
) -> Result<ProcessContext, IpcError> {
    if image_base == 0 || image_length == 0 {
        return Err(IpcError::BadCapability);
    }
    let len = usize::try_from(image_length).map_err(|_| IpcError::BadCapability)?;
    let bytes = unsafe { core::slice::from_raw_parts(image_base as *const u8, len) };
    let hhdm_offset = limine::hhdm_offset().ok_or(IpcError::BadCapability)?;
    match userspace::load(bytes, hhdm_offset, frame_allocator()?) {
        Ok(image) => {
            serial::write_str("Krust process image loaded from native store: process=");
            serial::write_str(name);
            serial::write_str(" entry=");
            serial::write_u64_hex(image.entry);
            serial::write_str(" stack=");
            serial::write_u64_hex(image.stack_top);
            serial::write_str(" cr3=");
            serial::write_u64_hex(image.cr3);
            serial::write_str("\n");
            Ok(ProcessContext {
                cr3: image.cr3,
                entry: image.entry,
                stack_top: image.stack_top,
            })
        }
        Err(error) => {
            userspace::print_load_error(error);
            Err(IpcError::BadCapability)
        }
    }
}

pub(super) fn reclaim_detached_address_space(name: &'static str, cr3: u64) {
    if cr3 == 0 {
        return;
    }
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        return;
    };
    let Ok(allocator) = frame_allocator() else {
        return;
    };
    if let Ok(stats) = paging::reclaim_user_address_space(hhdm_offset, cr3, allocator) {
        serial::write_str("Krust detached address space reaped: proc=");
        serial::write_str(name);
        serial::write_str(" user_frames=");
        serial::write_u64_dec(stats.user_leaf_frames);
        serial::write_str(" page_tables=");
        serial::write_u64_dec(stats.page_table_frames);
        serial::write_str(" device_mappings=");
        serial::write_u64_dec(stats.device_mappings);
        serial::write_str("\n");
    }
}

pub fn create_process(cap_slot: u64, config_process_index: u64) -> Result<u64, IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_CREATE)?;
    let caller = current_process_name();
    let Ok(config_process_index) = usize::try_from(config_process_index) else {
        return Err(IpcError::BadCapability);
    };

    recycle_exited_process_template(config_process_index)?;

    let process = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if config_process_index >= config.process_count {
            return Err(IpcError::BadCapability);
        }
        if runtime.process_template_pids[config_process_index].is_some() {
            return Err(IpcError::BadCapability);
        }
        let process = config.processes[config_process_index].ok_or(IpcError::BadCapability)?;
        if process.initial {
            return Err(IpcError::BadCapability);
        };
        validate_config_caps_for_process(runtime, config, config_process_index)
            .map_err(|_| IpcError::BadCapability)?;
        process
    };
    let context = load_process_context(process.name, process.image_base, process.image_length)?;

    let (pid, name) = {
        let runtime = runtime();
        let config = runtime.active_config.ok_or(IpcError::BadCapability)?;
        if runtime.process_template_pids[config_process_index].is_some() {
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        let mount_root = VfsPath::from_boot_root_path(process.mount_root)
            .map_err(|_| IpcError::BadCapability)?;
        let pid = runtime
            .processes
            .add_process(
                process.name,
                context,
                process.image_base,
                process.image_length,
                ProcessState::Declared,
                CapabilitySpace::new(),
                mount_root,
            )
            .map_err(|_| {
                reclaim_detached_address_space(process.name, context.cr3);
                IpcError::BadCapability
            })?;
        if install_declared_process_mounts(runtime, process, pid, mount_root).is_err() {
            let _ = runtime.remove_owned_declared_bind_mounts(pid);
            let _ = runtime.processes.remove_last_process(pid);
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        if grant_config_caps_to_process(runtime, config, config_process_index, pid).is_err() {
            let _ = runtime.remove_owned_declared_bind_mounts(pid);
            let _ = runtime.processes.remove_last_process(pid);
            reclaim_detached_address_space(process.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
        runtime.process_template_pids[config_process_index] = Some(pid);
        runtime.record_service_lifecycle(process.name, ServiceLifecycleState::Declared, None);
        print_process_by_pid(runtime, pid);
        serial::write_str("initial capability grants supplied explicitly: process=");
        serial::write_str(process.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str("\n");

        (pid, process.name)
    };

    serial::write_str("Krust process create accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(name);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str(" template=");
    serial::write_u64_dec(config_process_index as u64);
    serial::write_str("\n");
    serial::write_str("immutable launch object accepted: process=");
    serial::write_str(name);
    serial::write_str(" args-env-hash=blake3:metadata-v0\n");
    Ok(pid.raw())
}

pub fn start_process(cap_slot: u64, pid: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_START)?;
    let caller = current_process_name();
    let pid = ProcessId::new(pid);
    let process_snapshot = {
        let runtime = runtime();
        runtime
            .processes
            .process(pid)
            .copied()
            .ok_or(IpcError::BadCapability)?
    };
    if process_snapshot.state != ProcessState::Declared
        && process_snapshot.state != ProcessState::Exited
    {
        return Err(IpcError::BadCapability);
    }
    let reload_context = if process_snapshot.state == ProcessState::Exited {
        reap_process_context(pid)?;
        Some(load_process_context(
            process_snapshot.name,
            process_snapshot.image_base,
            process_snapshot.image_length,
        )?)
    } else {
        None
    };
    if let Some(context) = reload_context {
        let process_config = {
            let runtime = runtime();
            process_config_for_pid(runtime, pid).ok_or(IpcError::BadCapability)?
        };
        if install_declared_process_mounts(
            runtime(),
            process_config,
            pid,
            process_snapshot.mount_root,
        )
        .is_err()
        {
            reclaim_detached_address_space(process_snapshot.name, context.cr3);
            return Err(IpcError::BadCapability);
        }
    }

    let (target, lifecycle_state, release_files) = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process_mut(pid) else {
            return Err(IpcError::BadCapability);
        };

        let lifecycle_state = if let Some(context) = reload_context {
            process.context = context;
            process.context_reaped = false;
            process.caps = process.initial_caps;
            process.quota = process.initial_quota;
            process.clear_dma_mappings();
            process.clear_file_handles();
            serial::write_str("Krust process restart reload: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
            serial::write_str("Krust process restart restores quota baseline: proc=");
            serial::write_str(process.name);
            serial::write_str("\n");
            ServiceLifecycleState::Restarting
        } else {
            ServiceLifecycleState::Starting
        };

        process.state = ProcessState::Ready;
        process.has_saved_frame = false;
        process.exit_status = 0;
        process.has_exited = false;
        process.start_count = process.start_count.saturating_add(1);
        (process.name, lifecycle_state, reload_context.is_some())
    };
    if release_files {
        runtime().release_process_file_descriptions(pid);
    }
    release_unreferenced_derived_vfs_roots(runtime());
    runtime().record_service_lifecycle(target, lifecycle_state, None);

    serial::write_str("Krust process start accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str("\n");
    Ok(())
}

pub fn process_attempt() -> Result<u64, IpcError> {
    let runtime = runtime();
    runtime
        .processes
        .current_process()
        .map(|process| process.start_count)
        .ok_or(IpcError::BadCapability)
}

pub fn process_wait(cap_slot: u64, pid: u64) -> Result<u64, IpcError> {
    wake_timed_processes(read_tsc());
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_WAIT)?;
    let pid = ProcessId::new(pid);

    let process = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process(pid).copied() else {
            return Err(IpcError::BadCapability);
        };
        process
    };

    if process.state == ProcessState::Exited {
        serial::write_str("Krust process wait observed exit: proc=");
        serial::write_str(process.name);
        serial::write_str(" pid=");
        serial::write_u64_dec(pid.raw());
        serial::write_str(" status=");
        serial::write_u64_dec(process.exit_status);
        serial::write_str("\n");
        reap_process_context(pid)?;
        Ok(process.exit_status)
    } else {
        Ok(u64::MAX - 8)
    }
}

pub fn kill_process(cap_slot: u64, pid: u64) -> Result<(), IpcError> {
    let _process_control = process_control_from_cap(cap_slot, capability::RIGHT_KILL)?;
    let pid = ProcessId::new(pid);
    let caller = current_process_name();
    if runtime()
        .processes
        .current_process()
        .map(|process| process.pid == pid)
        .unwrap_or(false)
    {
        return Err(IpcError::BadCapability);
    }
    let target = {
        let runtime = runtime();
        let Some(process) = runtime.processes.process_mut(pid) else {
            return Err(IpcError::BadCapability);
        };
        if process.pid.raw() == 1 {
            return Err(IpcError::BadCapability);
        }
        process.state = ProcessState::Exited;
        process.has_saved_frame = false;
        process.exit_status = u64::MAX - 11;
        process.has_exited = true;
        process.clear_file_handles();
        process.name
    };
    runtime().release_process_file_descriptions(pid);
    let _ = cancel_blocked_receivers_for_endpoint_owner(pid, STATUS_BAD_CAPABILITY);
    release_unreferenced_derived_vfs_roots(runtime());
    if target == VERTEX_STATE_PROCESS_NAME {
        abort_vfs_state_transactions(STATUS_VFS_UNSUPPORTED);
    }
    if target == BLOCK_DRIVER_PROCESS_NAME {
        abort_vertexfs_sync_transactions(STATUS_VFS_UNSUPPORTED);
    }

    serial::write_str("Krust process kill accepted: proc=");
    serial::write_str(caller);
    serial::write_str(" target=");
    serial::write_str(target);
    serial::write_str(" pid=");
    serial::write_u64_dec(pid.raw());
    serial::write_str("\n");
    reap_process_context(pid)?;
    Ok(())
}
