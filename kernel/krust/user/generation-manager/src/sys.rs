use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;
pub const STATUS_TOO_LARGE: u64 = u64::MAX - 3;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_LOG_WRITE: u64 = 7;
const SYS_ACTIVATE_GENERATION: u64 = 8;
const SYS_ROLLBACK_GENERATION: u64 = 18;
const SYS_VERIFY_GENERATION: u64 = 72;

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

pub fn activate_generation(cap_slot: u64, generation: &[u8]) -> u64 {
    syscall3(
        SYS_ACTIVATE_GENERATION,
        cap_slot,
        generation.as_ptr() as u64,
        generation.len() as u64,
    )
}

pub fn verify_generation(cap_slot: u64, generation: &[u8]) -> u64 {
    syscall3(
        SYS_VERIFY_GENERATION,
        cap_slot,
        generation.as_ptr() as u64,
        generation.len() as u64,
    )
}

pub fn rollback_generation(cap_slot: u64, generation: &[u8]) -> u64 {
    syscall3(
        SYS_ROLLBACK_GENERATION,
        cap_slot,
        generation.as_ptr() as u64,
        generation.len() as u64,
    )
}

pub fn exit(code: u64) -> ! {
    syscall1(SYS_EXIT, code);
    loop {
        core::hint::spin_loop();
    }
}

fn syscall1(number: u64, arg0: u64) -> u64 {
    let ret;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => ret,
            in("rdi") arg0,
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    ret
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let ret;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => ret,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    ret
}
