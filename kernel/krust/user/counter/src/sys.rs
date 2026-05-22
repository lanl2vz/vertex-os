use core::arch::asm;

pub const STATUS_OK: u64 = 0;

const SYS_EXIT: u64 = 2;
const SYS_LOG_WRITE: u64 = 7;
const SYS_STATE_WRITE: u64 = 14;

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn state_write(cap_slot: u64, value: &[u8]) -> u64 {
    syscall3(
        SYS_STATE_WRITE,
        cap_slot,
        value.as_ptr() as u64,
        value.len() as u64,
    )
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
