#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_KEYBOARD_EVENTS: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_PS2_IO: u64 = 3;
const CAP_PS2_IRQ: u64 = 4;
const PS2_DATA: u64 = 0x60;
const PS2_STATUS: u64 = 0x64;
const PS2_COMMAND: u64 = 0x64;
const PS2_STATUS_OUTPUT_FULL: u64 = 1;
const PS2_STATUS_INPUT_FULL: u64 = 1 << 1;
const PS2_CMD_ENABLE_FIRST_PORT: u8 = 0xae;
const PS2_KEYBOARD_ENABLE_SCANNING: u8 = 0xf4;
const SCANCODE_EXTENDED: u8 = 0xe0;
const SCANCODE_RELEASE: u8 = 0x80;
const SCANCODE_LEFT_SHIFT: u8 = 0x2a;
const SCANCODE_RIGHT_SHIFT: u8 = 0x36;
const SCANCODE_CAPS_LOCK: u8 = 0x3a;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

struct KeyboardState {
    shift: bool,
    caps_lock: bool,
    extended: bool,
}

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"keyboard-driver ready");
    init_ps2_keyboard();
    send_ready();

    let mut state = KeyboardState {
        shift: false,
        caps_lock: false,
        extended: false,
    };
    loop {
        let wait = sys::irq_wait(CAP_PS2_IRQ, 1000);
        if wait == sys::STATUS_BAD_CAPABILITY {
            log(b"keyboard-driver IRQ wait failed");
            sys::exit(1);
        }
        let mut delivered = false;
        while ps2_output_full() {
            let scancode = read_data();
            if let Some(byte) = decode_scancode(&mut state, scancode) {
                send_key(byte);
                delivered = true;
            }
        }
        if !delivered {
            let _ = sys::yield_now();
        }
    }
}

fn init_ps2_keyboard() {
    flush_output();
    write_command(PS2_CMD_ENABLE_FIRST_PORT);
    write_data(PS2_KEYBOARD_ENABLE_SCANNING);
    flush_output();
    log(b"keyboard-driver initialized ps2 keyboard");
}

fn decode_scancode(state: &mut KeyboardState, scancode: u8) -> Option<u8> {
    if scancode == SCANCODE_EXTENDED {
        state.extended = true;
        return None;
    }
    if state.extended {
        state.extended = false;
        return None;
    }

    let released = scancode & SCANCODE_RELEASE != 0;
    let code = scancode & !SCANCODE_RELEASE;
    match code {
        SCANCODE_LEFT_SHIFT | SCANCODE_RIGHT_SHIFT => {
            state.shift = !released;
            return None;
        }
        SCANCODE_CAPS_LOCK if !released => {
            state.caps_lock = !state.caps_lock;
            return None;
        }
        _ if released => return None,
        _ => {}
    }

    scancode_to_ascii(code, state.shift, state.caps_lock)
}

fn scancode_to_ascii(code: u8, shift: bool, caps_lock: bool) -> Option<u8> {
    let alpha_shift = shift ^ caps_lock;
    match code {
        0x02 => Some(if shift { b'!' } else { b'1' }),
        0x03 => Some(if shift { b'@' } else { b'2' }),
        0x04 => Some(if shift { b'#' } else { b'3' }),
        0x05 => Some(if shift { b'$' } else { b'4' }),
        0x06 => Some(if shift { b'%' } else { b'5' }),
        0x07 => Some(if shift { b'^' } else { b'6' }),
        0x08 => Some(if shift { b'&' } else { b'7' }),
        0x09 => Some(if shift { b'*' } else { b'8' }),
        0x0a => Some(if shift { b'(' } else { b'9' }),
        0x0b => Some(if shift { b')' } else { b'0' }),
        0x0c => Some(if shift { b'_' } else { b'-' }),
        0x0d => Some(if shift { b'+' } else { b'=' }),
        0x0e => Some(8),
        0x0f => Some(b'\t'),
        0x10 => Some(letter(b'q', alpha_shift)),
        0x11 => Some(letter(b'w', alpha_shift)),
        0x12 => Some(letter(b'e', alpha_shift)),
        0x13 => Some(letter(b'r', alpha_shift)),
        0x14 => Some(letter(b't', alpha_shift)),
        0x15 => Some(letter(b'y', alpha_shift)),
        0x16 => Some(letter(b'u', alpha_shift)),
        0x17 => Some(letter(b'i', alpha_shift)),
        0x18 => Some(letter(b'o', alpha_shift)),
        0x19 => Some(letter(b'p', alpha_shift)),
        0x1a => Some(if shift { b'{' } else { b'[' }),
        0x1b => Some(if shift { b'}' } else { b']' }),
        0x1c => Some(b'\n'),
        0x1e => Some(letter(b'a', alpha_shift)),
        0x1f => Some(letter(b's', alpha_shift)),
        0x20 => Some(letter(b'd', alpha_shift)),
        0x21 => Some(letter(b'f', alpha_shift)),
        0x22 => Some(letter(b'g', alpha_shift)),
        0x23 => Some(letter(b'h', alpha_shift)),
        0x24 => Some(letter(b'j', alpha_shift)),
        0x25 => Some(letter(b'k', alpha_shift)),
        0x26 => Some(letter(b'l', alpha_shift)),
        0x27 => Some(if shift { b':' } else { b';' }),
        0x28 => Some(if shift { b'"' } else { b'\'' }),
        0x29 => Some(if shift { b'~' } else { b'`' }),
        0x2b => Some(if shift { b'|' } else { b'\\' }),
        0x2c => Some(letter(b'z', alpha_shift)),
        0x2d => Some(letter(b'x', alpha_shift)),
        0x2e => Some(letter(b'c', alpha_shift)),
        0x2f => Some(letter(b'v', alpha_shift)),
        0x30 => Some(letter(b'b', alpha_shift)),
        0x31 => Some(letter(b'n', alpha_shift)),
        0x32 => Some(letter(b'm', alpha_shift)),
        0x33 => Some(if shift { b'<' } else { b',' }),
        0x34 => Some(if shift { b'>' } else { b'.' }),
        0x35 => Some(if shift { b'?' } else { b'/' }),
        0x39 => Some(b' '),
        _ => None,
    }
}

fn letter(lower: u8, uppercase: bool) -> u8 {
    if uppercase { lower - 32 } else { lower }
}

fn send_key(byte: u8) {
    let buffer = [byte];
    if sys::ipc_send(CAP_KEYBOARD_EVENTS, &buffer) != sys::STATUS_OK {
        log(b"keyboard-driver key send failed");
        sys::exit(1);
    }
}

fn flush_output() {
    let mut attempts = 0;
    while attempts < 256 && ps2_output_full() {
        let _ = read_data();
        attempts += 1;
    }
}

fn ps2_output_full() -> bool {
    let status = sys::io_read(CAP_PS2_IO, PS2_STATUS);
    if status == sys::STATUS_BAD_CAPABILITY {
        log(b"keyboard-driver PS2 status read failed");
        sys::exit(1);
    }
    status & PS2_STATUS_OUTPUT_FULL != 0
}

fn read_data() -> u8 {
    let value = sys::io_read(CAP_PS2_IO, PS2_DATA);
    if value == sys::STATUS_BAD_CAPABILITY {
        log(b"keyboard-driver PS2 data read failed");
        sys::exit(1);
    }
    value as u8
}

fn write_command(value: u8) {
    wait_input_clear();
    if sys::io_write(CAP_PS2_IO, PS2_COMMAND, value) != sys::STATUS_OK {
        log(b"keyboard-driver PS2 command write failed");
        sys::exit(1);
    }
}

fn write_data(value: u8) {
    wait_input_clear();
    if sys::io_write(CAP_PS2_IO, PS2_DATA, value) != sys::STATUS_OK {
        log(b"keyboard-driver PS2 data write failed");
        sys::exit(1);
    }
}

fn wait_input_clear() {
    let mut attempts = 0;
    while attempts < 100_000 {
        let status = sys::io_read(CAP_PS2_IO, PS2_STATUS);
        if status == sys::STATUS_BAD_CAPABILITY {
            log(b"keyboard-driver PS2 input wait failed");
            sys::exit(1);
        }
        if status & PS2_STATUS_INPUT_FULL == 0 {
            return;
        }
        attempts += 1;
    }
    log(b"keyboard-driver PS2 input wait timeout");
    sys::exit(1);
}

fn send_ready() {
    let ready = ready_message(b"keyboard-driver");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"keyboard-driver ready send failed");
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

fn log(value: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, value) != sys::STATUS_OK {
        sys::exit(1);
    }
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
    sys::exit(255)
}
