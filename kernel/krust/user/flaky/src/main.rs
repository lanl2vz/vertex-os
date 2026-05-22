#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::process_attempt() <= 1 {
        log(b"flaky-service exits with status 1");
        sys::exit(1);
    }
    log(b"flaky-service exits 0");
    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
