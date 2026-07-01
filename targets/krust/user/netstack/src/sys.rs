use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_BAD_CAPABILITY: u64 = u64::MAX - 1;
pub const STATUS_EMPTY: u64 = u64::MAX - 4;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_YIELD: u64 = 5;
const SYS_LOG_WRITE: u64 = 7;
const SYS_VIRTIO_DEVICE_PROBE: u64 = 40;
const SYS_VIRTIO_RNG_READ: u64 = 41;
const SYS_VIRTIO_NET_TX: u64 = 42;
const SYS_VIRTIO_NET_RX: u64 = 43;
const SYS_NETWORK_RECV_UDP: u64 = 46;

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn log(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_LOG_WRITE,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn virtio_probe(cap_slot: u64) -> u64 {
    syscall3(SYS_VIRTIO_DEVICE_PROBE, cap_slot, 0, 0)
}

pub fn virtio_rng_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_VIRTIO_RNG_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn virtio_net_tx(cap_slot: u64, frame: &[u8]) -> u64 {
    syscall3(
        SYS_VIRTIO_NET_TX,
        cap_slot,
        frame.as_ptr() as u64,
        frame.len() as u64,
    )
}

pub fn virtio_net_rx(cap_slot: u64, frame: &mut [u8]) -> u64 {
    syscall3(
        SYS_VIRTIO_NET_RX,
        cap_slot,
        frame.as_mut_ptr() as u64,
        frame.len() as u64,
    )
}

pub fn network_recv_udp(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_NETWORK_RECV_UDP,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn yield_now() -> u64 {
    syscall3(SYS_YIELD, 0, 0, 0)
}

pub fn exit(status: u64) -> ! {
    syscall3(SYS_EXIT, status, 0, 0);
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

fn syscall3(number: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;

    unsafe {
        asm!(
            "syscall",
            inlateout("rax") number => result,
            in("rdi") arg0,
            in("rsi") arg1,
            in("rdx") arg2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }

    result
}
