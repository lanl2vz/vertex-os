use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_TOO_LARGE: u64 = u64::MAX - 3;
pub const STATUS_EMPTY: u64 = u64::MAX - 4;
pub const STATUS_TIMEOUT: u64 = u64::MAX - 9;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV_TIMEOUT: u64 = 19;
const SYS_LOG_WRITE: u64 = 7;
const SYS_YIELD: u64 = 5;
const SYS_FRAMEBUFFER_MAP: u64 = 76;

pub fn ipc_recv_timeout(cap_slot: u64, buffer: &mut [u8], timeout_ms: u64) -> u64 {
    syscall3(
        SYS_IPC_RECV_TIMEOUT,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        (timeout_ms << 32) | buffer.len() as u64,
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

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn framebuffer_map(cap_slot: u64, info: &mut [u8; 64]) -> u64 {
    syscall3(
        SYS_FRAMEBUFFER_MAP,
        cap_slot,
        info.as_mut_ptr() as u64,
        info.len() as u64,
    )
}

pub fn yield_now() -> u64 {
    syscall3(SYS_YIELD, 0, 0, 0)
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
