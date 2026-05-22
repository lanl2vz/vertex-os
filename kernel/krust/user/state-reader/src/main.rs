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
    log(b"reader-service has state API cap");
    if sys::ipc_send(CAP_STATE, b"R") != sys::STATUS_OK {
        log(b"reader-service state read request failed");
        sys::exit(1);
    }
    let mut read = sys::ipc_recv(CAP_STATE, &mut buffer);
    let mut attempts = 0;
    while read == sys::STATUS_EMPTY && attempts < 64 {
        sys::yield_now();
        read = sys::ipc_recv(CAP_STATE, &mut buffer);
        attempts += 1;
    }
    if read == sys::STATUS_BAD_CAPABILITY
        || read == sys::STATUS_BAD_BUFFER
        || read > buffer.len() as u64
    {
        log(b"reader-service state read failed");
        sys::exit(1);
    }
    log(b"reader-service reads state");
    log(b"reader-service receives state value");

    if sys::ipc_send(CAP_STATE, b"W2") != sys::STATUS_OK {
        log(b"reader-service write request failed");
        sys::exit(1);
    }
    let mut denial = [0u8; 8];
    let denied = sys::ipc_recv(CAP_STATE, &mut denial);
    if denied == 6 && bytes_eq(&denial[..6], b"DENIED") {
        log(b"reader-service write rejected");
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

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
