use core::{cell::UnsafeCell, ptr};

use crate::{capability, serial};

pub const ENDPOINT_CAP_SLOT: u64 = 0;
pub const BOOT_ENDPOINT_ID: u64 = 1;

const MAX_MESSAGE_BYTES: usize = 128;
const USER_CANONICAL_LIMIT: u64 = 0x0000_8000_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessId {
    Sender,
    Receiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    BadCapability,
    InvalidUserBuffer,
    MessageTooLarge,
    Empty,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

struct IpcState {
    current_process: ProcessId,
    message_ready: bool,
    message_len: usize,
    message: [u8; MAX_MESSAGE_BYTES],
}

impl IpcState {
    const fn new() -> Self {
        Self {
            current_process: ProcessId::Sender,
            message_ready: false,
            message_len: 0,
            message: [0; MAX_MESSAGE_BYTES],
        }
    }
}

static IPC_STATE: Global<IpcState> = Global(UnsafeCell::new(IpcState::new()));

pub fn reset_for_boot() {
    let state = state();
    *state = IpcState::new();
}

pub fn set_current_process(process: ProcessId) {
    state().current_process = process;
}

pub fn current_process() -> ProcessId {
    state().current_process
}

pub fn print_boot_capability_table() {
    serial::write_str("IPC boot capability table entries: 2\n");
    serial::write_str("  proc=ipc-sender cap[");
    serial::write_u64_dec(ENDPOINT_CAP_SLOT);
    serial::write_str("] endpoint=");
    serial::write_u64_dec(BOOT_ENDPOINT_ID);
    serial::write_str(" rights=send\n");
    serial::write_str("  proc=ipc-receiver cap[");
    serial::write_u64_dec(ENDPOINT_CAP_SLOT);
    serial::write_str("] endpoint=");
    serial::write_u64_dec(BOOT_ENDPOINT_ID);
    serial::write_str(" rights=receive\n");
}

pub fn send(cap_slot: u64, source: *const u8, len: usize) -> Result<(), IpcError> {
    if !has_endpoint_right(current_process(), cap_slot, capability::RIGHT_SEND) {
        return Err(IpcError::BadCapability);
    }
    if len > MAX_MESSAGE_BYTES {
        return Err(IpcError::MessageTooLarge);
    }
    if !valid_user_range(source as u64, len as u64) {
        return Err(IpcError::InvalidUserBuffer);
    }

    let state = state();
    unsafe {
        ptr::copy_nonoverlapping(source, state.message.as_mut_ptr(), len);
    }
    state.message_len = len;
    state.message_ready = true;

    serial::write_str("IPC send accepted: endpoint=");
    serial::write_u64_dec(BOOT_ENDPOINT_ID);
    serial::write_str(" bytes=");
    serial::write_u64_dec(len as u64);
    serial::write_str("\n");

    Ok(())
}

pub fn receive(cap_slot: u64, destination: *mut u8, max_len: usize) -> Result<usize, IpcError> {
    if !has_endpoint_right(current_process(), cap_slot, capability::RIGHT_RECEIVE) {
        return Err(IpcError::BadCapability);
    }
    if !valid_user_range(destination as u64, max_len as u64) {
        return Err(IpcError::InvalidUserBuffer);
    }

    let state = state();
    if !state.message_ready {
        return Err(IpcError::Empty);
    }

    let copy_len = min(state.message_len, max_len);
    unsafe {
        ptr::copy_nonoverlapping(state.message.as_ptr(), destination, copy_len);
    }
    state.message_ready = false;

    serial::write_str("IPC receive delivered: endpoint=");
    serial::write_u64_dec(BOOT_ENDPOINT_ID);
    serial::write_str(" bytes=");
    serial::write_u64_dec(copy_len as u64);
    serial::write_str("\n");

    Ok(copy_len)
}

fn has_endpoint_right(process: ProcessId, cap_slot: u64, right: u64) -> bool {
    match process {
        ProcessId::Sender => cap_slot == ENDPOINT_CAP_SLOT && right == capability::RIGHT_SEND,
        ProcessId::Receiver => cap_slot == ENDPOINT_CAP_SLOT && right == capability::RIGHT_RECEIVE,
    }
}

fn valid_user_range(base: u64, len: u64) -> bool {
    let Some(end) = base.checked_add(len) else {
        return false;
    };

    base < USER_CANONICAL_LIMIT && end <= USER_CANONICAL_LIMIT
}

fn state() -> &'static mut IpcState {
    unsafe { &mut *IPC_STATE.0.get() }
}

fn min(left: usize, right: usize) -> usize {
    if left < right { left } else { right }
}
