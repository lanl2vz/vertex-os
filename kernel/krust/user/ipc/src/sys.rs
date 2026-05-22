use core::arch::asm;

pub const ENDPOINT_CAP_SLOT: u64 = 0;
pub const SERIAL_LOG_CAP_SLOT: u64 = 1;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const BAD_USER_PTR: u64 = 0x0000_6000_0000_0000;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_YIELD: u64 = 5;
const SYS_LOG_WRITE: u64 = 7;

pub fn log_write(message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        SERIAL_LOG_CAP_SLOT,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

#[allow(dead_code)]
pub fn log_write_raw(user_ptr: u64, len: u64) -> u64 {
    syscall3(SYS_LOG_WRITE, SERIAL_LOG_CAP_SLOT, user_ptr, len)
}

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

#[allow(dead_code)]
pub fn ipc_send_raw(cap_slot: u64, user_ptr: u64, len: u64) -> u64 {
    syscall3(SYS_IPC_SEND, cap_slot, user_ptr, len)
}

pub fn ipc_recv(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_IPC_RECV,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

#[allow(dead_code)]
pub fn ipc_recv_raw(cap_slot: u64, user_ptr: u64, len: u64) -> u64 {
    syscall3(SYS_IPC_RECV, cap_slot, user_ptr, len)
}

#[allow(dead_code)]
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
