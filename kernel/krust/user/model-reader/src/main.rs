#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 64];
    log(b"model-reader has read cap to store:hello-text");
    let read = sys::object_read(CAP_STORE, &mut buffer);
    if read == sys::STATUS_BAD_CAPABILITY || read == sys::STATUS_BAD_BUFFER {
        log(b"model-reader store read failed");
        sys::exit(1);
    }
    log(b"model-reader reads bytes successfully");
    log(b"Native store-object read ok");
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
