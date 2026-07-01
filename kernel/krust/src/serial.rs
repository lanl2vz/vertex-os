use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};

const COM1: u16 = 0x3f8;
const INTERRUPT_ENABLE: u16 = COM1 + 1;
const FIFO_CONTROL: u16 = COM1 + 2;
const LINE_CONTROL: u16 = COM1 + 3;
const MODEM_CONTROL: u16 = COM1 + 4;
const LINE_STATUS: u16 = COM1 + 5;
const INTERACTIVE_QUIET: bool = option_env!("KRUST_INTERACTIVE_QUIET").is_some();
static SUPPRESS_QUIET_LINE: AtomicBool = AtomicBool::new(false);

pub fn interactive_quiet() -> bool {
    INTERACTIVE_QUIET
}

pub fn trace_enabled() -> bool {
    !INTERACTIVE_QUIET
}

pub fn init() {
    unsafe {
        outb(INTERRUPT_ENABLE, 0x00);
        outb(LINE_CONTROL, 0x80);
        outb(COM1, 0x01);
        outb(INTERRUPT_ENABLE, 0x00);
        outb(LINE_CONTROL, 0x03);
        outb(FIFO_CONTROL, 0xc7);
        outb(MODEM_CONTROL, 0x0b);
    }
}

pub fn write_str(value: &str) {
    if quiet_line_filtered(value) {
        return;
    }

    for byte in value.bytes() {
        write_byte(byte);
    }
}

pub fn write_ascii_bytes(value: &[u8]) {
    for byte in value {
        if byte.is_ascii_graphic() || *byte == b' ' {
            write_byte(*byte);
        } else {
            write_byte(b'.');
        }
    }
}

pub fn write_c_string(value: *const u8) {
    if value.is_null() {
        write_str("<null>");
        return;
    }

    let mut index = 0;
    while index < 256 {
        let byte = unsafe { value.add(index).read() };
        if byte == 0 {
            return;
        }

        if byte.is_ascii_graphic() || byte == b' ' {
            write_byte(byte);
        } else {
            write_byte(b'.');
        }

        index += 1;
    }

    write_str("...");
}

pub fn write_u64_dec(mut value: u64) {
    if value == 0 {
        write_byte(b'0');
        return;
    }

    let mut buffer = [0u8; 20];
    let mut index = buffer.len();

    while value > 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }

    for byte in &buffer[index..] {
        write_byte(*byte);
    }
}

pub fn write_u64_hex(value: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    write_str("0x");
    for nibble in (0..16).rev() {
        let digit = ((value >> (nibble * 4)) & 0xf) as usize;
        write_byte(HEX[digit]);
    }
}

fn write_byte(byte: u8) {
    if INTERACTIVE_QUIET && SUPPRESS_QUIET_LINE.load(Ordering::Relaxed) {
        if byte == b'\n' {
            SUPPRESS_QUIET_LINE.store(false, Ordering::Relaxed);
        }
        return;
    }

    if byte == b'\n' {
        write_byte(b'\r');
    }

    while !transmit_empty() {}

    unsafe {
        outb(COM1, byte);
    }
}

fn quiet_line_filtered(value: &str) -> bool {
    if !INTERACTIVE_QUIET {
        return false;
    }

    if SUPPRESS_QUIET_LINE.load(Ordering::Relaxed) {
        if value.bytes().any(|byte| byte == b'\n') {
            SUPPRESS_QUIET_LINE.store(false, Ordering::Relaxed);
        }
        return true;
    }

    if quiet_line_prefix(value) {
        if !value.bytes().any(|byte| byte == b'\n') {
            SUPPRESS_QUIET_LINE.store(true, Ordering::Relaxed);
        }
        return true;
    }

    false
}

fn quiet_line_prefix(value: &str) -> bool {
    value.starts_with("  [")
        || value.starts_with("  boot_module[")
        || value.starts_with("  process[")
        || value.starts_with("    mount[")
        || value.starts_with("  endpoint[")
        || value.starts_with("  grant[")
        || value.starts_with("  store_object[")
        || value.starts_with("  state_volume[")
        || value.starts_with("  network_port[")
        || value.starts_with("  io_port[")
        || value.starts_with("  interrupt_line[")
        || value.starts_with("  dma_region[")
        || value.starts_with("  pci_device[")
        || value.starts_with("  virtio_device[")
        || value.starts_with("  namespace[")
        || value.starts_with("    path=")
        || value.starts_with("  vfs_root[")
        || value.starts_with("Bad pointer test:")
        || value.starts_with("Boot capability table entries:")
        || value.starts_with("Capability ")
        || value.starts_with("Derived VFS root released:")
        || value.starts_with("DMA map accepted:")
        || value.starts_with("Endpoint create ")
        || value.starts_with("IPC negative test:")
        || value.starts_with("Kernel object table entries:")
        || value.starts_with("Krust declared mount snapshot restored:")
        || value.starts_with("Krust native config hash verified:")
        || value.starts_with("Krust process ")
        || value.starts_with("Krust native store indexed")
        || value.starts_with("KrustBoot ")
        || value.starts_with("Legacy object-read syscall rejected")
        || value.starts_with("Limine memory map entries:")
        || value.starts_with("Limine modules:")
        || value.starts_with("Namespace resolve ")
        || value.starts_with("Network-port ")
        || value.starts_with("Native VFS state ")
        || value.starts_with("Native VertexFS device ")
        || value.starts_with("Native generation metadata block ")
        || value.starts_with("Native secret grant:")
        || value.starts_with("Physical allocator ")
        || value.starts_with("Process exited:")
        || value.starts_with("Quota delegate ")
        || value.starts_with("Runtime inspect accepted:")
        || value.starts_with("SYS_")
        || value.starts_with("Secret read accepted:")
        || value.starts_with("UDP send queued for netstack:")
        || value.starts_with("VFS ")
        || value.starts_with("VertexFS v1 fsync device transaction")
        || value.starts_with("Virtio ")
        || value.starts_with("network-port ")
        || value.starts_with("endpoint[")
        || value.starts_with("initial capability grants supplied")
        || value.starts_with("immutable launch object accepted")
        || value.starts_with("logd sends log message")
        || value.starts_with("proc=")
        || value.starts_with("process[")
        || value.starts_with("store hash verified")
        || value.starts_with("vertex-inspect security event:")
        || value.starts_with("virtio-net ")
}

fn transmit_empty() -> bool {
    unsafe { inb(LINE_STATUS) & 0x20 != 0 }
}

pub unsafe fn outb_raw(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

pub unsafe fn inb_raw(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

pub unsafe fn outw_raw(port: u16, value: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

pub unsafe fn inw_raw(port: u16) -> u16 {
    let value: u16;
    unsafe {
        asm!(
            "in ax, dx",
            in("dx") port,
            out("ax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

pub unsafe fn outl_raw(port: u16, value: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

pub unsafe fn inl_raw(port: u16) -> u32 {
    let value: u32;
    unsafe {
        asm!(
            "in eax, dx",
            in("dx") port,
            out("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        outb_raw(port, value);
    }
}

unsafe fn inb(port: u16) -> u8 {
    unsafe { inb_raw(port) }
}
