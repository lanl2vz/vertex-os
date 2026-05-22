#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::ipc_send(CAP_LOG_SINK, b"hello from echo") != sys::STATUS_OK {
        log(b"echo send failed");
        sys::exit(1);
    }
    log(b"echo sent message to logd");

    let mut denied = [0u8; 8];
    if sys::ipc_recv(CAP_LOG_SINK, &mut denied) == sys::STATUS_BAD_CAPABILITY {
        log(b"negative test: echo receive rejected: bad capability");
    } else {
        log(b"echo negative receive failed");
        sys::exit(1);
    }

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
