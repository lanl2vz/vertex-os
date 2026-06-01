#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_LOG: u64 = 1;
const CAP_VFS_LOCK_TEST: u64 = 0;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let handle = lock_fault_cleanup_file();
    if sys::process_attempt() & 1 == 1 {
        log(b"faulty-service holds VFS lock before fault");
        log(b"faulty-service triggers direct invalid load");
        unsafe {
            let fault = 0x0000_0000_dead_0000 as *const u64;
            let _ = fault.read_volatile();
        }
        loop {
            sys::pause();
        }
    }

    if sys::vfs_unlock(handle) != sys::STATUS_OK || sys::vfs_close(handle) != sys::STATUS_OK {
        log(b"faulty-service VFS lock cleanup failed");
        sys::exit(1);
    }
    log(b"faulty-service reacquires VFS lock after fault cleanup");
    log(b"faulty-service exits 0 after restart");
    sys::exit(0)
}

fn lock_fault_cleanup_file() -> u64 {
    let handle = sys::vfs_open_path_readwrite(CAP_VFS_LOCK_TEST, b"/state/a");
    if status_is_error(handle) {
        log(b"faulty-service VFS lock open failed");
        sys::exit(1);
    }
    let status = sys::vfs_lock_exclusive(handle);
    if status == sys::STATUS_VFS_BUSY {
        log(b"faulty-service VFS lock survived fault");
        sys::exit(1);
    }
    if status != sys::STATUS_OK {
        log(b"faulty-service VFS lock failed");
        sys::exit(1);
    }
    handle
}

fn status_is_error(status: u64) -> bool {
    status >= u64::MAX - 255
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
