#![no_std]
#![no_main]

mod sys;

use core::{panic::PanicInfo, ptr};

const CAP_SERIAL_LOG: u64 = 1;
const CAP_PROCESS_CONTROL: u64 = 3;
const CAP_CREATED_ENDPOINT: u64 = 4;

static mut START_MARKER: u64 = 0;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let marker = unsafe {
        let marker = ptr::addr_of_mut!(START_MARKER);
        let old = marker.read_volatile();
        marker.write_volatile(old + 1);
        old
    };
    if marker != 0 {
        log(b"flaky-service retained stale userspace data");
        sys::exit(1);
    }
    log(b"M66 restart uses zeroed userspace data");

    if sys::endpoint_create(CAP_PROCESS_CONTROL, CAP_CREATED_ENDPOINT) != sys::STATUS_OK {
        log(b"flaky-service restart quota endpoint create failed");
        sys::exit(1);
    }
    log(b"flaky-service creates quota-backed endpoint");
    if sys::process_attempt() <= 1 {
        log(b"flaky-service exits with status 1");
        sys::exit(1);
    }
    log(b"flaky-service restart quota restored");
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
