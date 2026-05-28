#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};

const CAP_INSPECT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_MANIFEST: u64 = 3;
const KRUSTBOOT_MAGIC: &[u8; 16] = b"KRUSTBOOTM60\0\0\0\0";
const KRUSTBOOT_VERSION: u16 = 6;
const MANIFEST_BUFFER_LEN: usize = 16 * 1024;
const REPORT_BUFFER_LEN: usize = 64 * 1024;
const OFFSET_VERSION: usize = 16;
const OFFSET_PROCESSES: usize = 20;
const OFFSET_ENDPOINTS: usize = 22;
const OFFSET_GENERATION_ID: usize = 46;
const STRING_LEN: usize = 64;

struct ReportBuffer(UnsafeCell<[u8; REPORT_BUFFER_LEN]>);

unsafe impl Sync for ReportBuffer {}

static REPORT_BUFFER: ReportBuffer = ReportBuffer(UnsafeCell::new([0; REPORT_BUFFER_LEN]));

struct GenerationGraph {
    id: [u8; STRING_LEN],
    id_len: usize,
    processes: u16,
    endpoints: u16,
}

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"vertex-inspect started");

    let generation = read_generation_graph();
    log_generation_graph(
        &generation.id[..generation.id_len],
        generation.processes,
        generation.endpoints,
    );

    let report = report_buffer();
    let report_len = sys::runtime_inspect(CAP_INSPECT, report);
    if report_len == sys::STATUS_BAD_CAPABILITY
        || report_len == sys::STATUS_BAD_BUFFER
        || report_len == sys::STATUS_TOO_LARGE
        || report_len > report.len() as u64
    {
        log(b"vertex-inspect runtime report failed");
        sys::exit(1);
    }
    let report = &report[..report_len as usize];

    explain_echo_to_logd(report);
    explain_state_counter(report);
    explain_vertex_inspect_generation(report, &generation.id[..generation.id_len]);
    explain_derived_endpoint_caps(report);
    explain_cap_provenance(report);
    explain_config_authority(report);
    explain_secret_authority(report);

    log(b"Native introspection service ok");
    sys::exit(0)
}

fn report_buffer() -> &'static mut [u8; REPORT_BUFFER_LEN] {
    unsafe { &mut *REPORT_BUFFER.0.get() }
}

#[inline(never)]
fn read_generation_graph() -> GenerationGraph {
    let mut manifest = [0u8; MANIFEST_BUFFER_LEN];
    let manifest_len = sys::read_manifest(CAP_MANIFEST, &mut manifest);
    if manifest_len == sys::STATUS_BAD_CAPABILITY
        || manifest_len == sys::STATUS_BAD_BUFFER
        || manifest_len == sys::STATUS_TOO_LARGE
    {
        log(b"vertex-inspect manifest read failed");
        sys::exit(1);
    }
    let manifest_len = manifest_len as usize;
    if manifest_len < OFFSET_GENERATION_ID + STRING_LEN
        || !valid_magic(&manifest[..manifest_len])
        || read_u16(&manifest, OFFSET_VERSION) != KRUSTBOOT_VERSION
    {
        log(b"vertex-inspect manifest invalid");
        sys::exit(1);
    }

    let generation =
        fixed_string(&manifest[OFFSET_GENERATION_ID..OFFSET_GENERATION_ID + STRING_LEN]);
    let mut id = [0u8; STRING_LEN];
    let mut index = 0;
    while index < generation.len() {
        id[index] = generation[index];
        index += 1;
    }

    GenerationGraph {
        id,
        id_len: generation.len(),
        processes: read_u16(&manifest, OFFSET_PROCESSES),
        endpoints: read_u16(&manifest, OFFSET_ENDPOINTS),
    }
}

fn explain_echo_to_logd(report: &[u8]) {
    log(b"native why echo log-sink");
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
        log(b"why: echo can send to log-sink because delegated endpoint authority has send rights");
        return;
    }

    log(b"vertex-inspect why query failed");
    sys::exit(1);
}

fn explain_state_counter(report: &[u8]) {
    log(b"native who-can state:counter");
    let needles: [&[u8]; 5] = [
        b"space=initial proc=vertex-state cap[4]",
        b"endpoint=vertex-state-block-request",
        b"rights=send",
        b"owner=vertex-state",
        b"revoked=no",
    ];
    if find_line_contains_all(report, &needles).is_some() {
        log(b"who-can: vertex-state owns state:counter through VertexDisk block service authority");
        return;
    }

    log(b"vertex-inspect who-can query failed");
    sys::exit(1);
}

fn explain_vertex_inspect_generation(report: &[u8], generation: &[u8]) {
    log(b"native which-generation vertex-inspect");
    let needles: [&[u8]; 2] = [b"name=vertex-inspect", b" generation="];
    if let Some(line) = find_line_contains_all(report, &needles)
        && contains(line, generation)
    {
        log_prefix(b"generation: vertex-inspect started in ", generation);
        return;
    }

    log(b"vertex-inspect generation query failed");
    sys::exit(1);
}

fn explain_derived_endpoint_caps(report: &[u8]) {
    log(b"native delegated endpoint cap report");
    let needles: [&[u8]; 5] = [
        b"space=initial",
        b" endpoint=",
        b"parent_cap_id=",
        b"delegated_by=vertex-init",
        b"revoked=no",
    ];
    let count = log_lines_contains_all(report, &needles);
    if count == 0 {
        log(b"vertex-inspect delegated endpoint query failed");
        sys::exit(1);
    }

    let echo_needles: [&[u8]; 4] = [
        b"space=initial proc=echo cap[0] endpoint=log-sink",
        b"rights=send",
        b"parent_cap_id=",
        b"delegated_by=vertex-init",
    ];
    if let Some(line) = find_line_contains_all(report, &echo_needles)
        && field_u64(line, b"parent_cap_id=").unwrap_or(0) != 0
    {
        log_count(b"derived endpoint caps from vertex-init: ", count as u64);
        return;
    }

    log(b"vertex-inspect delegated endpoint query failed");
    sys::exit(1);
}

fn explain_cap_provenance(report: &[u8]) {
    log(b"native cap provenance report");
    let needles: [&[u8]; 6] = [
        b"space=initial proc=echo cap[0] endpoint=log-sink",
        b"rights=send",
        b"cap_id=",
        b"parent_cap_id=",
        b"generation=",
        b"delegated_by=vertex-init",
    ];
    if let Some(line) = find_line_contains_all(report, &needles)
        && field_u64(line, b"cap_id=").unwrap_or(0) != 0
        && field_u64(line, b"parent_cap_id=").unwrap_or(0) != 0
    {
        log(b"cap provenance: echo log-sink cap is derived from vertex-init endpoint authority");
        return;
    }

    log(b"vertex-inspect provenance query failed");
    sys::exit(1);
}

fn explain_config_authority(report: &[u8]) {
    let needles: [&[u8]; 4] = [
        b"space=initial proc=logd cap[4] config=config:logd",
        b"rights=read",
        b"owner=logd",
        b"revoked=no",
    ];
    if find_line_contains_all(report, &needles).is_some()
        && !contains(report, b"\"level\":\"info\"")
    {
        log(b"vertex-inspect shows config authority without dumping content");
        return;
    }

    log(b"vertex-inspect config authority query failed");
    sys::exit(1);
}

fn explain_secret_authority(report: &[u8]) {
    let needles: [&[u8]; 4] = [
        b"space=initial proc=logd cap[5] secret=secret:logd-token",
        b"rights=read|inspect-metadata",
        b"owner=logd",
        b"revoked=no",
    ];
    if find_line_contains_all(report, &needles).is_some()
        && !contains(report, b"native-secret-value")
    {
        log(b"vertex-inspect shows which services have secret access");
        log(b"vertex-inspect does not print secret value");
        return;
    }

    log(b"vertex-inspect secret authority query failed");
    sys::exit(1);
}

fn valid_magic(manifest: &[u8]) -> bool {
    manifest.len() >= KRUSTBOOT_MAGIC.len() && &manifest[..KRUSTBOOT_MAGIC.len()] == KRUSTBOOT_MAGIC
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn fixed_string(bytes: &[u8]) -> &[u8] {
    let mut len = 0;
    while len < bytes.len() && bytes[len] != 0 {
        len += 1;
    }
    &bytes[..len]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find_subslice(haystack, needle).is_some()
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

fn log_lines_contains_all(haystack: &[u8], needles: &[&[u8]]) -> usize {
    let mut count = 0;
    let mut start = 0;
    while start <= haystack.len() {
        let mut end = start;
        while end < haystack.len() && haystack[end] != b'\n' {
            end += 1;
        }
        let line = &haystack[start..end];
        if contains_all(line, needles) && field_u64(line, b"parent_cap_id=").unwrap_or(0) != 0 {
            log_derived_endpoint_cap(line);
            count += 1;
        }
        if end == haystack.len() {
            break;
        }
        start = end + 1;
    }
    count
}

fn log_derived_endpoint_cap(line: &[u8]) {
    let mut buffer = [0u8; 128];
    let mut len = append(&mut buffer, 0, b"derived endpoint cap: proc=");
    len = append_field(&mut buffer, len, line, b"proc=", b' ');
    len = append(&mut buffer, len, b" cap[");
    len = append_field(&mut buffer, len, line, b"cap[", b']');
    len = append(&mut buffer, len, b"] endpoint=");
    len = append_field(&mut buffer, len, line, b"endpoint=", b' ');
    log(&buffer[..len]);
}

fn append_field(
    buffer: &mut [u8],
    mut offset: usize,
    line: &[u8],
    key: &[u8],
    terminator: u8,
) -> usize {
    let Some(mut source) = find_subslice(line, key).map(|index| index + key.len()) else {
        return append(buffer, offset, b"<missing>");
    };
    while source < line.len() && line[source] != terminator && offset < buffer.len() {
        buffer[offset] = line[source];
        offset += 1;
        source += 1;
    }
    offset
}

fn contains_all(line: &[u8], needles: &[&[u8]]) -> bool {
    let mut index = 0;
    while index < needles.len() {
        if !contains(line, needles[index]) {
            return false;
        }
        index += 1;
    }
    true
}

fn field_u64(line: &[u8], key: &[u8]) -> Option<u64> {
    let mut offset = find_subslice(line, key)? + key.len();
    let mut value = 0u64;
    let mut saw_digit = false;
    while offset < line.len() && line[offset] >= b'0' && line[offset] <= b'9' {
        value = value
            .saturating_mul(10)
            .saturating_add((line[offset] - b'0') as u64);
        saw_digit = true;
        offset += 1;
    }
    if saw_digit { Some(value) } else { None }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }

    let mut offset = 0;
    while offset + needle.len() <= haystack.len() {
        if bytes_eq(&haystack[offset..offset + needle.len()], needle) {
            return Some(offset);
        }
        offset += 1;
    }
    None
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

fn log_generation_graph(generation: &[u8], processes: u16, endpoints: u16) {
    let mut buffer = [0u8; 128];
    let mut len = append(&mut buffer, 0, b"vertex-inspect generation graph: ");
    len = append(&mut buffer, len, generation);
    len = append(&mut buffer, len, b" processes=");
    len = append_u16(&mut buffer, len, processes);
    len = append(&mut buffer, len, b" endpoints=");
    len = append_u16(&mut buffer, len, endpoints);
    log(&buffer[..len]);
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn log_count(prefix: &[u8], value: u64) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append_u64(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn append(buffer: &mut [u8], mut offset: usize, value: &[u8]) -> usize {
    let mut index = 0;
    while index < value.len() && offset < buffer.len() {
        buffer[offset] = value[index];
        offset += 1;
        index += 1;
    }
    offset
}

fn append_u16(buffer: &mut [u8], offset: usize, value: u16) -> usize {
    append_u64(buffer, offset, value as u64)
}

fn append_u64(buffer: &mut [u8], mut offset: usize, mut value: u64) -> usize {
    if value == 0 {
        if offset < buffer.len() {
            buffer[offset] = b'0';
            offset += 1;
        }
        return offset;
    }

    let mut digits = [0u8; 20];
    let mut len = 0;
    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    while len > 0 {
        len -= 1;
        if offset < buffer.len() {
            buffer[offset] = digits[len];
            offset += 1;
        }
    }
    offset
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
