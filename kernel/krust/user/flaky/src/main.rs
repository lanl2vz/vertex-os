#![no_std]
#![no_main]

mod sys;

use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicU64, Ordering},
};

const CAP_SERIAL_LOG: u64 = 1;

static RUNS: AtomicU64 = AtomicU64::new(0);

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    if run == 0 {
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
