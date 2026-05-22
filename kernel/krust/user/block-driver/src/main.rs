#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_BLOCK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_MMIO: u64 = 3;
const CAP_IRQ: u64 = 4;
const CAP_DMA: u64 = 5;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"block-driver ready");

    if sys::mmio_map(CAP_MMIO) == sys::STATUS_BAD_CAPABILITY {
        log(b"block-driver MMIO authority failed");
        sys::exit(1);
    }
    if sys::irq_wait(CAP_IRQ, 0) != sys::STATUS_OK {
        log(b"block-driver IRQ authority failed");
        sys::exit(1);
    }
    if sys::mmio_map(CAP_DMA) == sys::STATUS_BAD_CAPABILITY {
        log(b"block-driver DMA is distinct from MMIO authority");
    }

    let mut request = [0u8; 32];
    let received = sys::ipc_recv(CAP_BLOCK, &mut request);
    if received > request.len() as u64 {
        log(b"block-driver request receive failed");
        sys::exit(1);
    }
    log(b"block-driver received block-read request");

    if sys::ipc_send(CAP_BLOCK, HELLO_OBJECT) != sys::STATUS_OK {
        log(b"block-driver response failed");
        sys::exit(1);
    }
    log(b"block-driver returns bytes");
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
