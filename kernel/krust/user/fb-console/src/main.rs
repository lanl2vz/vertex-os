#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_CONSOLE_OUTPUT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_CONSOLE_CONTROL: u64 = 3;
const CAP_KEYBOARD_EVENTS: u64 = 4;
const CAP_SHELL_REQUEST: u64 = 5;
const CAP_FRAMEBUFFER: u64 = 6;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const CONTROL_SHUTDOWN: &[u8] = b"shutdown";
const LOGD_PROOF_OUTPUT: &[u8] = b"logd sends log message";
const CELL_WIDTH: u64 = 12;
const CELL_HEIGHT: u64 = 16;
const GLYPH_SCALE: u64 = 2;
const GLYPH_WIDTH: usize = 5;
const GLYPH_HEIGHT: usize = 7;
const MARGIN_X: u64 = 8;
const MARGIN_Y: u64 = 8;
const MAX_COLS: usize = 160;
const MAX_ROWS: usize = 64;
const INTERACTIVE_QUIET: bool = option_env!("KRUST_INTERACTIVE_QUIET").is_some();

#[derive(Clone, Copy)]
struct Framebuffer {
    base: u64,
    length: u64,
    width: u64,
    height: u64,
    pitch: u64,
    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

struct Terminal {
    fb: Framebuffer,
    cols: usize,
    rows: usize,
    cursor_col: usize,
    cursor_row: usize,
    fg: u32,
    bg: u32,
    accent: u32,
}

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let fb = map_framebuffer();
    let mut terminal = Terminal::new(fb);
    let mut input = [0u8; 96];
    let mut input_len = 0;

    terminal.clear_screen();
    log(b"fb-console ready");
    send_ready();

    loop {
        let mut progressed = false;
        progressed |= drain_console_output(&mut terminal);
        if receive_shutdown() {
            drain_console_output(&mut terminal);
            log(b"fb-console shutdown requested");
            sys::exit(0);
        }
        progressed |= drain_keyboard(&mut terminal, &mut input, &mut input_len);
        if !progressed {
            let _ = sys::yield_now();
        }
    }
}

fn map_framebuffer() -> Framebuffer {
    let mut info = [0u8; 64];
    let result = sys::framebuffer_map(CAP_FRAMEBUFFER, &mut info);
    if result != 64 {
        log(b"fb-console framebuffer map failed");
        sys::exit(1);
    }

    let base = read_u64(&info, 0);
    let length = read_u64(&info, 8);
    let width = read_u64(&info, 16);
    let height = read_u64(&info, 24);
    let pitch = read_u64(&info, 32);
    let bpp = read_u64(&info, 40);
    let masks = read_u64(&info, 48);
    if base == 0 || length == 0 || width == 0 || height == 0 || pitch == 0 || bpp != 32 {
        log(b"fb-console unsupported framebuffer info");
        sys::exit(1);
    }
    let red_size = (masks & 0xff) as u8;
    let red_shift = ((masks >> 8) & 0xff) as u8;
    let green_size = ((masks >> 16) & 0xff) as u8;
    let green_shift = ((masks >> 24) & 0xff) as u8;
    let blue_size = ((masks >> 32) & 0xff) as u8;
    let blue_shift = ((masks >> 40) & 0xff) as u8;
    if red_size != 8 || green_size != 8 || blue_size != 8 {
        log(b"fb-console unsupported framebuffer masks");
        sys::exit(1);
    }

    Framebuffer {
        base,
        length,
        width,
        height,
        pitch,
        red_shift,
        green_shift,
        blue_shift,
    }
}

fn drain_console_output(terminal: &mut Terminal) -> bool {
    let mut progressed = false;
    loop {
        let mut buffer = [0u8; 128];
        let received = sys::ipc_recv_timeout(CAP_CONSOLE_OUTPUT, &mut buffer, 1);
        if received == sys::STATUS_TIMEOUT || received == sys::STATUS_EMPTY {
            return progressed;
        }
        if received == sys::STATUS_BAD_CAPABILITY || received > buffer.len() as u64 {
            log(b"fb-console output receive failed");
            sys::exit(1);
        }
        let payload = &buffer[..received as usize];
        if !(INTERACTIVE_QUIET && bytes_eq(payload, LOGD_PROOF_OUTPUT)) {
            terminal.write_bytes(payload);
        }
        progressed = true;
    }
}

fn receive_shutdown() -> bool {
    let mut buffer = [0u8; 16];
    let received = sys::ipc_recv_timeout(CAP_CONSOLE_CONTROL, &mut buffer, 1);
    if received == sys::STATUS_TIMEOUT || received == sys::STATUS_EMPTY {
        return false;
    }
    if received == sys::STATUS_BAD_CAPABILITY || received > buffer.len() as u64 {
        log(b"fb-console control receive failed");
        sys::exit(1);
    }
    if !bytes_eq(&buffer[..received as usize], CONTROL_SHUTDOWN) {
        log(b"fb-console unknown control message");
        sys::exit(1);
    }
    true
}

fn drain_keyboard(terminal: &mut Terminal, input: &mut [u8; 96], input_len: &mut usize) -> bool {
    let mut progressed = false;
    loop {
        let mut buffer = [0u8; 16];
        let received = sys::ipc_recv_timeout(CAP_KEYBOARD_EVENTS, &mut buffer, 1);
        if received == sys::STATUS_TIMEOUT || received == sys::STATUS_EMPTY {
            return progressed;
        }
        if received == sys::STATUS_BAD_CAPABILITY || received > buffer.len() as u64 {
            log(b"fb-console keyboard receive failed");
            sys::exit(1);
        }
        let mut index = 0;
        while index < received as usize {
            handle_input_byte(terminal, input, input_len, buffer[index]);
            index += 1;
        }
        progressed = true;
    }
}

fn handle_input_byte(
    terminal: &mut Terminal,
    input: &mut [u8; 96],
    input_len: &mut usize,
    byte: u8,
) {
    match byte {
        b'\r' | b'\n' => {
            terminal.write_byte(b'\n');
            if *input_len != 0 {
                send_shell_command(&input[..*input_len]);
                *input_len = 0;
            }
        }
        8 | 127 => {
            if *input_len != 0 {
                *input_len -= 1;
                terminal.backspace();
            }
        }
        _ => {
            if !byte.is_ascii_graphic() && byte != b' ' {
                return;
            }
            if *input_len >= input.len() {
                log(b"fb-console input line too long");
                *input_len = 0;
                terminal.write_byte(b'\n');
                return;
            }
            input[*input_len] = byte;
            *input_len += 1;
            terminal.write_byte(byte);
        }
    }
}

fn send_shell_command(command: &[u8]) {
    if sys::ipc_send(CAP_SHELL_REQUEST, command) != sys::STATUS_OK {
        log(b"fb-console shell command send failed");
        sys::exit(1);
    }
}

impl Terminal {
    fn new(fb: Framebuffer) -> Self {
        let cols = clamp_usize(
            fb.width.saturating_sub(MARGIN_X * 2) / CELL_WIDTH,
            1,
            MAX_COLS,
        );
        let rows = clamp_usize(
            fb.height.saturating_sub(MARGIN_Y * 2) / CELL_HEIGHT,
            1,
            MAX_ROWS,
        );
        let fg = fb.rgb(226, 232, 240);
        let bg = fb.rgb(11, 18, 32);
        let accent = fb.rgb(125, 211, 252);
        Self {
            fb,
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
            fg,
            bg,
            accent,
        }
    }

    fn clear_screen(&mut self) {
        self.fill_rect(0, 0, self.fb.width, self.fb.height, self.bg);
        self.fill_rect(0, 0, self.fb.width, 4, self.accent);
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            self.write_byte(bytes[index]);
            index += 1;
        }
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\r' => self.cursor_col = 0,
            b'\n' => self.newline(),
            8 => self.backspace(),
            _ => {
                if !byte.is_ascii_graphic() && byte != b' ' {
                    return;
                }
                if self.cursor_col >= self.cols {
                    self.newline();
                }
                self.draw_cell(self.cursor_col, self.cursor_row, byte);
                self.cursor_col += 1;
            }
        }
    }

    fn backspace(&mut self) {
        if self.cursor_col == 0 {
            return;
        }
        self.cursor_col -= 1;
        self.clear_cell(self.cursor_col, self.cursor_row);
    }

    fn newline(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;
        if self.cursor_row >= self.rows {
            self.scroll();
            self.cursor_row = self.rows - 1;
        }
    }

    fn scroll(&mut self) {
        let top = MARGIN_Y;
        let bottom = MARGIN_Y + self.rows as u64 * CELL_HEIGHT;
        let bytes_per_pixel = 4;
        let copy_rows = bottom.saturating_sub(top + CELL_HEIGHT);
        let mut row = 0;
        while row < copy_rows {
            let src = self.fb.base + (top + CELL_HEIGHT + row) * self.fb.pitch + MARGIN_X * 4;
            let dst = self.fb.base + (top + row) * self.fb.pitch + MARGIN_X * 4;
            let bytes = self.cols as u64 * CELL_WIDTH * bytes_per_pixel;
            let mut offset = 0;
            while offset < bytes {
                let value = unsafe { core::ptr::read_volatile((src + offset) as *const u32) };
                unsafe {
                    core::ptr::write_volatile((dst + offset) as *mut u32, value);
                }
                offset += 4;
            }
            row += 1;
        }
        self.fill_rect(
            MARGIN_X,
            top + copy_rows,
            self.cols as u64 * CELL_WIDTH,
            CELL_HEIGHT,
            self.bg,
        );
    }

    fn draw_cell(&mut self, col: usize, row: usize, byte: u8) {
        self.clear_cell(col, row);
        let glyph = glyph_rows(byte);
        let x = MARGIN_X + col as u64 * CELL_WIDTH;
        let y = MARGIN_Y + row as u64 * CELL_HEIGHT;
        let mut gy = 0;
        while gy < GLYPH_HEIGHT {
            let bits = glyph[gy];
            let mut gx = 0;
            while gx < GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - gx)) != 0 {
                    self.fill_rect(
                        x + gx as u64 * GLYPH_SCALE,
                        y + gy as u64 * GLYPH_SCALE,
                        GLYPH_SCALE,
                        GLYPH_SCALE,
                        self.fg,
                    );
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    fn clear_cell(&mut self, col: usize, row: usize) {
        let x = MARGIN_X + col as u64 * CELL_WIDTH;
        let y = MARGIN_Y + row as u64 * CELL_HEIGHT;
        self.fill_rect(x, y, CELL_WIDTH, CELL_HEIGHT, self.bg);
    }

    fn fill_rect(&mut self, x: u64, y: u64, width: u64, height: u64, color: u32) {
        let max_x = min_u64(x.saturating_add(width), self.fb.width);
        let max_y = min_u64(y.saturating_add(height), self.fb.height);
        let mut py = y;
        while py < max_y {
            let mut px = x;
            while px < max_x {
                self.fb.write_pixel(px, py, color);
                px += 1;
            }
            py += 1;
        }
    }
}

impl Framebuffer {
    fn rgb(self, r: u8, g: u8, b: u8) -> u32 {
        ((r as u32) << self.red_shift)
            | ((g as u32) << self.green_shift)
            | ((b as u32) << self.blue_shift)
    }

    fn write_pixel(self, x: u64, y: u64, color: u32) {
        let offset = y
            .saturating_mul(self.pitch)
            .saturating_add(x.saturating_mul(4));
        if offset + 4 > self.length {
            return;
        }
        unsafe {
            core::ptr::write_volatile((self.base + offset) as *mut u32, color);
        }
    }
}

fn glyph_rows(byte: u8) -> [u8; 7] {
    match byte {
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [6, 8, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 2, 12],
        b'A' | b'a' => [14, 17, 17, 31, 17, 17, 17],
        b'B' | b'b' => [30, 17, 17, 30, 17, 17, 30],
        b'C' | b'c' => [14, 17, 16, 16, 16, 17, 14],
        b'D' | b'd' => [30, 17, 17, 17, 17, 17, 30],
        b'E' | b'e' => [31, 16, 16, 30, 16, 16, 31],
        b'F' | b'f' => [31, 16, 16, 30, 16, 16, 16],
        b'G' | b'g' => [14, 17, 16, 23, 17, 17, 15],
        b'H' | b'h' => [17, 17, 17, 31, 17, 17, 17],
        b'I' | b'i' => [14, 4, 4, 4, 4, 4, 14],
        b'J' | b'j' => [7, 2, 2, 2, 18, 18, 12],
        b'K' | b'k' => [17, 18, 20, 24, 20, 18, 17],
        b'L' | b'l' => [16, 16, 16, 16, 16, 16, 31],
        b'M' | b'm' => [17, 27, 21, 21, 17, 17, 17],
        b'N' | b'n' => [17, 25, 21, 19, 17, 17, 17],
        b'O' | b'o' => [14, 17, 17, 17, 17, 17, 14],
        b'P' | b'p' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' | b'q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' | b'r' => [30, 17, 17, 30, 20, 18, 17],
        b'S' | b's' => [15, 16, 16, 14, 1, 1, 30],
        b'T' | b't' => [31, 4, 4, 4, 4, 4, 4],
        b'U' | b'u' => [17, 17, 17, 17, 17, 17, 14],
        b'V' | b'v' => [17, 17, 17, 17, 17, 10, 4],
        b'W' | b'w' => [17, 17, 17, 21, 21, 21, 10],
        b'X' | b'x' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' | b'y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' | b'z' => [31, 1, 2, 4, 8, 16, 31],
        b' ' => [0, 0, 0, 0, 0, 0, 0],
        b':' => [0, 4, 4, 0, 4, 4, 0],
        b';' => [0, 4, 4, 0, 4, 4, 8],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b',' => [0, 0, 0, 0, 4, 4, 8],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'/' => [1, 1, 2, 4, 8, 16, 16],
        b'\\' => [16, 16, 8, 4, 2, 1, 1],
        b'>' => [16, 8, 4, 2, 4, 8, 16],
        b'<' => [1, 2, 4, 8, 4, 2, 1],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        b'|' => [4, 4, 4, 4, 4, 4, 4],
        b'*' => [0, 21, 14, 31, 14, 21, 0],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'"' => [10, 10, 0, 0, 0, 0, 0],
        b'#' => [10, 31, 10, 10, 31, 10, 0],
        b'%' => [17, 2, 4, 8, 16, 17, 0],
        b'&' => [12, 18, 20, 8, 21, 18, 13],
        b'@' => [14, 17, 23, 21, 23, 16, 14],
        _ => [31, 17, 21, 17, 21, 17, 31],
    }
}

fn send_ready() {
    let ready = ready_message(b"fb-console");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"fb-console ready send failed");
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

fn read_u64(buffer: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = buffer[offset + index];
        index += 1;
    }
    u64::from_le_bytes(bytes)
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

fn clamp_usize(value: u64, min: usize, max: usize) -> usize {
    let mut value = value as usize;
    if value < min {
        value = min;
    }
    if value > max {
        value = max;
    }
    value
}

fn min_u64(left: u64, right: u64) -> u64 {
    if left < right { left } else { right }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(255)
}
