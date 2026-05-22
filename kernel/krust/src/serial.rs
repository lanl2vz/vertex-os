use core::arch::asm;

const COM1: u16 = 0x3f8;
const INTERRUPT_ENABLE: u16 = COM1 + 1;
const FIFO_CONTROL: u16 = COM1 + 2;
const LINE_CONTROL: u16 = COM1 + 3;
const MODEM_CONTROL: u16 = COM1 + 4;
const LINE_STATUS: u16 = COM1 + 5;

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
    if byte == b'\n' {
        write_byte(b'\r');
    }

    while !transmit_empty() {}

    unsafe {
        outb(COM1, byte);
    }
}

fn transmit_empty() -> bool {
    unsafe { inb(LINE_STATUS) & 0x20 != 0 }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

unsafe fn inb(port: u16) -> u8 {
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
