#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE_API: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_BLOCK: u64 = 3;
const STORE_ID: &[u8] = b"store:hello-text";
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"vertex-store ready");

    let mut request = [0u8; 64];
    let received = sys::ipc_recv(CAP_STORE_API, &mut request);
    if received > request.len() as u64 || !starts_with(&request[..received as usize], STORE_ID) {
        log(b"vertex-store request invalid");
        sys::exit(1);
    }

    log(b"store-service requests block read");
    if sys::ipc_send(CAP_BLOCK, b"read store:hello-text") != sys::STATUS_OK {
        log(b"vertex-store block request failed");
        sys::exit(1);
    }

    let mut object = [0u8; 64];
    let object_len = sys::ipc_recv(CAP_BLOCK, &mut object);
    if object_len > object.len() as u64 {
        log(b"vertex-store block response failed");
        sys::exit(1);
    }
    let object_len = object_len as usize;
    if !bytes_eq(&object[..object_len], HELLO_OBJECT) {
        log(b"vertex-store hash verification failed");
        sys::exit(1);
    }
    log(b"vertex-store verifies hash");

    object[0] ^= 1;
    if !bytes_eq(&object[..object_len], HELLO_OBJECT) {
        log(b"modified object fails hash check");
    } else {
        log(b"vertex-store modified-object negative failed");
        sys::exit(1);
    }
    object[0] ^= 1;

    if sys::ipc_send(CAP_STORE_API, &object[..object_len]) != sys::STATUS_OK {
        log(b"vertex-store response failed");
        sys::exit(1);
    }
    log(b"Native immutable store service ok");
    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    bytes_eq(&value[..prefix.len()], prefix)
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
