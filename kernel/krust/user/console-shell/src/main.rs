#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};

const CAP_SHELL_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_CONSOLE_OUTPUT: u64 = 3;
const CAP_CONSOLE_CONTROL: u64 = 4;
const CAP_INSPECT: u64 = 5;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const REPORT_BUFFER_LEN: usize = 32 * 1024;
const CONTROL_SHUTDOWN: &[u8] = b"shutdown";
const SERVICE_NAMES: [&[u8]; 5] = [
    b"vertex-init",
    b"logd",
    b"vertex-store",
    b"vertex-state",
    b"console-shell",
];

struct ReportBuffer(UnsafeCell<[u8; REPORT_BUFFER_LEN]>);

unsafe impl Sync for ReportBuffer {}

static REPORT_BUFFER: ReportBuffer = ReportBuffer(UnsafeCell::new([0; REPORT_BUFFER_LEN]));

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"console-shell ready");
    send_ready();
    console_write(b"Vertex shell ready\n> ");

    loop {
        let mut command = [0u8; 96];
        let received = sys::ipc_recv(CAP_SHELL_REQUEST, &mut command);
        if received == sys::STATUS_BAD_CAPABILITY || received > command.len() as u64 {
            log(b"console-shell command receive failed");
            sys::exit(1);
        }
        let command = &command[..received as usize];
        if bytes_eq(command, b"help") {
            log(b"console-shell command: help");
            console_write(b"commands: generation services why halt\n> ");
            continue;
        }
        if bytes_eq(command, b"generation") {
            log(b"console-shell command: generation");
            let report = runtime_report();
            let generation = generation_for_self(report);
            console_write_generation(generation);
            continue;
        }
        if bytes_eq(command, b"services") {
            log(b"console-shell command: services");
            let report = runtime_report();
            console_write_services(report);
            continue;
        }
        if bytes_eq(command, b"why svc:echo cap:log.sink") {
            log(b"console-shell command: why");
            let report = runtime_report();
            require_echo_log_authority(report);
            console_write(
                b"svc:echo has send authority because generation graph granted cap slot 0\n> ",
            );
            continue;
        }
        if bytes_eq(command, b"halt") {
            log(b"console-shell command: halt");
            console_write(b"Native console shell ok\n");
            if sys::ipc_send(CAP_CONSOLE_CONTROL, CONTROL_SHUTDOWN) != sys::STATUS_OK {
                log(b"console-shell shutdown send failed");
                sys::exit(1);
            }
            sys::exit(0);
        }

        log(b"console-shell unknown command");
        console_write(b"unknown command\n> ");
    }
}

fn runtime_report() -> &'static [u8] {
    let report = report_buffer();
    let report_len = sys::runtime_inspect(CAP_INSPECT, report);
    if report_len == sys::STATUS_BAD_CAPABILITY
        || report_len == sys::STATUS_BAD_BUFFER
        || report_len == sys::STATUS_TOO_LARGE
        || report_len > report.len() as u64
    {
        log(b"console-shell runtime report failed");
        sys::exit(1);
    }
    &report[..report_len as usize]
}

fn report_buffer() -> &'static mut [u8; REPORT_BUFFER_LEN] {
    unsafe { &mut *REPORT_BUFFER.0.get() }
}

fn generation_for_self(report: &[u8]) -> &[u8] {
    let needles: [&[u8]; 2] = [b"name=console-shell", b" generation="];
    if let Some(line) = find_line_contains_all(report, &needles)
        && let Some(generation) = field_slice(line, b"generation=")
    {
        log(b"native shell generation query ok");
        return generation;
    }

    log(b"console-shell generation query failed");
    sys::exit(1);
}

fn console_write_generation(generation: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"current generation: ");
    append(&mut payload, &mut len, generation);
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
}

fn console_write_services(report: &[u8]) {
    let mut payload = [0u8; 128];
    let mut len = 0;
    append(&mut payload, &mut len, b"services:");
    let mut index = 0;
    while index < SERVICE_NAMES.len() {
        let state = process_state(report, SERVICE_NAMES[index]);
        log_service_state(SERVICE_NAMES[index], state);
        append(&mut payload, &mut len, b" ");
        append(&mut payload, &mut len, SERVICE_NAMES[index]);
        append(&mut payload, &mut len, b"=");
        append(&mut payload, &mut len, state);
        index += 1;
    }
    append(&mut payload, &mut len, b"\n> ");
    console_write(&payload[..len]);
    log(b"native shell services query ok");
}

fn process_state<'a>(report: &'a [u8], name: &[u8]) -> &'a [u8] {
    let mut start = 0;
    while start <= report.len() {
        let mut end = start;
        while end < report.len() && report[end] != b'\n' {
            end += 1;
        }
        let line = &report[start..end];
        if starts_with(line, b"process[")
            && field_eq(line, b"name=", name)
            && let Some(state) = field_slice(line, b"state=")
        {
            return state;
        }
        if end == report.len() {
            break;
        }
        start = end + 1;
    }

    log(b"console-shell services query failed");
    sys::exit(1);
}

fn require_echo_log_authority(report: &[u8]) {
    let needles: [&[u8]; 6] = [
        b"space=initial proc=echo cap[0] endpoint=log-sink",
        b"rights=send",
        b"parent_cap_id=",
        b"owner=echo",
        b"delegated_by=vertex-init",
        b"revoked=no",
    ];
    if let Some(line) = find_line_contains_all(report, &needles)
        && field_u64(line, b"parent_cap_id=").unwrap_or(0) != 0
    {
        log(b"native shell why query ok");
        log(b"console-shell why result: svc:echo cap:log.sink send slot 0");
        return;
    }

    log(b"console-shell why query failed");
    sys::exit(1);
}

fn console_write(payload: &[u8]) {
    if payload.len() > 128 {
        log(b"console-shell payload too large");
        sys::exit(1);
    }
    if sys::ipc_send(CAP_CONSOLE_OUTPUT, payload) != sys::STATUS_OK {
        log(b"console-shell console write failed");
        sys::exit(1);
    }
}

fn append(buffer: &mut [u8], len: &mut usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() {
        if *len >= buffer.len() {
            log(b"console-shell payload too large");
            sys::exit(1);
        }
        buffer[*len] = value[index];
        *len += 1;
        index += 1;
    }
}

fn log(value: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, value) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_service_state(name: &[u8], state: &[u8]) {
    let mut buffer = [0u8; 128];
    let mut len = 0;
    append(&mut buffer, &mut len, b"console-shell service state: ");
    append(&mut buffer, &mut len, name);
    append(&mut buffer, &mut len, b"=");
    append(&mut buffer, &mut len, state);
    log(&buffer[..len]);
}

fn send_ready() {
    let ready = ready_message(b"console-shell");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"console-shell ready send failed");
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

fn find_line_contains_all<'a>(haystack: &'a [u8], needles: &[&[u8]]) -> Option<&'a [u8]> {
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        let line = &haystack[start..end];
        if contains_all(line, needles) {
            return Some(line);
        }
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn contains_all(haystack: &[u8], needles: &[&[u8]]) -> bool {
    let mut index = 0;
    while index < needles.len() {
        if find_subslice(haystack, needles[index]).is_none() {
            return false;
        }
        index += 1;
    }
    true
}

fn field_eq(line: &[u8], prefix: &[u8], expected: &[u8]) -> bool {
    field_slice(line, prefix).is_some_and(|value| bytes_eq(value, expected))
}

fn field_slice<'a>(line: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    let start = find_subslice(line, prefix)? + prefix.len();
    let mut end = start;
    while end < line.len() && line[end] != b' ' && line[end] != b'\n' {
        end += 1;
    }
    Some(&line[start..end])
}

fn field_u64(line: &[u8], prefix: &[u8]) -> Option<u64> {
    let value = field_slice(line, prefix)?;
    let mut out = 0u64;
    if value.is_empty() {
        return None;
    }
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
        index += 1;
    }
    Some(out)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        let mut index = 0;
        while index < needle.len() && haystack[start + index] == needle[index] {
            index += 1;
        }
        if index == needle.len() {
            return Some(start);
        }
        start += 1;
    }
    None
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if value[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
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
