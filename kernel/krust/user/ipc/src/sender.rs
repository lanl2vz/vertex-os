#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys::log_write(b"ipc sender started");

    if sys::log_write_raw(sys::BAD_USER_PTR, 4) != sys::STATUS_BAD_BUFFER {
        sys::log_write(b"ipc sender bad log pointer failed");
        sys::exit(1);
    }

    let mut denied_receive = [0u8; 8];
    if sys::ipc_recv(sys::ENDPOINT_CAP_SLOT, &mut denied_receive) != sys::STATUS_BAD_CAPABILITY {
        sys::log_write(b"ipc sender negative receive failed");
        sys::exit(1);
    }

    if sys::ipc_send_raw(sys::ENDPOINT_CAP_SLOT, sys::BAD_USER_PTR, 14) != sys::STATUS_BAD_BUFFER {
        sys::log_write(b"ipc sender bad send pointer failed");
        sys::exit(1);
    }

    let message = b"Krust IPC ping";
    let status = sys::ipc_send(sys::ENDPOINT_CAP_SLOT, message);

    if status == 0 {
        sys::log_write(b"ipc sender sent message");
    } else {
        sys::log_write(b"ipc sender send failed");
    }

    sys::exit(status)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
