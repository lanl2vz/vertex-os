#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 8];
    log(b"reader-service has read-only cap");
    let mut read = sys::state_read(CAP_STATE, &mut buffer);
    let mut attempts = 0;
    while read == sys::STATUS_EMPTY && attempts < 64 {
        sys::yield_now();
        read = sys::state_read(CAP_STATE, &mut buffer);
        attempts += 1;
    }
    if read == sys::STATUS_BAD_CAPABILITY
        || read == sys::STATUS_BAD_BUFFER
        || read > buffer.len() as u64
    {
        log(b"reader-service state read failed");
        sys::exit(1);
    }
    log(b"reader-service reads value");

    if sys::state_write(CAP_STATE, b"2") == sys::STATUS_BAD_CAPABILITY {
        log(b"reader-service write rejected");
    } else {
        log(b"reader-service write denial failed");
        sys::exit(1);
    }
    log(b"Native state-volume access ok");
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
