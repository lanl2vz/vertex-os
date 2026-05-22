#![no_std]
#![no_main]

mod limine;
mod serial;

use core::arch::asm;
use core::panic::PanicInfo;

const VERTEX_MANIFEST_MODULE: &[u8] = b"vertex-manifest";
const GENERATION_ID_PREFIX: &[u8] = b"gen:";

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

    print_manifest_module();
}

fn print_manifest_module() {
    let Some(modules) = limine::modules() else {
        serial::write_str("Limine modules unavailable\n");
        return;
    };

    serial::write_str("Limine modules: ");
    serial::write_u64_dec(modules.module_count());
    serial::write_str("\n");

    let mut index = 0;
    while index < modules.module_count() {
        if let Some(module) = modules.module(index) {
            serial::write_str("  module[");
            serial::write_u64_dec(index);
            serial::write_str("] path=");
            serial::write_c_string(module.path);
            serial::write_str(" string=");
            serial::write_c_string(module.string);
            serial::write_str(" size=");
            serial::write_u64_dec(module.size);
            serial::write_str("\n");

            if c_string_eq(module.string, VERTEX_MANIFEST_MODULE) {
                print_vertex_manifest(module);
            }
        }

        index += 1;
    }
}

fn print_vertex_manifest(module: &limine::File) {
    serial::write_str("Vertex manifest module: ");
    serial::write_c_string(module.path);
    serial::write_str(" bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");

    let bytes = unsafe { core::slice::from_raw_parts(module.address, module.size as usize) };
    serial::write_str("Vertex manifest generation: ");
    if let Some(id) = find_generation_id(bytes) {
        serial::write_ascii_bytes(id);
    } else {
        serial::write_str("<not found>");
    }
    serial::write_str("\n");
}

fn c_string_eq(value: *const u8, expected: &[u8]) -> bool {
    if value.is_null() {
        return false;
    }

    let mut index = 0;
    while index < expected.len() {
        let byte = unsafe { value.add(index).read() };
        if byte != expected[index] {
            return false;
        }

        index += 1;
    }

    unsafe { value.add(expected.len()).read() == 0 }
}

fn find_generation_id(bytes: &[u8]) -> Option<&[u8]> {
    let start = find_bytes(bytes, GENERATION_ID_PREFIX)?;
    let mut end = start;

    while end < bytes.len() {
        let byte = bytes[end];
        if !(byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-')) {
            break;
        }
        end += 1;
    }

    Some(&bytes[start..end])
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    let mut index = 0;
    while index <= haystack.len() - needle.len() {
        if &haystack[index..index + needle.len()] == needle {
            return Some(index);
        }
        index += 1;
    }

    None
}
