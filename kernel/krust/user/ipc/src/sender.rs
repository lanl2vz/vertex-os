#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys::write_serial(b"ipc sender started");

    let message = b"Krust IPC ping";
    let status = sys::ipc_send(sys::ENDPOINT_CAP_SLOT, message);

    if status == 0 {
        sys::write_serial(b"ipc sender sent message");
    } else {
        sys::write_serial(b"ipc sender send failed");
    }

    sys::exit(status)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
