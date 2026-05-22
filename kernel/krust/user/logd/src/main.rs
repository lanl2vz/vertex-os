#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const ECHO_PROCESS_INDEX: u64 = 2;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 64];
    let received = sys::ipc_recv(CAP_LOG_SINK, &mut buffer);
    if received > buffer.len() as u64 {
        log(b"logd receive failed");
        sys::exit(1);
    }

    log_prefix(b"logd received: ", &buffer[..received as usize]);

    if sys::process_start(CAP_LOG_SINK, ECHO_PROCESS_INDEX) == sys::STATUS_BAD_CAPABILITY {
        log(b"negative test: logd process-start rejected: bad capability");
    } else {
        log(b"logd negative process-start failed");
        sys::exit(1);
    }

    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
