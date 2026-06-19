#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_PACKAGE_IMPORT_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_GENERATION_MANAGER_REQUEST: u64 = 3;
const CAP_PACKAGE_FRAGMENT_LOGD: u64 = 4;
const CAP_PACKAGE_FRAGMENT_MISSING_DEPENDENCY: u64 = 5;
const CAP_PACKAGE_FRAGMENT_EXCESS_AUTHORITY: u64 = 6;
const CAP_CONFIG_PROOF: u64 = 7;

const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const EXPECTED_AUTHORITY_DELTA: &[u8] = b"cap:console.output/send,cap:vfs.logd-log-stream/resolve+read,cap:net.udp.9000/listen+bind,cap:log.sink/send,config:logd/read";
const EXPECTED_CLOSURE_MATERIAL: &[u8] = b"packages=pkg:logd;services=svc:echo-server,svc:logd;objects=config:logd,store:echo-server-demo,store:logd-demo";

static mut IMPORTED_LOGD: bool = false;

struct ImportFragment<'a> {
    package: &'a [u8],
    service: &'a [u8],
    capability: &'a [u8],
    activated_service: &'a [u8],
    candidate: &'a [u8],
    authority_delta: &'a [u8],
    closure_hash: &'a [u8],
}

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
        if bytes_eq(command, b"import pkg:missing-dependency") {
            validate_missing_dependency_fragment();
            log(
                b"package-import negative missing-dependency import aborted before materialization",
            );
            continue;
        }
        if bytes_eq(command, b"import pkg:excess-authority") {
            validate_excess_authority_fragment();
            log(b"package-import negative excess-authority import aborted before materialization");
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
    let fragment = validate_logd_fragment(&mut fragment_buffer);

    log_pair(
        b"package-import parsed compact typed graph fragment: package=",
        fragment.package,
    );
    log_two_values(
        b"native package import materializes graph delta: add_service=",
        fragment.service,
        b" add_capability=",
        fragment.capability,
    );
    log_pair(
        b"native package import activates closure service: ",
        fragment.activated_service,
    );
    log_pair(
        b"package-import authority delta accepted: ",
        fragment.authority_delta,
    );
    log_pair(b"native graph-link closure hash: ", fragment.closure_hash);
    log(b"package-import verified canonical closure hash");

    let already_imported = unsafe { IMPORTED_LOGD };
    if already_imported {
        log_pair(
            b"package-import duplicate import idempotent: package=",
            fragment.package,
        );
        log(b"package-import duplicate import queues no candidate install");
        return;
    }
    unsafe {
        IMPORTED_LOGD = true;
    }

    log_pair(
        b"package-import registers native graph generation before activation: ",
        fragment.candidate,
    );
    let mut register = [0u8; 96];
    let mut register_len = 0;
    append(&mut register, &mut register_len, b"register-import ");
    append(&mut register, &mut register_len, fragment.candidate);
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &register[..register_len]) != sys::STATUS_OK {
        log(b"package-import generation-manager register request failed");
        sys::exit(1);
    }

    log(b"package-import queues candidate generation for activation");
    let mut install = [0u8; 96];
    let mut len = 0;
    append(&mut install, &mut len, b"install ");
    append(&mut install, &mut len, fragment.candidate);
    if sys::ipc_send(CAP_GENERATION_MANAGER_REQUEST, &install[..len]) != sys::STATUS_OK {
        log(b"package-import generation-manager install request failed");
        sys::exit(1);
    }
}

fn validate_logd_fragment<'a>(buffer: &'a mut [u8]) -> ImportFragment<'a> {
    let fragment_len = read_store_object(CAP_PACKAGE_FRAGMENT_LOGD, buffer);
    let fragment = &buffer[..fragment_len];
    validate_fragment_magic(fragment);
    assert_field_eq(fragment, b"kind=", b"import");

    let package = required_field(fragment, b"package=");
    assert_bytes(package, b"pkg:logd", b"package");
    let candidate = required_field(fragment, b"candidate=");
    assert_bytes(candidate, b"gen:package-import-new-0002", b"candidate");
    let service = required_field(fragment, b"add_service=");
    assert_bytes(service, b"svc:logd", b"add_service");
    let capability = required_field(fragment, b"add_capability=");
    assert_bytes(capability, b"cap:log.sink", b"add_capability");
    let activated_service = required_field(fragment, b"activate_service=");
    assert_bytes(activated_service, b"svc:echo-server", b"activate_service");
    assert_field_eq(
        fragment,
        b"requires_base=",
        b"cap:console.output,cap:vfs.logd-log-stream,cap:net.udp.9000",
    );
    assert_field_eq(fragment, b"requires_import=", b"cap:log.sink");

    let authority_delta = required_field(fragment, b"authority_delta=");
    assert_bytes(
        authority_delta,
        EXPECTED_AUTHORITY_DELTA,
        b"authority_delta",
    );
    let object = required_field(fragment, b"object=");
    assert_bytes(object, b"config:logd", b"object");
    let object_size = required_decimal(fragment, b"object_size=");
    let object_hash = required_field(fragment, b"object_hash=");
    let closure_material = required_field(fragment, b"closure_material=");
    assert_bytes(
        closure_material,
        EXPECTED_CLOSURE_MATERIAL,
        b"closure_material",
    );
    let closure_hash = required_field(fragment, b"closure_hash=");

    let mut config_buffer = [0u8; 128];
    let config_len = read_store_object(CAP_CONFIG_PROOF, &mut config_buffer);
    if config_len != object_size {
        log(b"package-import rejected store object: size mismatch");
        sys::exit(1);
    }
    verify_blake3(
        &config_buffer[..config_len],
        object_hash,
        b"store-object hash",
    );
    log_two_values(
        b"package-import verified store-object hash: object=",
        object,
        b" size=",
        required_field(fragment, b"object_size="),
    );

    verify_blake3(closure_material, closure_hash, b"closure hash");

    ImportFragment {
        package,
        service,
        capability,
        activated_service,
        candidate,
        authority_delta,
        closure_hash,
    }
}

fn validate_missing_dependency_fragment() {
    log(b"package-import validates missing-dependency fragment before materialization");
    let mut buffer = [0u8; 256];
    let len = read_store_object(CAP_PACKAGE_FRAGMENT_MISSING_DEPENDENCY, &mut buffer);
    let fragment = &buffer[..len];
    validate_fragment_magic(fragment);
    assert_field_eq(fragment, b"kind=", b"negative-missing-dependency");
    let missing = required_field(fragment, b"require=");
    if provider_known(missing) {
        log_pair(
            b"package-import negative dependency unexpectedly resolved: ",
            missing,
        );
        sys::exit(1);
    }
    log_two_values(
        b"package-import rejected missing dependency: capability=",
        missing,
        b" reason=",
        b"no-provider no candidate install",
    );
}

fn validate_excess_authority_fragment() {
    log(b"package-import validates excess-authority fragment before materialization");
    let mut buffer = [0u8; 256];
    let len = read_store_object(CAP_PACKAGE_FRAGMENT_EXCESS_AUTHORITY, &mut buffer);
    let fragment = &buffer[..len];
    validate_fragment_magic(fragment);
    assert_field_eq(fragment, b"kind=", b"negative-excess-authority");
    let grant = required_field(fragment, b"grant=");
    if authority_allowed(grant) {
        log_pair(
            b"package-import negative authority unexpectedly allowed: ",
            grant,
        );
        sys::exit(1);
    }
    log_two_values(
        b"package-import rejected excess authority: capability=",
        grant,
        b" reason=",
        b"undeclared no candidate install",
    );
}

fn validate_fragment_magic(fragment: &[u8]) {
    if !starts_with(fragment, b"PKGFRAGV1\n") {
        log(b"package-import rejected graph fragment: bad magic");
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
    if read == buffer.len() as u64 {
        let mut extra = [0u8; 1];
        let extra_read = sys::vfs_read(handle, &mut extra);
        if status_is_error(extra_read) {
            log(b"package-import failed to read store object");
            sys::exit(1);
        }
        if extra_read != 0 {
            log(b"package-import rejected store object: oversized");
            sys::exit(1);
        }
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

fn required_decimal(fragment: &[u8], key: &[u8]) -> usize {
    let value = required_field(fragment, key);
    let Some(parsed) = parse_decimal(value) else {
        log_pair(b"package-import rejected graph fragment: bad decimal ", key);
        sys::exit(1);
    };
    parsed
}

fn parse_decimal(value: &[u8]) -> Option<usize> {
    if value.is_empty() {
        return None;
    }
    let mut out = 0usize;
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.checked_mul(10)?;
        out = out.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(out)
}

fn assert_field_eq(fragment: &[u8], key: &[u8], expected: &[u8]) {
    let value = required_field(fragment, key);
    assert_bytes(value, expected, key);
}

fn assert_bytes(value: &[u8], expected: &[u8], context: &[u8]) {
    if !bytes_eq(value, expected) {
        log_pair(
            b"package-import rejected graph fragment: unexpected ",
            context,
        );
        sys::exit(1);
    }
}

fn provider_known(capability: &[u8]) -> bool {
    bytes_eq(capability, b"cap:console.output")
        || bytes_eq(capability, b"cap:vfs.logd-log-stream")
        || bytes_eq(capability, b"cap:net.udp.9000")
        || bytes_eq(capability, b"cap:log.sink")
}

fn authority_allowed(grant: &[u8]) -> bool {
    bytes_eq(grant, b"cap:console.output/send")
        || bytes_eq(grant, b"cap:vfs.logd-log-stream/resolve+read")
        || bytes_eq(grant, b"cap:net.udp.9000/listen+bind")
        || bytes_eq(grant, b"cap:log.sink/send")
        || bytes_eq(grant, b"config:logd/read")
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
    let mut payload = [0u8; 256];
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
