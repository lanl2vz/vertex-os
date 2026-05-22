#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE_API: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_STATE_BACKEND: u64 = 3;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"vertex-state ready");

    let mut request = [0u8; 16];
    let mut value = [0u8; 16];
    let value_len: usize;
    let mut wrote_value = false;
    let mut pending_read = false;
    loop {
        let received = sys::ipc_recv(CAP_STATE_API, &mut request);
        if received == 1 && request[0] == b'R' {
            if wrote_value {
                let read_len = read_snapshot(&mut value);
                send_read_response(&value[..read_len]);
                value_len = read_len;
                break;
            }
            pending_read = true;
            continue;
        }

        if received >= 2 && received <= request.len() as u64 && request[0] == b'W' && !wrote_value {
            if sys::state_write(CAP_STATE_BACKEND, &request[1..received as usize]) != sys::STATUS_OK
            {
                log(b"vertex-state backend write failed");
                sys::exit(1);
            }
            log(b"counter-service writes state");
            wrote_value = true;
            if pending_read {
                let read_len = read_snapshot(&mut value);
                send_read_response(&value[..read_len]);
                value_len = read_len;
                break;
            }
            continue;
        }

        log(b"vertex-state request invalid");
        sys::exit(1);
    }

    let received = sys::ipc_recv(CAP_STATE_API, &mut request);
    if received < 2 || received > request.len() as u64 || request[0] != b'W' {
        log(b"vertex-state write-denial request invalid");
        sys::exit(1);
    }
    log(b"reader-service write denied");
    if sys::ipc_send(CAP_STATE_API, b"DENIED") != sys::STATUS_OK {
        log(b"vertex-state denial response failed");
        sys::exit(1);
    }

    if sys::state_write(CAP_STATE_BACKEND, &value[..value_len]) != sys::STATUS_OK {
        log(b"vertex-state restore failed");
        sys::exit(1);
    }
    log(b"state restored");
    log(b"system generation rollback does not automatically roll back state unless policy says so");
    log(b"Native state-volume service ok");
    sys::exit(0)
}

fn read_snapshot(value: &mut [u8; 16]) -> usize {
    let value_len = sys::state_read(CAP_STATE_BACKEND, value);
    if value_len == sys::STATUS_EMPTY || value_len > value.len() as u64 {
        log(b"vertex-state backend read failed");
        sys::exit(1);
    }
    log(b"snapshot created");
    value_len as usize
}

fn send_read_response(value: &[u8]) {
    if sys::ipc_send(CAP_STATE_API, value) != sys::STATUS_OK {
        log(b"vertex-state read response failed");
        sys::exit(1);
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
