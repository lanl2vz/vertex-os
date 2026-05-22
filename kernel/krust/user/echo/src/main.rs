#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_COPY: u64 = 28;
const CAP_MOVED: u64 = 27;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::ipc_send(CAP_LOG_SINK, b"hello from echo") != sys::STATUS_OK {
        log(b"echo send failed");
        sys::exit(1);
    }
    if sys::process_attempt() > 1 {
        log(b"echo restart retained delegated log cap");
    } else {
        log(b"echo sent message to logd");
    }

    if sys::endpoint_create(CAP_LOG_SINK, 26) == sys::STATUS_BAD_CAPABILITY {
        log(b"service with no allocation authority cannot create endpoint");
    } else {
        log(b"echo endpoint create denial failed");
        sys::exit(1);
    }

    if sys::io_write(CAP_LOG_SINK, 0x3f8, b'!') == sys::STATUS_BAD_CAPABILITY {
        log(b"echo I/O write rejected");
        log(b"echo cannot write COM1 directly");
    } else {
        log(b"echo I/O denial failed");
        sys::exit(1);
    }
    if sys::ipc_send(3, b"block read") == sys::STATUS_BAD_CAPABILITY {
        log(b"unauthorized service cannot talk to block-driver");
    } else {
        log(b"echo block-driver denial failed");
        sys::exit(1);
    }
    if sys::mmio_map(3) == sys::STATUS_BAD_CAPABILITY
        && sys::irq_wait(3, 0) == sys::STATUS_BAD_CAPABILITY
    {
        log(b"unauthorized service cannot access MMIO, IRQ, or DMA capabilities");
    } else {
        log(b"echo device authority denial failed");
        sys::exit(1);
    }

    if sys::cap_inspect(CAP_LOG_SINK) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo cap inspect failed");
        sys::exit(1);
    }
    log(b"cap inspect shows parent chain");

    if sys::cap_copy(CAP_LOG_SINK, CAP_COPY, sys::RIGHT_SEND) != sys::STATUS_OK {
        log(b"echo cap copy failed");
        sys::exit(1);
    }
    if sys::cap_inspect(CAP_LOG_SINK) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo cap copy did not preserve source");
        sys::exit(1);
    }
    log(b"cap copy preserves source slot");

    if sys::cap_move(CAP_COPY, CAP_MOVED) != sys::STATUS_OK {
        log(b"echo cap move failed");
        sys::exit(1);
    }
    if sys::cap_inspect(CAP_COPY) == sys::STATUS_BAD_CAPABILITY {
        log(b"cap move removes source slot");
    } else {
        log(b"echo cap move source still valid");
        sys::exit(1);
    }
    if sys::cap_revoke(CAP_MOVED) != sys::STATUS_OK {
        log(b"echo cap revoke failed");
        sys::exit(1);
    }
    if sys::ipc_send(CAP_MOVED, b"after revoke") == sys::STATUS_BAD_CAPABILITY {
        log(b"echo send after revoke rejected");
    } else {
        log(b"echo send after revoke failed");
        sys::exit(1);
    }

    let mut denied = [0u8; 8];
    if sys::ipc_recv(CAP_LOG_SINK, &mut denied) == sys::STATUS_BAD_CAPABILITY {
        log(b"negative test: echo receive rejected: bad capability");
    } else {
        log(b"echo negative receive failed");
        sys::exit(1);
    }

    let mut object_buffer = [0u8; 16];
    if sys::object_read(CAP_LOG_SINK, &mut object_buffer) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo read rejected: bad capability");
        log(b"unauthorized process cannot read object");
    } else {
        log(b"echo negative object-read failed");
        sys::exit(1);
    }

    if sys::cap_drop(CAP_LOG_SINK) != sys::STATUS_OK {
        log(b"echo drop cap failed");
        sys::exit(1);
    }
    log(b"echo drops cap");

    if sys::ipc_send(CAP_LOG_SINK, b"after drop") == sys::STATUS_BAD_CAPABILITY {
        log(b"echo send after drop rejected");
    } else {
        log(b"echo send after drop failed");
        sys::exit(1);
    }

    sys::exit(0)
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
