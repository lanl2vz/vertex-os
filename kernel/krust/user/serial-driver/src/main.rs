#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_INPUT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_COM1: u64 = 3;
const CAP_VIRTIO_CONSOLE: u64 = 5;
const COM1: u64 = 0x3f8;
const COM1_LINE_STATUS: u64 = COM1 + 5;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"serial-driver ready");
    log(b"serial-driver has COM1 I/O port capability");
    if sys::virtio_probe(CAP_VIRTIO_CONSOLE) != sys::STATUS_OK {
        log(b"serial-driver virtio-console probe failed");
        sys::exit(1);
    }
    log(b"virtio-console replaces raw serial shell transport");

    if sys::io_read(CAP_COM1, COM1_LINE_STATUS) == sys::STATUS_BAD_CAPABILITY {
        log(b"serial-driver COM1 read failed");
        sys::exit(1);
    }

    if sys::io_write(CAP_COM1, COM1, b'*') != sys::STATUS_OK {
        log(b"serial-driver COM1 write failed");
        sys::exit(1);
    }
    write_byte(b'\n');
    log(b"serial-driver can write byte");
    send_ready();

    let mut buffer = [0u8; 96];
    let received = sys::ipc_recv(CAP_SERIAL_INPUT, &mut buffer);
    if received > buffer.len() as u64 {
        log(b"serial-driver receive failed");
        sys::exit(1);
    }
    let len = received as usize;
    write_bytes(&buffer[..len]);
    write_byte(b'\n');
    log(&buffer[..len]);
    log(b"serial-driver writes message to COM1");

    sys::exit(0)
}

fn log(value: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, value) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"serial-driver");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"serial-driver ready send failed");
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

fn write_bytes(value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        write_byte(value[index]);
        index += 1;
    }
}

fn write_byte(value: u8) {
    if value == b'\n' {
        let _ = sys::io_write(CAP_COM1, COM1, b'\r');
    }
    if sys::io_write(CAP_COM1, COM1, value) != sys::STATUS_OK {
        sys::exit(1);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
