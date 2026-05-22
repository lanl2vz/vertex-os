#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_TIMER: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"timer-service sleeps 10 ms");
    if sys::sleep_ms(CAP_TIMER, 10) != sys::STATUS_OK {
        log(b"timer-service sleep failed");
        sys::exit(1);
    }
    log(b"wakes");
    log(b"timer ok");
    log(b"Native timer ok");
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
