use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const RIGHT_SEND: u64 = 1 << 4;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_LOG_WRITE: u64 = 7;
const SYS_CAP_DROP: u64 = 11;
const SYS_OBJECT_READ: u64 = 13;
const SYS_PROCESS_ATTEMPT: u64 = 20;
const SYS_CAP_REVOKE: u64 = 21;
const SYS_CAP_INSPECT: u64 = 22;
const SYS_CAP_MOVE: u64 = 23;
const SYS_CAP_COPY: u64 = 24;
const SYS_ENDPOINT_CREATE: u64 = 25;
const SYS_IO_READ: u64 = 27;
const SYS_IO_WRITE: u64 = 28;
const SYS_IRQ_WAIT: u64 = 29;
const SYS_DMA_MAP: u64 = 32;

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

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn cap_drop(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_DROP, cap_slot, 0, 0)
}

pub fn cap_revoke(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_REVOKE, cap_slot, 0, 0)
}

pub fn cap_inspect(cap_slot: u64) -> u64 {
    syscall3(SYS_CAP_INSPECT, cap_slot, 0, 0)
}

pub fn cap_move(source_slot: u64, target_slot: u64) -> u64 {
    syscall3(SYS_CAP_MOVE, source_slot, target_slot, 0)
}

pub fn cap_copy(source_slot: u64, target_slot: u64, rights: u64) -> u64 {
    syscall3(SYS_CAP_COPY, source_slot, target_slot, rights)
}

pub fn endpoint_create(control_slot: u64, cap_slot: u64) -> u64 {
    syscall3(SYS_ENDPOINT_CREATE, control_slot, cap_slot, 0)
}

pub fn io_write(cap_slot: u64, port: u64, value: u8) -> u64 {
    syscall3(SYS_IO_WRITE, cap_slot, port, value as u64)
}

pub fn io_read(cap_slot: u64, port: u64) -> u64 {
    syscall3(SYS_IO_READ, cap_slot, port, 0)
}

pub fn irq_wait(cap_slot: u64, timeout_ms: u64) -> u64 {
    syscall3(SYS_IRQ_WAIT, cap_slot, timeout_ms, 0)
}

pub fn dma_map(cap_slot: u64, buffer: &mut [u8; 24]) -> u64 {
    syscall3(
        SYS_DMA_MAP,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn object_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_OBJECT_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn process_attempt() -> u64 {
    syscall3(SYS_PROCESS_ATTEMPT, 0, 0, 0)
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
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
