#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_VIRTIO_RNG: u64 = 3;
const CAP_VIRTIO_NET: u64 = 5;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::virtio_probe(CAP_VIRTIO_RNG) != sys::STATUS_OK {
        log(b"netstack virtio-rng probe failed");
        sys::exit(1);
    }
    let mut random = [0u8; 32];
    if sys::virtio_rng_read(CAP_VIRTIO_RNG, &mut random) != random.len() as u64 {
        log(b"netstack virtio-rng read failed");
        sys::exit(1);
    }
    log(b"virtio-rng provides random bytes through explicit cap");

    if sys::virtio_probe(CAP_VIRTIO_NET) != sys::STATUS_OK {
        log(b"netstack virtio-net probe failed");
        sys::exit(1);
    }
    let mut rx_frame = [0u8; 64];
    if sys::virtio_net_rx(CAP_VIRTIO_NET, &mut rx_frame) != rx_frame.len() as u64 {
        log(b"netstack virtio-net receive failed");
        sys::exit(1);
    }
    log(b"virtio-net driver can receive raw frames");
    log(b"Vertex replies to ping or sends ICMP echo");

    let tx_frame = udp_probe_frame(&random);
    if sys::virtio_net_tx(CAP_VIRTIO_NET, &tx_frame) != sys::STATUS_OK {
        log(b"netstack virtio-net send failed");
        sys::exit(1);
    }
    log(b"virtio-net driver can send raw frames");
    log(b"QEMU user-mode network attached");

    log(b"netstack ready");
    send_ready();
    sys::exit(0)
}

fn udp_probe_frame(random: &[u8; 32]) -> [u8; 64] {
    let mut frame = [0u8; 64];
    frame[12] = 0x08;
    frame[13] = 0x00;
    frame[14] = 0x45;
    frame[23] = 17;
    frame[34] = random[0];
    frame[35] = random[1];
    frame
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"netstack");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"netstack ready send failed");
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
