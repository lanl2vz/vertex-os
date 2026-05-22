#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys::write_serial(b"ipc receiver started");

    let mut buffer = [0u8; 64];
    let received = sys::ipc_recv(sys::ENDPOINT_CAP_SLOT, &mut buffer);

    if received <= buffer.len() as u64 {
        sys::write_serial(b"ipc receiver received message");
        sys::write_serial(&buffer[..received as usize]);

        if sys::ipc_recv_raw(sys::ENDPOINT_CAP_SLOT, sys::BAD_USER_PTR, 8) != sys::STATUS_BAD_BUFFER
        {
            sys::write_serial(b"ipc receiver bad recv pointer failed");
            sys::exit(1);
        }

        if sys::ipc_send(sys::ENDPOINT_CAP_SLOT, b"not allowed") != sys::STATUS_BAD_CAPABILITY {
            sys::write_serial(b"ipc receiver negative send failed");
            sys::exit(1);
        }

        sys::exit(0);
    }

    sys::write_serial(b"ipc receiver receive failed");
    sys::exit(1)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
