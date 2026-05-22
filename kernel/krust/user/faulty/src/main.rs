#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_LOG: u64 = 1;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::process_attempt() <= 1 {
        log(b"faulty-service triggers direct invalid load");
        unsafe {
            let fault = 0x0000_0000_dead_0000 as *const u64;
            let _ = fault.read_volatile();
        }
        loop {
            sys::pause();
        }
    }

    log(b"faulty-service exits 0 after restart");
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
