#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

const SYS_LOG_WRITE: u64 = 7;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let message = b"Krust userspace says hello";

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") SYS_LOG_WRITE => _,
            in("rdi") CAP_SERIAL_LOG,
            in("rsi") message.as_ptr() as u64,
            in("rdx") message.len() as u64,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}
