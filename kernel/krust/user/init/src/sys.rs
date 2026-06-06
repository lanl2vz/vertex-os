use core::arch::asm;

pub const CAP_MANIFEST: u64 = 0;
pub const CAP_LOG: u64 = 1;
pub const CAP_PROCESS_CONTROL: u64 = 2;
pub const CAP_READINESS: u64 = 3;
pub const CAP_ENDPOINT_AUTH_BASE: u64 = 4;
pub const CAP_CREATED_ENDPOINT: u64 = 29;
pub const CAP_TIMER: u64 = 30;
pub const CAP_DERIVED: u64 = 31;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const STATUS_TOO_LARGE: u64 = u64::MAX - 3;
pub const STATUS_TIMEOUT: u64 = u64::MAX - 9;
pub const RIGHT_SEND: u64 = 1 << 4;
pub const RIGHT_RECEIVE: u64 = 1 << 5;
pub const RIGHT_CONTROL: u64 = 1 << 6;
pub const RIGHT_ALLOCATE: u64 = 1 << 7;
pub const RIGHT_REVOKE: u64 = 1 << 13;
pub const RIGHT_INSPECT: u64 = 1 << 14;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_YIELD: u64 = 5;
const SYS_BOOT_READ: u64 = 6;
const SYS_LOG_WRITE: u64 = 7;
const SYS_ACTIVATE_GENERATION: u64 = 8;
const SYS_PROCESS_CREATE: u64 = 9;
const SYS_CAP_DERIVE: u64 = 10;
const SYS_CAP_DROP: u64 = 11;
const SYS_CAP_TRANSFER: u64 = 12;
const SYS_SLEEP_MS: u64 = 16;
const SYS_PROCESS_WAIT: u64 = 17;
const SYS_ROLLBACK_GENERATION: u64 = 18;
const SYS_IPC_RECV_TIMEOUT: u64 = 19;
const SYS_CAP_INSPECT: u64 = 22;
const SYS_CAP_MOVE: u64 = 23;
const SYS_ENDPOINT_CREATE: u64 = 25;
const SYS_QUOTA_DELEGATE: u64 = 26;
const SYS_RUNTIME_INSPECT: u64 = 31;
const SYS_PROCESS_START: u64 = 37;
const SYS_PROCESS_KILL: u64 = 38;

pub fn read_manifest(buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_BOOT_READ,
        CAP_MANIFEST,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn read_manifest_raw(destination: u64, len: u64) -> u64 {
    syscall3(SYS_BOOT_READ, CAP_MANIFEST, destination, len)
}

pub fn log(message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        CAP_LOG,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn ipc_recv(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_IPC_RECV,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn process_create(process_index: u64) -> u64 {
    syscall3(SYS_PROCESS_CREATE, CAP_PROCESS_CONTROL, process_index, 0)
}

pub fn process_create_with_cap(cap_slot: u64, process_index: u64) -> u64 {
    syscall3(SYS_PROCESS_CREATE, cap_slot, process_index, 0)
}

pub fn process_start(pid: u64) -> u64 {
    syscall3(SYS_PROCESS_START, CAP_PROCESS_CONTROL, pid, 0)
}

pub fn process_wait(pid: u64) -> u64 {
    syscall3(SYS_PROCESS_WAIT, CAP_PROCESS_CONTROL, pid, 0)
}

pub fn process_kill(pid: u64) -> u64 {
    syscall3(SYS_PROCESS_KILL, CAP_PROCESS_CONTROL, pid, 0)
}

pub fn sleep_ms(milliseconds: u64) -> u64 {
    syscall3(SYS_SLEEP_MS, CAP_TIMER, milliseconds, 0)
}

pub fn runtime_inspect(buffer: &mut [u8]) -> u64 {
    runtime_inspect_with_cap(CAP_PROCESS_CONTROL, buffer)
}

pub fn runtime_inspect_with_cap(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_RUNTIME_INSPECT,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn rollback_generation(generation: &[u8]) -> u64 {
    syscall3(
        SYS_ROLLBACK_GENERATION,
        CAP_PROCESS_CONTROL,
        generation.as_ptr() as u64,
        generation.len() as u64,
    )
}

pub fn activate_generation(generation: &[u8]) -> u64 {
    syscall3(
        SYS_ACTIVATE_GENERATION,
        CAP_PROCESS_CONTROL,
        generation.as_ptr() as u64,
        generation.len() as u64,
    )
}

pub fn readiness_recv_timeout(buffer: &mut [u8], timeout_ms: u64) -> u64 {
    let packed = (timeout_ms << 32) | buffer.len() as u64;
    syscall3(
        SYS_IPC_RECV_TIMEOUT,
        CAP_READINESS,
        buffer.as_mut_ptr() as u64,
        packed,
    )
}

pub fn cap_derive(parent_slot: u64, new_slot: u64, rights_mask: u64) -> u64 {
    syscall3(SYS_CAP_DERIVE, parent_slot, new_slot, rights_mask)
}

pub fn cap_drop(slot: u64) -> u64 {
    syscall3(SYS_CAP_DROP, slot, 0, 0)
}

pub fn cap_transfer(target_pid: u64, cap_slot: u64, target_slot: u64, rights_mask: u64) -> u64 {
    let packed_transfer = (rights_mask << 32) | (target_slot << 16) | cap_slot;
    syscall3(
        SYS_CAP_TRANSFER,
        CAP_PROCESS_CONTROL,
        target_pid,
        packed_transfer,
    )
}

pub fn cap_inspect(slot: u64) -> u64 {
    syscall3(SYS_CAP_INSPECT, slot, 0, 0)
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> u64 {
    syscall3(SYS_CAP_MOVE, source_slot, target_slot, 0)
}

pub fn endpoint_create(cap_slot: u64) -> u64 {
    syscall3(SYS_ENDPOINT_CREATE, CAP_PROCESS_CONTROL, cap_slot, 0)
}

pub fn quota_delegate(target_pid: u64, max_endpoints: u64) -> u64 {
    syscall3(
        SYS_QUOTA_DELEGATE,
        CAP_PROCESS_CONTROL,
        target_pid,
        max_endpoints,
    )
}

pub fn yield_now() -> u64 {
    syscall3(SYS_YIELD, 0, 0, 0)
}

pub fn pause() {
    unsafe {
        asm!("pause", options(nomem, nostack, preserves_flags));
    }
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        pause();
    }
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    result
}
