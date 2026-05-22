#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 64];
    log(b"model-reader asks for store:hello-text");
    if sys::ipc_send(CAP_STORE, b"store:hello-text") != sys::STATUS_OK {
        log(b"model-reader store request failed");
        sys::exit(1);
    }
    let read = sys::ipc_recv(CAP_STORE, &mut buffer);
    if read == sys::STATUS_BAD_CAPABILITY || read == sys::STATUS_BAD_BUFFER {
        log(b"model-reader store read failed");
        sys::exit(1);
    }
    if read > buffer.len() as u64 || !bytes_eq(&buffer[..read as usize], HELLO_OBJECT) {
        log(b"model-reader store bytes invalid");
        sys::exit(1);
    }
    log(b"model-reader reads bytes");
    log(b"model-reader reads bytes successfully");
    log(b"Native immutable store client ok");
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
