#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_SERIAL_DRIVER: u64 = 3;
const CAP_CONFIG: u64 = 4;
const CAP_SECRET: u64 = 5;
const ECHO_PROCESS_INDEX: u64 = 2;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let ready = ready_message(b"logd");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"logd ready send failed");
        sys::exit(1);
    }
    log(b"logd ready");

    if sys::ipc_send(CAP_SERIAL_DRIVER, b"logd sends log message") != sys::STATUS_OK {
        log(b"logd serial-driver send failed");
        sys::exit(1);
    }
    if sys::io_write(CAP_SERIAL_DRIVER, 0x3f8, b'!') == sys::STATUS_BAD_CAPABILITY {
        log(b"logd cannot write COM1 directly");
    } else {
        log(b"logd COM1 denial failed");
        sys::exit(1);
    }

    let mut config = [0u8; 64];
    let config_len = sys::object_read(CAP_CONFIG, &mut config);
    if config_len == sys::STATUS_BAD_CAPABILITY || config_len > config.len() as u64 {
        log(b"logd config read failed");
        sys::exit(1);
    }
    log(b"logd reads config object");

    let mut secret = [0u8; 64];
    let secret_len = sys::secret_read(CAP_SECRET, &mut secret);
    if secret_len == sys::STATUS_BAD_CAPABILITY
        || secret_len == 0
        || secret_len > secret.len() as u64
    {
        log(b"logd secret read failed");
        sys::exit(1);
    }
    log(b"service with secret cap reads secret");

    if sys::ipc_recv_raw(CAP_LOG_SINK, 1, 8) == sys::STATUS_BAD_BUFFER
        && sys::object_read_raw(CAP_CONFIG, 1, 8) == sys::STATUS_BAD_BUFFER
    {
        log(b"M61 provider malformed receive/read buffers rejected");
    } else {
        log(b"M61 provider malformed buffer test failed");
        sys::exit(1);
    }

    let mut buffer = [0u8; 64];
    let received = sys::ipc_recv(CAP_LOG_SINK, &mut buffer);
    if received > buffer.len() as u64 {
        log(b"logd receive failed");
        sys::exit(1);
    }

    log_prefix(b"logd received: ", &buffer[..received as usize]);

    if sys::process_create(CAP_LOG_SINK, ECHO_PROCESS_INDEX) == sys::STATUS_BAD_CAPABILITY {
        log(b"unprivileged service calls SYS_PROCESS_CREATE");
        log(b"negative test: logd process-create rejected: bad capability");
    } else {
        log(b"logd negative process-create failed");
        sys::exit(1);
    }

    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn append(buffer: &mut [u8], mut offset: usize, value: &[u8]) -> usize {
    let mut index = 0;
    while offset < buffer.len() && index < value.len() {
        buffer[offset] = value[index];
        offset += 1;
        index += 1;
    }
    offset
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
