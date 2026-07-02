#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;
use vertex_package_import as package_import;

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
static mut IMPORTED_LOGD: bool = false;

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

fn validate_logd_fragment<'a>(buffer: &'a mut [u8]) -> package_import::ImportFragment<'a> {
    let fragment_len = read_store_object(CAP_PACKAGE_FRAGMENT_LOGD, buffer);
    let fragment = &buffer[..fragment_len];
    let mut config_buffer = [0u8; 128];
    let config_len = read_store_object(CAP_CONFIG_PROOF, &mut config_buffer);
    let fragment =
        match package_import::validate_logd_fragment(fragment, &config_buffer[..config_len]) {
            Ok(fragment) => fragment,
            Err(error) => {
                log(error.message);
                sys::exit(1);
            }
        };
    log_two_values(
        b"package-import verified store-object hash: object=",
        fragment.object,
        b" size=",
        fragment.object_size_field,
    );
    fragment
}

fn validate_missing_dependency_fragment() {
    log(b"package-import validates missing-dependency fragment before materialization");
    let mut buffer = [0u8; 256];
    let len = read_store_object(CAP_PACKAGE_FRAGMENT_MISSING_DEPENDENCY, &mut buffer);
    let fragment = &buffer[..len];
    let missing = match package_import::validate_missing_dependency_fragment(fragment) {
        Ok(missing) => missing,
        Err(error) => {
            log(error.message);
            sys::exit(1);
        }
    };
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
    let grant = match package_import::validate_excess_authority_fragment(fragment) {
        Ok(grant) => grant,
        Err(error) => {
            log(error.message);
            sys::exit(1);
        }
    };
    log_two_values(
        b"package-import rejected excess authority: capability=",
        grant,
        b" reason=",
        b"undeclared no candidate install",
    );
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
