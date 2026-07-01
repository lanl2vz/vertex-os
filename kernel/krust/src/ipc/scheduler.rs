use core::arch::{asm, x86_64::__cpuid_count};

use super::*;

pub fn sleep_ms(
    cap_slot: u64,
    milliseconds: u64,
    frame: &mut SyscallFrame,
) -> Result<(), IpcError> {
    let timer = timer_from_cap(cap_slot, capability::RIGHT_CONTROL)?;
    if serial::trace_enabled() {
        serial::write_str("Timer sleep accepted: proc=");
        serial::write_str(current_process_name());
        serial::write_str(" timer=");
        serial::write_str(timer.name);
        serial::write_str(" ms=");
        serial::write_u64_dec(milliseconds);
        serial::write_str("\n");
    }

    if milliseconds == 0 {
        frame.rax = STATUS_OK;
        return Ok(());
    }

    let wake_tsc = deadline_after_ms(milliseconds);
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return Err(IpcError::BadCapability);
        };

        process.saved_frame = *frame;
        process.saved_frame.rax = STATUS_OK;
        process.has_saved_frame = true;
        process.state = ProcessState::Sleeping { wake_tsc };
        process.name
    };

    if serial::trace_enabled() {
        serial::write_str("Timer sleep blocked: proc=");
        serial::write_str(current);
        serial::write_str("\n");
    }

    if schedule_next_ready(frame) {
        return Ok(());
    }

    let runtime = runtime();
    if let Some(process) = runtime.processes.current_process_mut() {
        process.state = ProcessState::Running;
    }

    Err(IpcError::Empty)
}

pub(super) fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | low as u64
}

pub(super) fn deadline_after_ms(milliseconds: u64) -> u64 {
    read_tsc().saturating_add(milliseconds.saturating_mul(tsc_ticks_per_ms()))
}

fn tsc_ticks_per_ms() -> u64 {
    let leaf15 = __cpuid_count(0x15, 0);
    if leaf15.eax != 0 && leaf15.ebx != 0 && leaf15.ecx != 0 {
        let hz = (leaf15.ecx as u64)
            .saturating_mul(leaf15.ebx as u64)
            .saturating_div(leaf15.eax as u64);
        if hz != 0 {
            return hz / 1_000;
        }
    }

    let leaf16 = __cpuid_count(0x16, 0);
    if leaf16.eax != 0 {
        return (leaf16.eax as u64).saturating_mul(1_000);
    }

    FALLBACK_TSC_TICKS_PER_MS
}

pub(super) fn block_current_on_endpoint(
    endpoint: KernelObjectId,
    cap_id: u64,
    destination: u64,
    max_len: usize,
    timeout_tsc: Option<u64>,
    frame: &mut SyscallFrame,
) -> bool {
    let current = {
        let runtime = runtime();
        let Some(process) = runtime.processes.current_process_mut() else {
            return false;
        };

        process.saved_frame = *frame;
        process.has_saved_frame = true;
        process.state = ProcessState::BlockedOnEndpoint {
            endpoint,
            cap_id,
            destination,
            max_len,
            timeout_tsc,
        };
        process.name
    };

    if serial::trace_enabled() {
        serial::write_str("IPC receive blocked: proc=");
        serial::write_str(current);
        serial::write_str(" endpoint=");
        serial::write_u64_dec(endpoint.raw());
        if timeout_tsc.is_some() {
            serial::write_str(" timeout=yes");
        }
        serial::write_str("\n");
    }

    if schedule_next_ready(frame) {
        true
    } else {
        let runtime = runtime();
        if let Some(process) = runtime.processes.current_process_mut() {
            process.state = ProcessState::Running;
        }

        if serial::trace_enabled() {
            serial::write_str("Scheduler blocked: proc=");
            serial::write_str(current);
            serial::write_str(" no ready process\n");
        }
        false
    }
}

pub(super) fn wake_blocked_receiver(endpoint: KernelObjectId) {
    wake_timed_processes(read_tsc());

    loop {
        let Some(waiter_index) = blocked_receiver_index(endpoint) else {
            return;
        };

        let should_cancel = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index] else {
                return;
            };
            let ProcessState::BlockedOnEndpoint {
                endpoint, cap_id, ..
            } = waiter.state
            else {
                return;
            };
            !process_has_live_endpoint_cap(
                runtime,
                waiter,
                endpoint,
                cap_id,
                capability::RIGHT_RECEIVE,
            )
        };
        if should_cancel {
            let runtime = runtime();
            let _ = cancel_blocked_endpoint_waiter_at(
                runtime,
                waiter_index,
                STATUS_BAD_CAPABILITY,
                "authority-revoked",
            );
            continue;
        }

        let (name, receiver_pid, receiver_cr3, destination, max_len, current_cr3) = {
            let runtime = runtime();
            let Some(waiter) = runtime.processes.processes[waiter_index] else {
                return;
            };
            let ProcessState::BlockedOnEndpoint {
                destination,
                max_len,
                ..
            } = waiter.state
            else {
                return;
            };

            let current_cr3 = runtime
                .processes
                .current_process()
                .map(|process| process.context.cr3)
                .unwrap_or_else(paging::active_root_table_physical);

            (
                waiter.name,
                waiter.pid,
                waiter.context.cr3,
                destination,
                max_len,
                current_cr3,
            )
        };

        let Some(message) = ({
            let runtime = runtime();
            let Some(endpoint_object) = runtime.objects.get_endpoint_mut(endpoint) else {
                return;
            };
            endpoint_object.dequeue_for(receiver_pid)
        }) else {
            return;
        };

        let copy_len = min(message.len, max_len);
        let copy_result = unsafe {
            gdt::switch_address_space(receiver_cr3);
            let result =
                usercopy::copy_to_user(UserPtr::new(destination), &message.bytes[..copy_len]);
            gdt::switch_address_space(current_cr3);
            result
        };

        match copy_result {
            Ok(()) => {
                {
                    let runtime = runtime();
                    let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                        return;
                    };
                    waiter.saved_frame.rax = copy_len as u64;
                    waiter.state = ProcessState::Ready;
                }
                record_ready_lifecycle(endpoint, receiver_pid, message);

                if serial::trace_enabled() {
                    serial::write_str("IPC receive delivered: endpoint=");
                    serial::write_u64_dec(endpoint.raw());
                    serial::write_str(" bytes=");
                    serial::write_u64_dec(copy_len as u64);
                    serial::write_str("\n");

                    serial::write_str("IPC wake receiver: proc=");
                    serial::write_str(name);
                    serial::write_str(" endpoint=");
                    serial::write_u64_dec(endpoint.raw());
                    serial::write_str("\n");
                }
            }
            Err(_) => {
                {
                    let runtime = runtime();
                    let Some(waiter) = runtime.processes.processes[waiter_index].as_mut() else {
                        return;
                    };
                    waiter.saved_frame.rax = STATUS_BAD_BUFFER;
                    waiter.state = ProcessState::Ready;
                }
                serial::write_str("IPC wake receiver failed: bad user buffer proc=");
                serial::write_str(name);
                serial::write_str("\n");
            }
        }
        return;
    }
}

fn blocked_receiver_index(endpoint: KernelObjectId) -> Option<usize> {
    let runtime = runtime();
    let mut index = 0;

    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index]
            && let ProcessState::BlockedOnEndpoint {
                endpoint: waiting_endpoint,
                ..
            } = process.state
            && waiting_endpoint == endpoint
            && runtime
                .objects
                .get_endpoint(endpoint)
                .map(|endpoint_object| endpoint_object.has_message_for(process.pid))
                .unwrap_or(false)
        {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn process_has_live_endpoint_cap(
    runtime: &RuntimeState,
    process: Process,
    endpoint: KernelObjectId,
    cap_id: u64,
    required_right: u64,
) -> bool {
    if runtime.objects.get_endpoint(endpoint).is_none() {
        return false;
    }

    let mut slot = 0;
    while slot < process.caps.caps.len() {
        if let Some(cap) = process.caps.caps[slot]
            && cap.id == cap_id
            && cap.object == endpoint
            && !cap.revoked
            && !runtime.cap_id_revoked(cap.id)
            && !capability_has_revoked_ancestor(runtime, cap)
            && cap.generation_id == runtime.generation_id
            && cap.rights & required_right == required_right
        {
            return true;
        }
        slot += 1;
    }
    false
}

fn cancel_blocked_endpoint_waiter_at(
    runtime: &mut RuntimeState,
    index: usize,
    status: u64,
    reason: &'static str,
) -> bool {
    let Some(process) = runtime.processes.processes[index].as_mut() else {
        return false;
    };
    let ProcessState::BlockedOnEndpoint {
        endpoint, cap_id, ..
    } = process.state
    else {
        return false;
    };

    process.saved_frame.rax = status;
    process.state = ProcessState::Ready;

    serial::write_str("IPC receive canceled: proc=");
    serial::write_str(process.name);
    serial::write_str(" endpoint=");
    serial::write_u64_dec(endpoint.raw());
    serial::write_str(" cap_id=");
    serial::write_u64_dec(cap_id);
    serial::write_str(" reason=");
    serial::write_str(reason);
    serial::write_str(" status=");
    serial::write_u64_dec(status);
    serial::write_str("\n");
    true
}

pub(super) fn cancel_unauthorized_blocked_receivers(status: u64) -> usize {
    let runtime = runtime();
    let mut canceled = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        let should_cancel = runtime.processes.processes[index]
            .map(|process| {
                if let ProcessState::BlockedOnEndpoint {
                    endpoint, cap_id, ..
                } = process.state
                {
                    !process_has_live_endpoint_cap(
                        runtime,
                        process,
                        endpoint,
                        cap_id,
                        capability::RIGHT_RECEIVE,
                    )
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_cancel
            && cancel_blocked_endpoint_waiter_at(runtime, index, status, "authority-revoked")
        {
            canceled += 1;
        }
        index += 1;
    }
    canceled
}

pub(super) fn cancel_blocked_receivers_for_endpoint_owner(owner: ProcessId, status: u64) -> usize {
    let runtime = runtime();
    let mut canceled = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        let should_cancel = runtime.processes.processes[index]
            .map(|process| {
                if let ProcessState::BlockedOnEndpoint { endpoint, .. } = process.state {
                    runtime
                        .objects
                        .get_endpoint(endpoint)
                        .map(|endpoint_object| endpoint_object.owner == owner)
                        .unwrap_or(true)
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_cancel
            && cancel_blocked_endpoint_waiter_at(runtime, index, status, "endpoint-destroyed")
        {
            canceled += 1;
        }
        index += 1;
    }
    canceled
}

pub(super) fn wake_timed_processes(now: u64) -> usize {
    let runtime = runtime();
    let mut woke = 0;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index].as_mut() {
            match process.state {
                ProcessState::Sleeping { wake_tsc } if deadline_reached(now, wake_tsc) => {
                    process.saved_frame.rax = STATUS_OK;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    if serial::trace_enabled() {
                        serial::write_str("Timer wake: proc=");
                        serial::write_str(process.name);
                        serial::write_str("\n");
                    }
                }
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } if deadline_reached(now, timeout_tsc) => {
                    process.saved_frame.rax = STATUS_TIMEOUT;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    if serial::trace_enabled() {
                        serial::write_str("IPC receive timeout: proc=");
                        serial::write_str(process.name);
                        serial::write_str("\n");
                    }
                }
                ProcessState::BlockedOnInterrupt {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } if deadline_reached(now, timeout_tsc) => {
                    process.saved_frame.rax = STATUS_TIMEOUT;
                    process.state = ProcessState::Ready;
                    woke += 1;
                    if serial::trace_enabled() {
                        serial::write_str("IRQ wait timeout: proc=");
                        serial::write_str(process.name);
                        serial::write_str("\n");
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    woke
}

fn next_deadline_tsc() -> Option<u64> {
    let runtime = runtime();
    let mut next = None;
    let mut index = 0;
    while index < runtime.processes.count {
        if let Some(process) = runtime.processes.processes[index] {
            let deadline = match process.state {
                ProcessState::Sleeping { wake_tsc } => Some(wake_tsc),
                ProcessState::BlockedOnEndpoint {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } => Some(timeout_tsc),
                ProcessState::BlockedOnInterrupt {
                    timeout_tsc: Some(timeout_tsc),
                    ..
                } => Some(timeout_tsc),
                _ => None,
            };
            if let Some(deadline) = deadline
                && next
                    .map(|current| deadline_before(deadline, current))
                    .unwrap_or(true)
            {
                next = Some(deadline);
            }
        }
        index += 1;
    }
    next
}

fn wait_until_deadline(deadline: u64, include_current: bool) {
    while !deadline_reached(read_tsc(), deadline)
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
        && wake_timed_processes(read_tsc()) == 0
    {
        crate::timer::wait_for_interrupt();
    }
}

fn deadline_reached(now: u64, deadline: u64) -> bool {
    (now as i64).wrapping_sub(deadline as i64) >= 0
}

fn deadline_before(left: u64, right: u64) -> bool {
    (left as i64).wrapping_sub(right as i64) < 0
}

pub(super) fn schedule_next_ready(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, true, true)
}

pub(super) fn schedule_next_ready_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, true)
}

pub(super) fn schedule_next_ready_no_wait_excluding_current(frame: &mut SyscallFrame) -> bool {
    schedule_next_ready_inner(frame, false, false)
}

fn schedule_next_ready_inner(
    frame: &mut SyscallFrame,
    include_current: bool,
    wait_for_deadline: bool,
) -> bool {
    wake_timed_processes(read_tsc());
    if wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
        && let Some(deadline) = next_deadline_tsc()
    {
        wait_until_deadline(deadline, include_current);
        wake_timed_processes(read_tsc());
    }

    if !wait_for_deadline
        && runtime()
            .processes
            .next_ready_index_round_robin(include_current)
            .is_none()
    {
        return false;
    }

    let (from, to, next_frame, next_cr3) = {
        let runtime = runtime();
        let from = runtime
            .processes
            .current_process()
            .map(|process| process.name)
            .unwrap_or("<none>");
        let Some(next_index) = runtime
            .processes
            .next_ready_index_round_robin(include_current)
        else {
            return false;
        };
        let (next_pid, to, next_frame, next_cr3) = {
            let Some(next) = runtime.processes.processes[next_index].as_mut() else {
                return false;
            };

            next.state = ProcessState::Running;

            let next_frame = if next.has_saved_frame {
                next.saved_frame
            } else {
                SyscallFrame::from_context(next.context)
            };

            (next.pid, next.name, next_frame, next.context.cr3)
        };

        runtime.processes.current = Some(next_pid);

        (from, to, next_frame, next_cr3)
    };

    *frame = next_frame;

    if serial::trace_enabled() {
        serial::write_str("Scheduler switch: from=");
        serial::write_str(from);
        serial::write_str(" to=");
        serial::write_str(to);
        serial::write_str("\n");
    }

    unsafe {
        gdt::switch_address_space(next_cr3);
    }

    true
}
