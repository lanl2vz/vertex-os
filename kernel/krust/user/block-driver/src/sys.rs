use core::arch::asm;

pub const STATUS_OK: u64 = 0;
pub const STATUS_TIMEOUT: u64 = u64::MAX - 9;

const SYS_EXIT: u64 = 2;
const SYS_IPC_SEND: u64 = 3;
const SYS_IPC_RECV: u64 = 4;
const SYS_LOG_WRITE: u64 = 7;
const SYS_OBJECT_READ: u64 = 13;
const SYS_IPC_RECV_TIMEOUT: u64 = 19;
const SYS_PROCESS_ATTEMPT: u64 = 20;
const SYS_IO_READ: u64 = 27;
const SYS_IO_WRITE: u64 = 28;
const SYS_IRQ_WAIT: u64 = 29;
const SYS_DMA_MAP: u64 = 32;
const SYS_IO_READ16: u64 = 33;
const SYS_IO_WRITE16: u64 = 34;
const SYS_IO_READ32: u64 = 35;
const SYS_IO_WRITE32: u64 = 36;
const SYS_VIRTIO_DEVICE_PROBE: u64 = 40;
const SYS_VIRTIO_DEVICE_REPORT: u64 = 47;

#[repr(C)]
pub struct DmaMapping {
    pub virtual_base: u64,
    pub physical_base: u64,
    pub length: u64,
}

pub fn ipc_send(cap_slot: u64, message: &[u8]) -> u64 {
    syscall3(
        SYS_IPC_SEND,
        cap_slot,
        message.as_ptr() as u64,
        message.len() as u64,
    )
}

pub fn ipc_recv(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_IPC_RECV,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn ipc_recv_timeout(cap_slot: u64, buffer: &mut [u8], timeout_ms: u64) -> u64 {
    syscall3(
        SYS_IPC_RECV_TIMEOUT,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        (timeout_ms << 32) | buffer.len() as u64,
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

pub fn process_attempt() -> u64 {
    syscall3(SYS_PROCESS_ATTEMPT, 0, 0, 0)
}

pub fn irq_wait(cap_slot: u64, timeout_ms: u64) -> u64 {
    syscall3(SYS_IRQ_WAIT, cap_slot, timeout_ms, 0)
}

pub fn object_read(cap_slot: u64, buffer: &mut [u8]) -> u64 {
    syscall3(
        SYS_OBJECT_READ,
        cap_slot,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    )
}

pub fn io_read(cap_slot: u64, port: u64) -> u64 {
    syscall3(SYS_IO_READ, cap_slot, port, 0)
}

pub fn io_write(cap_slot: u64, port: u64, value: u8) -> u64 {
    syscall3(SYS_IO_WRITE, cap_slot, port, value as u64)
}

pub fn io_read16(cap_slot: u64, port: u64) -> u64 {
    syscall3(SYS_IO_READ16, cap_slot, port, 0)
}

pub fn io_write16(cap_slot: u64, port: u64, value: u16) -> u64 {
    syscall3(SYS_IO_WRITE16, cap_slot, port, value as u64)
}

pub fn io_read32(cap_slot: u64, port: u64) -> u64 {
    syscall3(SYS_IO_READ32, cap_slot, port, 0)
}

pub fn io_write32(cap_slot: u64, port: u64, value: u32) -> u64 {
    syscall3(SYS_IO_WRITE32, cap_slot, port, value as u64)
}

pub fn dma_map(cap_slot: u64, mapping: &mut DmaMapping) -> u64 {
    syscall3(
        SYS_DMA_MAP,
        cap_slot,
        mapping as *mut DmaMapping as u64,
        core::mem::size_of::<DmaMapping>() as u64,
    )
}

pub fn virtio_probe(cap_slot: u64) -> u64 {
    syscall3(SYS_VIRTIO_DEVICE_PROBE, cap_slot, 0, 0)
}

pub fn virtio_report(cap_slot: u64, report: &VirtioDriverReport) -> u64 {
    syscall3(
        SYS_VIRTIO_DEVICE_REPORT,
        cap_slot,
        report as *const VirtioDriverReport as u64,
        core::mem::size_of::<VirtioDriverReport>() as u64,
    )
}

#[repr(C)]
pub struct VirtioDriverReport {
    pub queue_size: u64,
    pub avail_idx: u64,
    pub used_idx: u64,
    pub submissions: u64,
    pub completions: u64,
    pub timeouts: u64,
    pub reset_count: u64,
    pub last_error: u64,
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
