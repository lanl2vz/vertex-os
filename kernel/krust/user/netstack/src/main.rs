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

    let mut rng_denied = [0u8; 8];
    let mut net_denied = [0u8; 64];
    if sys::virtio_rng_read(CAP_VIRTIO_NET, &mut rng_denied) == sys::STATUS_BAD_CAPABILITY
        && sys::virtio_net_tx(CAP_VIRTIO_RNG, b"wrong device") == sys::STATUS_BAD_CAPABILITY
        && sys::virtio_net_rx(CAP_VIRTIO_RNG, &mut net_denied) == sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 virtio typed device syscalls reject mismatched device IDs");
    } else {
        log(b"M61 virtio typed device denial failed");
        sys::exit(1);
    }

    if sys::virtio_probe(CAP_VIRTIO_NET) != sys::STATUS_OK {
        log(b"netstack virtio-net probe failed");
        sys::exit(1);
    }
    let tx_frame = arp_probe_frame(&random);
    if sys::virtio_net_tx(CAP_VIRTIO_NET, &tx_frame) != sys::STATUS_OK {
        log(b"netstack virtio-net send failed");
        sys::exit(1);
    }
    log(b"virtio-net driver can send raw frames");

    let mut rx_frame = [0u8; 128];
    let received = sys::virtio_net_rx(CAP_VIRTIO_NET, &mut rx_frame);
    if received < 60 || received > rx_frame.len() as u64 {
        log(b"netstack virtio-net receive failed");
        sys::exit(1);
    }
    if !is_arp_reply_from_gateway(&rx_frame[..received as usize]) {
        log(b"netstack ARP reply validation failed");
        sys::exit(1);
    }
    log(b"virtio-net driver can receive raw frames");
    log(b"QEMU user-mode network delivered a raw frame");

    let gateway_mac = ethernet_source(&rx_frame);
    let icmp_frame = icmp_echo_frame(gateway_mac, &random);
    if sys::virtio_net_tx(CAP_VIRTIO_NET, &icmp_frame) != sys::STATUS_OK {
        log(b"netstack ICMP echo send failed");
        sys::exit(1);
    }
    log(b"Vertex sends ICMP echo");
    let icmp_reply = sys::virtio_net_rx(CAP_VIRTIO_NET, &mut rx_frame);
    if icmp_reply < 60 || icmp_reply > rx_frame.len() as u64 {
        log(b"netstack ICMP echo reply failed");
        sys::exit(1);
    }
    if !is_icmp_echo_reply_from_gateway(&rx_frame[..icmp_reply as usize], &random) {
        log(b"netstack ICMP echo reply validation failed");
        sys::exit(1);
    }
    log(b"QEMU user-mode network delivered ICMP echo reply");
    log(b"QEMU user-mode network attached");

    log(b"netstack ready");
    send_ready();
    sys::exit(0)
}

fn arp_probe_frame(random: &[u8; 32]) -> [u8; 60] {
    let mut frame = [0u8; 60];
    let source_mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    let mut index = 0;
    while index < 6 {
        frame[index] = 0xff;
        frame[6 + index] = source_mac[index];
        frame[22 + index] = source_mac[index];
        index += 1;
    }
    frame[12] = 0x08;
    frame[13] = 0x06;
    frame[14] = 0x00;
    frame[15] = 0x01;
    frame[16] = 0x08;
    frame[17] = 0x00;
    frame[18] = 6;
    frame[19] = 4;
    frame[20] = 0x00;
    frame[21] = 0x01;
    frame[28] = 10;
    frame[29] = 0;
    frame[30] = 2;
    frame[31] = 15;
    frame[38] = 10;
    frame[39] = 0;
    frame[40] = 2;
    frame[41] = 2;
    frame[42] = random[0];
    frame
}

fn icmp_echo_frame(destination_mac: [u8; 6], random: &[u8; 32]) -> [u8; 60] {
    let mut frame = [0u8; 60];
    let source_mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    let mut index = 0;
    while index < 6 {
        frame[index] = destination_mac[index];
        frame[6 + index] = source_mac[index];
        index += 1;
    }
    frame[12] = 0x08;
    frame[13] = 0x00;

    let ip = 14;
    frame[ip] = 0x45;
    write_be_u16(&mut frame, ip + 2, 28);
    write_be_u16(&mut frame, ip + 4, 0x5657);
    frame[ip + 8] = 64;
    frame[ip + 9] = 1;
    frame[ip + 12] = 10;
    frame[ip + 14] = 2;
    frame[ip + 15] = 15;
    frame[ip + 16] = 10;
    frame[ip + 18] = 2;
    frame[ip + 19] = 2;
    let ip_checksum = checksum(&frame[ip..ip + 20]);
    write_be_u16(&mut frame, ip + 10, ip_checksum);

    let icmp = ip + 20;
    frame[icmp] = 8;
    write_be_u16(&mut frame, icmp + 4, 0x5657);
    write_be_u16(&mut frame, icmp + 6, random[1] as u16);
    let icmp_checksum = checksum(&frame[icmp..icmp + 8]);
    write_be_u16(&mut frame, icmp + 2, icmp_checksum);
    frame
}

fn ethernet_source(frame: &[u8]) -> [u8; 6] {
    let mut mac = [0u8; 6];
    let mut index = 0;
    while index < 6 && 6 + index < frame.len() {
        mac[index] = frame[6 + index];
        index += 1;
    }
    mac
}

fn is_arp_reply_from_gateway(frame: &[u8]) -> bool {
    frame.len() >= 42
        && frame[12] == 0x08
        && frame[13] == 0x06
        && frame[20] == 0x00
        && frame[21] == 0x02
        && frame[28] == 10
        && frame[29] == 0
        && frame[30] == 2
        && frame[31] == 2
        && frame[38] == 10
        && frame[39] == 0
        && frame[40] == 2
        && frame[41] == 15
}

fn is_icmp_echo_reply_from_gateway(frame: &[u8], random: &[u8; 32]) -> bool {
    let ip = 14;
    let icmp = ip + 20;
    frame.len() >= icmp + 8
        && frame[12] == 0x08
        && frame[13] == 0x00
        && frame[ip] == 0x45
        && frame[ip + 9] == 1
        && frame[ip + 12] == 10
        && frame[ip + 13] == 0
        && frame[ip + 14] == 2
        && frame[ip + 15] == 2
        && frame[ip + 16] == 10
        && frame[ip + 17] == 0
        && frame[ip + 18] == 2
        && frame[ip + 19] == 15
        && frame[icmp] == 0
        && frame[icmp + 1] == 0
        && frame[icmp + 4] == 0x56
        && frame[icmp + 5] == 0x57
        && frame[icmp + 6] == 0
        && frame[icmp + 7] == random[1]
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut index = 0;
    while index + 1 < bytes.len() {
        sum += u16::from_be_bytes([bytes[index], bytes[index + 1]]) as u32;
        index += 2;
    }
    if index < bytes.len() {
        sum += (bytes[index] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
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

fn write_be_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_be_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
