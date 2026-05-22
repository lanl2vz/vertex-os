use core::{
    arch::asm,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::serial;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xa0;
const PIC2_DATA: u16 = 0xa1;
const PIC_EOI: u8 = 0x20;
const PIC_MASTER_IRQ0_ONLY: u8 = 0xfe;
const PIC_SLAVE_ALL_MASKED: u8 = 0xff;

const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_BASE_HZ: u32 = 1_193_182;
const TIMER_HZ: u32 = 100;
const PIT_DIVISOR: u16 = (PIT_BASE_HZ / TIMER_HZ) as u16;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    remap_pic();
    program_pit();
    serial::write_str("PIT timer interrupt initialized: vector=32 hz=100\n");
    serial::write_str("Preemption disabled in kernel critical sections\n");
}

pub fn handle_tick() -> u64 {
    let tick = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    if tick == 1 {
        serial::write_str("Timer tick increments: ticks=1\n");
    }
    tick
}

pub fn eoi() {
    unsafe {
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

pub fn wait_for_interrupt() {
    unsafe {
        asm!("sti; hlt; cli", options(nomem, nostack, preserves_flags));
    }
}

fn remap_pic() {
    unsafe {
        outb(PIC1_COMMAND, 0x11);
        io_wait();
        outb(PIC2_COMMAND, 0x11);
        io_wait();
        outb(PIC1_DATA, 0x20);
        io_wait();
        outb(PIC2_DATA, 0x28);
        io_wait();
        outb(PIC1_DATA, 0x04);
        io_wait();
        outb(PIC2_DATA, 0x02);
        io_wait();
        outb(PIC1_DATA, 0x01);
        io_wait();
        outb(PIC2_DATA, 0x01);
        io_wait();

        outb(PIC1_DATA, PIC_MASTER_IRQ0_ONLY);
        outb(PIC2_DATA, PIC_SLAVE_ALL_MASKED);
    }
}

fn program_pit() {
    unsafe {
        outb(PIT_COMMAND, 0x34);
        outb(PIT_CHANNEL0, (PIT_DIVISOR & 0xff) as u8);
        outb(PIT_CHANNEL0, (PIT_DIVISOR >> 8) as u8);
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

unsafe fn io_wait() {
    unsafe {
        asm!("out 0x80, al", in("al") 0u8, options(nomem, nostack, preserves_flags));
    }
}
