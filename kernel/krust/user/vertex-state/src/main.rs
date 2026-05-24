#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_READER_REPLY: u64 = 3;
const CAP_STATE_BACKEND: u64 = 4;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    send_ready();
    log(b"vertex-state ready");

    let mut request = [0u8; 16];
    let mut value = [0u8; 16];
    let value_len: usize;
    let mut wrote_value = false;
    let mut pending_read = false;
    loop {
        let received = sys::ipc_recv(CAP_STATE_REQUEST, &mut request);
        if received == 1 && request[0] == b'R' {
            if wrote_value {
                let read_len = read_snapshot(&mut value);
                send_read_response(&value[..read_len]);
                value_len = read_len;
                break;
            }
            pending_read = true;
            continue;
        }

        if received >= 2 && received <= request.len() as u64 && request[0] == b'W' && !wrote_value {
            if sys::state_write(CAP_STATE_BACKEND, &request[1..received as usize]) != sys::STATUS_OK
            {
                log(b"vertex-state backend write failed");
                sys::exit(1);
            }
            log(b"counter-service writes state");
            wrote_value = true;
            if pending_read {
                let read_len = read_snapshot(&mut value);
                send_read_response(&value[..read_len]);
                value_len = read_len;
                break;
            }
            continue;
        }

        log(b"vertex-state request invalid");
        sys::exit(1);
    }

    let received = sys::ipc_recv(CAP_STATE_REQUEST, &mut request);
    if received < 2 || received > request.len() as u64 || request[0] != b'W' {
        log(b"vertex-state write-denial request invalid");
        sys::exit(1);
    }
    log(b"reader-service write denied");
    if sys::ipc_send(CAP_READER_REPLY, b"DENIED") != sys::STATUS_OK {
        log(b"vertex-state denial response failed");
        sys::exit(1);
    }

    if sys::state_write(CAP_STATE_BACKEND, &value[..value_len]) != sys::STATUS_OK {
        log(b"vertex-state restore failed");
        sys::exit(1);
    }
    log(b"state restored");
    log(b"system generation rollback does not automatically roll back state unless policy says so");
    log(b"Native state-volume service ok");
    sys::exit(0)
}

fn read_snapshot(value: &mut [u8; 16]) -> usize {
    let value_len = sys::state_read(CAP_STATE_BACKEND, value);
    if value_len == sys::STATUS_EMPTY || value_len > value.len() as u64 {
        log(b"vertex-state backend read failed");
        sys::exit(1);
    }
    log(b"snapshot created");
    value_len as usize
}

fn send_read_response(value: &[u8]) {
    if sys::ipc_send(CAP_READER_REPLY, value) != sys::STATUS_OK {
        log(b"vertex-state read response failed");
        sys::exit(1);
    }
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"vertex-state");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"vertex-state ready send failed");
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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
