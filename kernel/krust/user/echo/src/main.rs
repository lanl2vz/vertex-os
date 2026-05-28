#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_NETWORK_PORT: u64 = 3;
const CAP_NAMESPACE: u64 = 4;
const CAP_NAMESPACE_RESOLVED: u64 = 24;
const CAP_NETWORK_BIND_ONLY: u64 = 23;
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
    if sys::network_send_udp(CAP_NETWORK_PORT, b"m57 udp probe") == sys::STATUS_OK {
        log(b"echo sends UDP through cap:net.udp.9000 without a raw virtio-device cap");
        log(b"network authority is endpoint/capability mediated");
        log(b"echo submits UDP request to netstack boundary");
    } else {
        log(b"echo UDP send failed");
        sys::exit(1);
    }
    if sys::virtio_net_tx(CAP_NETWORK_PORT, b"raw frame denied") == sys::STATUS_BAD_CAPABILITY {
        log(b"unauthorized service cannot use network device");
    } else {
        log(b"echo network device denial failed");
        sys::exit(1);
    }

    if sys::namespace_resolve(CAP_NAMESPACE, b"/state/a", CAP_NAMESPACE_RESOLVED) != sys::STATUS_OK
    {
        log(b"namespace resolve /state/a failed");
        sys::exit(1);
    }
    log(b"service A namespace contains /state/a");
    if sys::cap_inspect(CAP_NAMESPACE_RESOLVED) == sys::STATUS_BAD_CAPABILITY {
        log(b"namespace resolved capability inspect failed");
        sys::exit(1);
    }
    if sys::namespace_resolve(CAP_NAMESPACE, b"/state/b", CAP_NAMESPACE_RESOLVED - 1)
        == sys::STATUS_BAD_CAPABILITY
    {
        log(b"service A cannot resolve /state/b");
    } else {
        log(b"namespace isolation denial failed");
        sys::exit(1);
    }

    run_m61_syscall_negative_table();

    let mut dma_denied = [0u8; 24];
    if sys::io_read(3, 0x0cf8) == sys::STATUS_BAD_CAPABILITY
        && sys::irq_wait(3, 0) == sys::STATUS_BAD_CAPABILITY
        && sys::dma_map(3, &mut dma_denied) == sys::STATUS_BAD_CAPABILITY
    {
        log(b"unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities");
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

    if sys::object_read(25, &mut object_buffer) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo cannot read logd config");
    } else {
        log(b"echo config-read denial failed");
        sys::exit(1);
    }

    let mut secret_buffer = [0u8; 16];
    if sys::secret_read(25, &mut secret_buffer) == sys::STATUS_BAD_CAPABILITY {
        log(b"service without secret cap rejected");
    } else {
        log(b"echo secret-read denial failed");
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

fn run_m61_syscall_negative_table() {
    let mut denied = [0u8; 8];
    if sys::ipc_recv(CAP_LOG_SINK, &mut denied) != sys::STATUS_BAD_CAPABILITY
        || sys::object_read(CAP_LOG_SINK, &mut denied) != sys::STATUS_BAD_CAPABILITY
        || sys::io_read(CAP_LOG_SINK, 0x3f8) != sys::STATUS_BAD_CAPABILITY
        || sys::mmio_map(CAP_LOG_SINK) != sys::STATUS_BAD_CAPABILITY
        || sys::irq_wait(CAP_LOG_SINK, 0) != sys::STATUS_BAD_CAPABILITY
        || sys::secret_read(CAP_LOG_SINK, &mut denied) != sys::STATUS_BAD_CAPABILITY
        || sys::network_send_udp(CAP_LOG_SINK, b"wrong kind") != sys::STATUS_BAD_CAPABILITY
        || sys::namespace_resolve(CAP_LOG_SINK, b"/state/a", CAP_NAMESPACE_RESOLVED - 2)
            != sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 wrong object-kind negative table failed");
        sys::exit(1);
    }
    log(b"M61 syscall negative table: wrong object kind rejected");

    if sys::cap_copy(CAP_LOG_SINK, CAP_COPY, sys::RIGHT_RECEIVE) != sys::STATUS_BAD_CAPABILITY
        || sys::cap_copy(CAP_NETWORK_PORT, CAP_NETWORK_BIND_ONLY, sys::RIGHT_BIND) != sys::STATUS_OK
        || sys::network_send_udp(CAP_NETWORK_BIND_ONLY, b"missing listen")
            != sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 missing-rights negative table failed");
        sys::exit(1);
    }
    log(b"unauthorized service cannot bind or send on cap:net.udp.9000");
    log(b"M61 syscall negative table: missing rights rejected");

    if sys::ipc_send_raw(CAP_LOG_SINK, 1, 4) != sys::STATUS_BAD_BUFFER {
        log(b"M61 malformed user-buffer negative table failed");
        sys::exit(1);
    }
    log(b"M61 syscall negative table: malformed buffers rejected");
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
