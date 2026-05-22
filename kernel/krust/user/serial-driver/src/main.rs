#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_SERIAL_INPUT: u64 = 0;
const CAP_COM1: u64 = 3;
const COM1: u64 = 0x3f8;
const COM1_LINE_STATUS: u64 = COM1 + 5;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write_line(b"serial-driver ready");
    write_line(b"serial-driver has COM1 I/O port capability");

    if sys::io_read(CAP_COM1, COM1_LINE_STATUS) == sys::STATUS_BAD_CAPABILITY {
        write_line(b"serial-driver COM1 read failed");
        sys::exit(1);
    }

    if sys::io_write(CAP_COM1, COM1, b'*') != sys::STATUS_OK {
        write_line(b"serial-driver COM1 write failed");
        sys::exit(1);
    }
    write_byte(b'\n');
    write_line(b"serial-driver can write byte");

    let mut buffer = [0u8; 96];
    let received = sys::ipc_recv(CAP_SERIAL_INPUT, &mut buffer);
    if received > buffer.len() as u64 {
        write_line(b"serial-driver receive failed");
        sys::exit(1);
    }
    let len = received as usize;
    write_bytes(&buffer[..len]);
    write_byte(b'\n');
    write_line(b"serial-driver writes message to COM1");

    sys::exit(0)
}

fn write_line(value: &[u8]) {
    write_bytes(value);
    write_byte(b'\n');
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
