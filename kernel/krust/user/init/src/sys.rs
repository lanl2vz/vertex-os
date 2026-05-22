use core::arch::asm;

pub const CAP_MANIFEST: u64 = 0;
pub const CAP_LOG: u64 = 1;
pub const CAP_PROCESS_CONTROL: u64 = 2;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_BAD_BUFFER: u64 = u64::MAX - 2;

const SYS_EXIT: u64 = 2;
const SYS_BOOT_READ: u64 = 6;
const SYS_LOG_WRITE: u64 = 7;
const SYS_ACTIVATE_GENERATION: u64 = 8;

pub fn read_manifest(buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_BOOT_READ,
        CAP_MANIFEST,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn log(message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        CAP_LOG,
        message.as_ptr() as u64,
        message.len() as u64,
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
