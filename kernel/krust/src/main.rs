#![no_std]
#![no_main]

mod limine;
mod serial;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial::init();
    serial::write_str("Krust Kernel booted\n");
    print_boot_info();
    halt_loop()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial::write_str("Krust panic\n");
    halt_loop()
}

fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

fn print_boot_info() {
    if limine::base_revision_supported() {
        serial::write_str("Limine base revision supported\n");
    } else {
        serial::write_str("Limine base revision unsupported\n");
        return;
    }

    let Some(memory_map) = limine::memory_map() else {
        serial::write_str("Limine memory map unavailable\n");
        return;
    };

    serial::write_str("Limine memory map entries: ");
    serial::write_u64_dec(memory_map.entry_count());
    serial::write_str("\n");

    let mut index = 0;
    while index < memory_map.entry_count() {
        if let Some(entry) = memory_map.entry(index) {
            serial::write_str("  [");
            serial::write_u64_dec(index);
            serial::write_str("] ");
            serial::write_str(limine::memmap_type_name(entry.entry_type));
            serial::write_str(" base=");
            serial::write_u64_hex(entry.base);
            serial::write_str(" length=");
            serial::write_u64_hex(entry.length);
            serial::write_str("\n");
        }

        index += 1;
    }
}
