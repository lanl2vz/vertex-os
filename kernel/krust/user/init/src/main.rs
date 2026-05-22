#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const KRUSTBOOT_MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const MANIFEST_BUFFER_LEN: usize = 2048;
const OFFSET_BOOT_MODULES: usize = 18;
const OFFSET_PROCESSES: usize = 20;
const OFFSET_ENDPOINTS: usize = 22;
const OFFSET_GRANTS: usize = 24;
const OFFSET_GENERATION_ID: usize = 26;
const STRING_LEN: usize = 64;
const BOOT_MODULE_RECORD_LEN: usize = STRING_LEN * 2;
const PROCESS_RECORD_LEN: usize = STRING_LEN * 2 + 4;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"vertex-init started");

    let mut manifest = [0u8; MANIFEST_BUFFER_LEN];
    let manifest_len = sys::read_manifest(&mut manifest);
    if manifest_len == sys::STATUS_BAD_CAPABILITY || manifest_len == sys::STATUS_BAD_BUFFER {
        log(b"vertex-init manifest read failed");
        sys::exit(1);
    }

    let Ok(manifest_len) = usize::try_from(manifest_len) else {
        log(b"vertex-init manifest length invalid");
        sys::exit(1);
    };

    if manifest_len < OFFSET_GENERATION_ID + STRING_LEN || !valid_magic(&manifest[..manifest_len]) {
        log(b"vertex-init manifest invalid");
        sys::exit(1);
    }

    let generation =
        fixed_string(&manifest[OFFSET_GENERATION_ID..OFFSET_GENERATION_ID + STRING_LEN]);

    log(b"vertex-init received cap[0]=manifest-read");
    log(b"vertex-init received cap[1]=serial-log");
    log(b"vertex-init received cap[2]=process-control");

    log_prefix(b"vertex-init manifest generation: ", generation);

    let boot_modules = read_u16(&manifest, OFFSET_BOOT_MODULES);
    let processes = read_u16(&manifest, OFFSET_PROCESSES);
    let endpoints = read_u16(&manifest, OFFSET_ENDPOINTS);
    let grants = read_u16(&manifest, OFFSET_GRANTS);

    log_count(b"vertex-init boot modules: ", boot_modules);
    log_count(b"vertex-init processes: ", processes);
    log_count(b"vertex-init endpoints: ", endpoints);
    log_count(b"vertex-init grants: ", grants);

    let Some(logd_index) =
        process_index_by_name(&manifest[..manifest_len], boot_modules, processes, b"logd")
    else {
        log(b"vertex-init missing logd process");
        sys::exit(1);
    };
    let Some(echo_index) =
        process_index_by_name(&manifest[..manifest_len], boot_modules, processes, b"echo")
    else {
        log(b"vertex-init missing echo process");
        sys::exit(1);
    };

    start_service(b"logd", logd_index);
    sys::yield_now();

    start_service(b"echo", echo_index);
    sys::yield_now();
    sys::yield_now();

    log(b"Native service activation ok");
    sys::exit(0)
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

fn process_index_by_name(
    manifest: &[u8],
    boot_modules: u16,
    processes: u16,
    name: &[u8],
) -> Option<u64> {
    let base = OFFSET_GENERATION_ID + STRING_LEN + boot_modules as usize * BOOT_MODULE_RECORD_LEN;
    let mut index = 0;

    while index < processes as usize {
        let offset = base + index * PROCESS_RECORD_LEN;
        if offset + PROCESS_RECORD_LEN > manifest.len() {
            return None;
        }

        if fixed_string(&manifest[offset..offset + STRING_LEN]) == name {
            return Some(index as u64);
        }
        index += 1;
    }

    None
}

fn start_service(name: &[u8], process_index: u64) {
    log_prefix(b"vertex-init starting service: ", name);
    if sys::process_start(process_index) != sys::STATUS_OK {
        log_prefix(b"vertex-init service start failed: ", name);
        sys::exit(1);
    }
}

fn log(message: &[u8]) {
    if sys::log(message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
}

fn log_count(prefix: &[u8], value: u16) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append_decimal(&mut buffer, len, value as u64);
    log(&buffer[..len]);
}

fn append(buffer: &mut [u8], mut offset: usize, value: &[u8]) -> usize {
    let mut index = 0;
    while offset < buffer.len() && index < value.len() {
        buffer[offset] = value[index];
        offset += 1;
        index += 1;
    }
    offset
}

fn append_decimal(buffer: &mut [u8], offset: usize, mut value: u64) -> usize {
    if value == 0 {
        return append(buffer, offset, b"0");
    }

    let mut digits = [0u8; 20];
    let mut len = 0;
    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }

    let mut out = offset;
    while len > 0 {
        len -= 1;
        out = append(buffer, out, &digits[len..len + 1]);
    }
    out
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
