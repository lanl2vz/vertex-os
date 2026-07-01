#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_CONSOLE_OUTPUT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_CONSOLE_CONTROL: u64 = 3;
const CAP_SHELL_REQUEST: u64 = 4;
const CAP_COM1: u64 = 5;
const COM1: u64 = 0x3f8;
const COM1_LINE_STATUS: u64 = COM1 + 5;
const LINE_STATUS_DATA_READY: u64 = 1;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const CONTROL_SHUTDOWN: &[u8] = b"shutdown";
const LOGD_PROOF_OUTPUT: &[u8] = b"logd sends log message";
const INTERACTIVE_QUIET: bool = option_env!("KRUST_INTERACTIVE_QUIET").is_some();
const INPUT_BUFFER_LEN: usize = 160;
const SHELL_COMMAND_SEND_ATTEMPTS: u64 = 4096;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut input = [0u8; INPUT_BUFFER_LEN];
    let mut input_len = 0;

    log(b"console-driver ready");
    log(b"console-driver has COM1 I/O port capability");

    if sys::io_read(CAP_COM1, COM1_LINE_STATUS) == sys::STATUS_BAD_CAPABILITY {
        log(b"console-driver COM1 read failed");
        sys::exit(1);
    }

    if !INTERACTIVE_QUIET {
        if sys::io_write(CAP_COM1, COM1, b'>') != sys::STATUS_OK {
            log(b"console-driver COM1 write failed");
            sys::exit(1);
        }
        write_byte(b'\n');
        log(b"console-driver can write byte");
    }
    send_ready();

    loop {
        drain_console_output();
        if receive_shutdown() {
            drain_console_output();
            log(b"console-driver shutdown requested");
            sys::exit(0);
        }
        if poll_serial_input(&mut input, &mut input_len) {
            continue;
        }
        let _ = sys::yield_now();
    }
}

fn drain_console_output() {
    loop {
        let mut buffer = [0u8; 128];
        let received = sys::ipc_recv_timeout(CAP_CONSOLE_OUTPUT, &mut buffer, 1);
        if received == sys::STATUS_TIMEOUT || received == sys::STATUS_EMPTY {
            return;
        }
        if received == sys::STATUS_BAD_CAPABILITY || received > buffer.len() as u64 {
            log(b"console-driver output receive failed");
            sys::exit(1);
        }
        let payload = &buffer[..received as usize];
        if INTERACTIVE_QUIET && bytes_eq(payload, LOGD_PROOF_OUTPUT) {
            return;
        }
        mirror_console_lines(payload);
        write_bytes(payload);
        log(b"console-driver wrote console output");
    }
}

fn receive_shutdown() -> bool {
    let mut buffer = [0u8; 16];
    let received = sys::ipc_recv_timeout(CAP_CONSOLE_CONTROL, &mut buffer, 1);
    if received == sys::STATUS_TIMEOUT || received == sys::STATUS_EMPTY {
        return false;
    }
    if received == sys::STATUS_BAD_CAPABILITY || received > buffer.len() as u64 {
        log(b"console-driver control receive failed");
        sys::exit(1);
    }
    if !bytes_eq(&buffer[..received as usize], CONTROL_SHUTDOWN) {
        log(b"console-driver unknown control message");
        sys::exit(1);
    }
    true
}

fn poll_serial_input(input: &mut [u8; INPUT_BUFFER_LEN], input_len: &mut usize) -> bool {
    let status = sys::io_read(CAP_COM1, COM1_LINE_STATUS);
    if status == sys::STATUS_BAD_CAPABILITY {
        log(b"console-driver COM1 status read failed");
        sys::exit(1);
    }
    if status & LINE_STATUS_DATA_READY == 0 {
        return false;
    }

    let value = sys::io_read(CAP_COM1, COM1);
    if value == sys::STATUS_BAD_CAPABILITY {
        log(b"console-driver COM1 byte read failed");
        sys::exit(1);
    }
    let byte = value as u8;
    match byte {
        b'\r' | b'\n' => {
            write_byte(b'\n');
            if *input_len != 0 {
                send_shell_command(&input[..*input_len]);
                *input_len = 0;
                return true;
            }
        }
        8 | 127 => {
            if *input_len != 0 {
                *input_len -= 1;
                write_bytes(b"\x08 \x08");
            }
        }
        _ => {
            if !byte.is_ascii_graphic() && byte != b' ' {
                return true;
            }
            if *input_len >= input.len() {
                log(b"console-driver input line too long");
                *input_len = 0;
                write_byte(b'\n');
                return true;
            }
            input[*input_len] = byte;
            *input_len += 1;
            write_byte(byte);
        }
    }
    false
}

fn send_shell_command(command: &[u8]) {
    let mut attempts = 0;
    loop {
        let status = sys::ipc_send(CAP_SHELL_REQUEST, command);
        if status == sys::STATUS_OK {
            break;
        }
        if status == sys::STATUS_TOO_LARGE && attempts < SHELL_COMMAND_SEND_ATTEMPTS {
            attempts += 1;
            let _ = sys::yield_now();
            continue;
        }
        log(b"console-driver shell command send failed");
        sys::exit(1);
    }
    log_prefix(b"console-driver forwarded serial command: ", command);
}

fn log(value: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, value) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let mut len = 0;
    append(&mut buffer, &mut len, prefix);
    append(&mut buffer, &mut len, value);
    log(&buffer[..len]);
}

fn mirror_console_lines(payload: &[u8]) {
    let mut start = 0;
    let mut index = 0;
    while index < payload.len() {
        if payload[index] == b'\n' {
            log_console_line(&payload[start..index]);
            start = index + 1;
        }
        index += 1;
    }
}

fn log_console_line(line: &[u8]) {
    let mut end = line.len();
    while end != 0 && line[end - 1] == b'\r' {
        end -= 1;
    }
    let line = &line[..end];
    if line.is_empty() || bytes_eq(line, b">") || bytes_eq(line, b"> ") {
        return;
    }
    log(line);
}

fn append(buffer: &mut [u8], len: &mut usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() && *len < buffer.len() {
        buffer[*len] = value[index];
        *len += 1;
        index += 1;
    }
}

fn send_ready() {
    let ready = ready_message(b"console-driver");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"console-driver ready send failed");
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

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
