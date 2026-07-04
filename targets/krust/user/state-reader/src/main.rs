#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE_VFS: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_NAMESPACE: u64 = 3;
const STATE_VALUE_PATH: &[u8] = b"/state/counter/value";
const UNDECLARED_STATE_VALUE_PATH: &[u8] = b"/state/scratch/value";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 8];
    log(b"reader-service has VFS state file");
    let handle = sys::vfs_open_path_read(CAP_STATE_VFS, STATE_VALUE_PATH);
    let read = sys::vfs_read(handle, &mut buffer);
    if read == sys::STATUS_BAD_CAPABILITY
        || read == sys::STATUS_BAD_BUFFER
        || read > buffer.len() as u64
    {
        log(b"reader-service state read failed");
        sys::exit(1);
    }
    let _ = sys::vfs_close(handle);
    log(b"reader-service reads state");
    log(b"reader-service receives state value");

    if sys::namespace_resolve(CAP_NAMESPACE, b"/state/b", CAP_STATE_VFS)
        == sys::STATUS_BAD_CAPABILITY
    {
        log(b"M68 namespace_resolve occupied slot leaves target unchanged");
    } else {
        log(b"reader-service namespace occupied-slot test failed");
        sys::exit(1);
    }

    let undeclared = sys::vfs_open_path_read(CAP_STATE_VFS, UNDECLARED_STATE_VALUE_PATH);
    if undeclared == sys::STATUS_VFS_PERMISSION || undeclared == sys::STATUS_BAD_CAPABILITY {
        log(b"statefs graph authority rejects undeclared state path alias");
    } else {
        let _ = sys::vfs_close(undeclared);
        log(b"reader-service undeclared state alias test failed");
        sys::exit(1);
    }

    if sys::vfs_open_path_readwrite(CAP_STATE_VFS, STATE_VALUE_PATH) == sys::STATUS_VFS_PERMISSION {
        log(b"reader-service write rejected");
        log(b"statefs shared state requires graph policy and attenuated VFS rights");
    } else {
        log(b"reader-service write denial failed");
        sys::exit(1);
    }

    log(b"Native state service client ok");
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
