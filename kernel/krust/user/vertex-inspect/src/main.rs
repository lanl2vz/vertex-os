#![no_std]
#![no_main]

mod sys;

use core::{cell::UnsafeCell, panic::PanicInfo};

const CAP_INSPECT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_MANIFEST: u64 = 3;
const KRUSTBOOT_MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const KRUSTBOOT_VERSION: u16 = 4;
const MANIFEST_BUFFER_LEN: usize = 16 * 1024;
const REPORT_BUFFER_LEN: usize = 32 * 1024;
const OFFSET_VERSION: usize = 16;
const OFFSET_PROCESSES: usize = 20;
const OFFSET_ENDPOINTS: usize = 22;
const OFFSET_GENERATION_ID: usize = 40;
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
    explain_cap_provenance(report);

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
    if contains(report, b"proc=echo cap[0] endpoint=log-sink rights=send")
        && contains(report, b"parent_cap_id=")
    {
        log(b"why: echo can send to log-sink because delegated endpoint authority has send rights");
        return;
    }

    log(b"vertex-inspect why query failed");
    sys::exit(1);
}

fn explain_state_counter(report: &[u8]) {
    log(b"native who-can state:counter");
    if contains(
        report,
        b"proc=vertex-state cap[3] state-volume=state:counter rights=read|write|snapshot|restore",
    ) {
        log(b"who-can: vertex-state owns state:counter with rights=read|write|snapshot|restore");
        return;
    }

    log(b"vertex-inspect who-can query failed");
    sys::exit(1);
}

fn explain_cap_provenance(report: &[u8]) {
    log(b"native cap provenance report");
    if contains(report, b"space=initial proc=echo cap[0] endpoint=log-sink")
        && contains(report, b"parent_cap_id=")
        && contains(report, b"generation=")
    {
        log(b"cap provenance: echo log-sink cap is derived from vertex-init endpoint authority");
        return;
    }

    log(b"vertex-inspect provenance query failed");
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
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }

    let mut offset = 0;
    while offset + needle.len() <= haystack.len() {
        if bytes_eq(&haystack[offset..offset + needle.len()], needle) {
            return true;
        }
        offset += 1;
    }
    false
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
