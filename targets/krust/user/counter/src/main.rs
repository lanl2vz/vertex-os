#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_COUNTER_REQUEST: u64 = 0;
const CAP_STATE_VFS_ONESHOT: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_CONSOLE_REPLY: u64 = 3;
const CAP_STATE_VFS_CONSOLE: u64 = 4;
const STATE_VALUE_PATH: &[u8] = b"/state/counter/value";
const UNDECLARED_STATE_VALUE_PATH: &[u8] = b"/state/scratch/value";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut probe = [0u8; 8];
    let received = sys::ipc_recv(CAP_COUNTER_REQUEST, &mut probe);
    if received != sys::STATUS_BAD_CAPABILITY {
        run_counter_endpoint(received, &probe);
    }

    log(b"counter-service has VFS state file");
    persist_counter(CAP_STATE_VFS_ONESHOT, 41);
    log(b"counter-service writes state through VFS");
    sys::exit(0)
}

fn run_counter_endpoint(first_received: u64, first_buffer: &[u8; 8]) -> ! {
    log(b"counter-service has request endpoint");
    let mut value = 41u64;
    prove_undeclared_state_alias_rejected();
    if first_received != sys::STATUS_EMPTY {
        if first_received == 1 && first_buffer[0] == b'H' {
            handle_counter_request(first_received, first_buffer, &mut value);
        }
        persist_counter(CAP_STATE_VFS_CONSOLE, value);
        handle_counter_request(first_received, first_buffer, &mut value);
    } else {
        persist_counter(CAP_STATE_VFS_CONSOLE, value);
    }
    loop {
        let mut request = [0u8; 8];
        let received = sys::ipc_recv(CAP_COUNTER_REQUEST, &mut request);
        if received == sys::STATUS_EMPTY {
            sys::yield_now();
            continue;
        }
        handle_counter_request(received, &request, &mut value);
    }
}

fn prove_undeclared_state_alias_rejected() {
    let handle = sys::vfs_open_path_readwrite(CAP_STATE_VFS_CONSOLE, UNDECLARED_STATE_VALUE_PATH);
    if handle == sys::STATUS_VFS_PERMISSION || handle == sys::STATUS_BAD_CAPABILITY {
        log(b"statefs graph authority rejects undeclared state path alias");
        return;
    }
    let _ = sys::vfs_close(handle);
    log(b"counter-service undeclared state alias test failed");
    sys::exit(1);
}

fn handle_counter_request(received: u64, request: &[u8; 8], value: &mut u64) {
    if received == 1 && request[0] == b'G' {
        reply_counter(*value);
        return;
    }
    if received == 1 && request[0] == b'I' {
        *value = value.saturating_add(1);
        persist_counter(CAP_STATE_VFS_CONSOLE, *value);
        reply_counter(*value);
        return;
    }
    if received == 1 && request[0] == b'H' {
        log(b"counter-service shutdown requested");
        sys::exit(0);
    }
    log(b"counter-service request invalid");
    sys::exit(1);
}

fn persist_counter(cap_slot: u64, value: u64) {
    let mut payload = [0u8; 3];
    let len = write_decimal(&mut payload, 0, value);
    let handle = sys::vfs_open_path_readwrite(cap_slot, STATE_VALUE_PATH);
    if handle == sys::STATUS_BAD_CAPABILITY {
        log(b"counter-service state open failed");
        sys::exit(1);
    }
    if sys::vfs_write(handle, &payload[..len]) != len as u64 {
        log(b"counter-service state write failed");
        sys::exit(1);
    }
    let _ = sys::vfs_close(handle);
    log(b"counter-service persists state value");
}

fn reply_counter(value: u64) {
    let mut payload = [0u8; 8];
    let len = write_decimal(&mut payload, 0, value);
    if sys::ipc_send(CAP_CONSOLE_REPLY, &payload[..len]) != sys::STATUS_OK {
        log(b"counter-service reply failed");
        sys::exit(1);
    }
}

fn write_decimal(buffer: &mut [u8], offset: usize, value: u64) -> usize {
    if value >= 100 {
        buffer[offset] = b'0' + (value / 100) as u8;
        buffer[offset + 1] = b'0' + ((value / 10) % 10) as u8;
        buffer[offset + 2] = b'0' + (value % 10) as u8;
        offset + 3
    } else if value >= 10 {
        buffer[offset] = b'0' + (value / 10) as u8;
        buffer[offset + 1] = b'0' + (value % 10) as u8;
        offset + 2
    } else {
        buffer[offset] = b'0' + value as u8;
        offset + 1
    }
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
