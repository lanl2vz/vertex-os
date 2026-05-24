#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_BLOCK_REPLY: u64 = 3;
const CAP_BLOCK_REQUEST: u64 = 4;
const CAP_MODEL_REPLY: u64 = 5;
const CAP_INIT_REPLY: u64 = 6;
const STORE_ID: &[u8] = b"store:hello-text";
const GENERATION_B_MANIFEST_ID: &[u8] = b"store:generation-b-manifest";
const GENERATION_B_MANIFEST: &[u8] = b"krustboot:gen:switch-b-0002";
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    send_ready();
    log(b"vertex-store ready");

    loop {
        let mut request = [0u8; 64];
        let received = sys::ipc_recv(CAP_STORE_REQUEST, &mut request);
        if received > request.len() as u64 {
            log(b"vertex-store request invalid");
            sys::exit(1);
        }

        let request = &request[..received as usize];
        if starts_with(request, GENERATION_B_MANIFEST_ID) {
            serve_generation_b_manifest();
            continue;
        }
        if starts_with(request, STORE_ID) {
            serve_hello_object();
        }

        log(b"vertex-store request invalid");
        sys::exit(1);
    }
}

fn serve_generation_b_manifest() {
    log(b"vertex-store exposes generation B manifest");
    if sys::ipc_send(CAP_INIT_REPLY, GENERATION_B_MANIFEST) != sys::STATUS_OK {
        log(b"vertex-store generation manifest response failed");
        sys::exit(1);
    }
}

fn serve_hello_object() -> ! {
    log(b"store-service requests block read");
    if sys::ipc_send(CAP_BLOCK_REQUEST, b"read store:hello-text") != sys::STATUS_OK {
        log(b"vertex-store block request failed");
        sys::exit(1);
    }

    let mut object = [0u8; 64];
    let object_len = sys::ipc_recv(CAP_BLOCK_REPLY, &mut object);
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

    if sys::ipc_send(CAP_MODEL_REPLY, &object[..object_len]) != sys::STATUS_OK {
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

fn send_ready() {
    let ready = ready_message(b"vertex-store");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"vertex-store ready send failed");
        sys::exit(1);
    }
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    write_u16(&mut message, 0, PROTOCOL_HEALTH_V0);
    write_u16(&mut message, 2, MESSAGE_READY);
    write_u32(&mut message, 4, service.len() as u32);
    write_u64(&mut message, 8, 1);
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
    buffer[offset + 2] = bytes[2];
    buffer[offset + 3] = bytes[3];
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
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
