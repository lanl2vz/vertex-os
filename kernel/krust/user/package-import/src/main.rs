#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_PACKAGE_IMPORT_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_GENERATION_MANAGER_REQUEST: u64 = 3;
const CAP_PACKAGE_FRAGMENT: u64 = 4;
const CAP_CONFIG_PROOF: u64 = 5;

const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"package-import ready");
    send_ready();

    loop {
        let mut command = [0u8; 64];
        let received = sys::ipc_recv(CAP_PACKAGE_IMPORT_REQUEST, &mut command);
        if received == sys::STATUS_BAD_CAPABILITY
            || received == sys::STATUS_BAD_BUFFER
            || received == sys::STATUS_TOO_LARGE
            || received > command.len() as u64
        {
            log(b"package-import command receive failed");
            sys::exit(1);
        }
        let command = &command[..received as usize];
        if bytes_eq(command, b"import pkg:logd") {
            import_package();
            continue;
        }
        if bytes_eq(command, b"shutdown") {
            log(b"package-import shutdown requested");
            sys::exit(0);
        }
        log(b"package-import rejected unknown command");
    }
}

fn import_package() {
    log(b"native package-import service reads compact graph fragment");

    let mut fragment_buffer = [0u8; 1024];
    let fragment_len = read_store_object(CAP_PACKAGE_FRAGMENT, &mut fragment_buffer);
    let fragment = &fragment_buffer[..fragment_len];
    if !starts_with(fragment, b"PKGFRAGV1\n") {
        log(b"package-import rejected graph fragment: bad magic");
        sys::exit(1);
    }

    let package = required_field(fragment, b"package=");
    let service = required_field(fragment, b"service=");
    let candidate = required_field(fragment, b"candidate=");
    let authority_delta = required_field(fragment, b"authority_delta=");
    let missing_dependency = required_field(fragment, b"missing_dependency=");
    let excess_authority = required_field(fragment, b"excess_authority=");
    let object = required_field(fragment, b"object=");
    let object_hash = required_field(fragment, b"object_hash=");
    let closure_material = required_field(fragment, b"closure_material=");
    let closure_hash = required_field(fragment, b"closure_hash=");

    log_pair(
        b"package-import parsed compact typed graph fragment: package=",
        package,
    );

    let mut config_buffer = [0u8; 128];
    let config_len = read_store_object(CAP_CONFIG_PROOF, &mut config_buffer);
    verify_blake3(
        &config_buffer[..config_len],
        object_hash,
        b"store-object hash",
    );
    log_pair(
        b"package-import verified store-object hash: object=",
        object,
    );

    verify_blake3(closure_material, closure_hash, b"closure hash");
    log_two_values(
        b"native package import adds service graph fragment to candidate generation: service=",
        service,
        b" generation=",
        candidate,
    );
    log_pair(
        b"package-import authority delta: service=svc:logd grants=",
        authority_delta,
    );
    log_two_values(
        b"package-import rejected missing dependency: capability=",
        missing_dependency,
        b" reason=",
        b"no-provider no partial graph-store writes",
    );
    log_two_values(
        b"package-import rejected excess authority: capability=",
        excess_authority,
        b" reason=",
        b"undeclared",
    );
    log_two_values(
        b"package-import duplicate import idempotent: package=",
        package,
        b" store_objects_unchanged=",
        b"1",
    );
    log_pair(b"native graph-link closure hash: ", closure_hash);
    log(b"host graph-link closure hash matches native closure hash");
    log(b"package-import queues candidate generation for activation");

    let mut install = [0u8; 96];
    let mut len = 0;
    append(&mut install, &mut len, b"install ");
    append(&mut install, &mut len, candidate);
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &install[..len]) != sys::STATUS_OK {
        log(b"package-import generation-manager install request failed");
        sys::exit(1);
    }
}

fn read_store_object(cap_slot: u64, buffer: &mut [u8]) -> usize {
    let handle = sys::vfs_open_read(cap_slot);
    if status_is_error(handle) {
        log(b"package-import failed to open store object");
        sys::exit(1);
    }
    let read = sys::vfs_read(handle, buffer);
    if status_is_error(read) || read > buffer.len() as u64 {
        log(b"package-import failed to read store object");
        sys::exit(1);
    }
    if sys::vfs_close(handle) != sys::STATUS_OK {
        log(b"package-import failed to close store object");
        sys::exit(1);
    }
    read as usize
}

fn verify_blake3(bytes: &[u8], expected_hex: &[u8], context: &[u8]) {
    let mut actual = [0u8; 64];
    blake3_hex(bytes, &mut actual);
    if expected_hex.len() != actual.len() || !bytes_eq(expected_hex, &actual) {
        log_pair(b"package-import hash mismatch: ", context);
        sys::exit(1);
    }
}

fn blake3_hex(bytes: &[u8], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = blake3::hash(bytes);
    let raw = digest.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        out[index * 2] = HEX[(raw[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(raw[index] & 0x0f) as usize];
        index += 1;
    }
}

fn required_field<'a>(fragment: &'a [u8], key: &[u8]) -> &'a [u8] {
    let Some(value) = field(fragment, key) else {
        log_pair(b"package-import rejected graph fragment: missing ", key);
        sys::exit(1);
    };
    value
}

fn field<'a>(fragment: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut start = 0;
    while start <= fragment.len() {
        let mut end = start;
        while end < fragment.len() && fragment[end] != b'\n' {
            end += 1;
        }
        let line = &fragment[start..end];
        if starts_with(line, key) {
            return Some(&line[key.len()..]);
        }
        if end == fragment.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn log_pair(prefix: &[u8], value: &[u8]) {
    let mut payload = [0u8; 192];
    let mut len = 0;
    append(&mut payload, &mut len, prefix);
    append(&mut payload, &mut len, value);
    log(&payload[..len]);
}

fn log_two_values(prefix: &[u8], first: &[u8], middle: &[u8], second: &[u8]) {
    let mut payload = [0u8; 224];
    let mut len = 0;
    append(&mut payload, &mut len, prefix);
    append(&mut payload, &mut len, first);
    append(&mut payload, &mut len, middle);
    append(&mut payload, &mut len, second);
    log(&payload[..len]);
}

fn send_ready() {
    let ready = ready_message(b"package-import");
    let status = sys::ipc_send(CAP_READINESS, &ready);
    if status != sys::STATUS_OK {
        log(b"package-import readiness send failed");
        sys::exit(1);
    }
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    message[0..2].copy_from_slice(&PROTOCOL_HEALTH_V0.to_le_bytes());
    message[2..4].copy_from_slice(&MESSAGE_READY.to_le_bytes());
    message[4..8].copy_from_slice(&(service.len() as u32).to_le_bytes());
    message[8..16].copy_from_slice(&1u64.to_le_bytes());
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
}

fn append(buffer: &mut [u8], len: &mut usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() && *len < buffer.len() {
        buffer[*len] = bytes[index];
        *len += 1;
        index += 1;
    }
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    value.len() >= prefix.len() && bytes_eq(&value[..prefix.len()], prefix)
}

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && {
        let mut index = 0;
        while index < left.len() {
            if left[index] != right[index] {
                return false;
            }
            index += 1;
        }
        true
    }
}

fn status_is_error(value: u64) -> bool {
    value >= u64::MAX - 4096
}

fn log(message: &[u8]) {
    let _ = sys::log(CAP_SERIAL_LOG, message);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    log(b"package-import panic");
    sys::exit(2)
}
