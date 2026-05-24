#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_BLOCK_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_BLOCK_REPLY: u64 = 3;
const CAP_MMIO: u64 = 4;
const CAP_IRQ: u64 = 5;
const CAP_DMA: u64 = 6;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"block-driver ready");

    if sys::mmio_map(CAP_MMIO) == sys::STATUS_BAD_CAPABILITY {
        log(b"block-driver MMIO authority failed");
        sys::exit(1);
    }
    if sys::irq_wait(CAP_IRQ, 0) != sys::STATUS_OK {
        log(b"block-driver IRQ authority failed");
        sys::exit(1);
    }
    if sys::mmio_map(CAP_DMA) == sys::STATUS_BAD_CAPABILITY {
        log(b"block-driver DMA is distinct from MMIO authority");
    }

    send_ready();

    let mut request = [0u8; 32];
    let received = sys::ipc_recv(CAP_BLOCK_REQUEST, &mut request);
    if received > request.len() as u64 {
        log(b"block-driver request receive failed");
        sys::exit(1);
    }
    log(b"block-driver received block-read request");

    if sys::ipc_send(CAP_BLOCK_REPLY, HELLO_OBJECT) != sys::STATUS_OK {
        log(b"block-driver response failed");
        sys::exit(1);
    }
    log(b"block-driver returns bytes");
    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"block-driver");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"block-driver ready send failed");
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
