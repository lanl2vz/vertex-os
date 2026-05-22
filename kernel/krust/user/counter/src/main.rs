#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"counter-service has state API cap");
    if sys::ipc_send(CAP_STATE, b"W1") != sys::STATUS_OK {
        log(b"counter-service state write failed");
        sys::exit(1);
    }
    log(b"counter-service sends state write");
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
