#![no_std]
#![no_main]

mod limine;
mod memory;
mod paging;
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
    let mut allocator = match init_physical_allocator(&memory_map) {
        Some(allocator) => allocator,
        None => return,
    };

    run_physical_allocator_demo(&mut allocator);
    run_virtual_memory_demo(&mut allocator);
}

fn init_physical_allocator(memory_map: &limine::MemoryMap) -> Option<memory::FrameAllocator> {
    let mut allocator = memory::FrameAllocator::new();

    match allocator.init_from_limine(memory_map) {
        Ok(()) => {}
        Err(memory::InitError::TooManyRanges) => {
            serial::write_str("Physical allocator init failed: too many usable ranges\n");
            return None;
        }
        Err(memory::InitError::NoUsableFrames) => {
            serial::write_str("Physical allocator init failed: no usable frames\n");
            return None;
        }
    }

    Some(allocator)
}

fn run_physical_allocator_demo(allocator: &mut memory::FrameAllocator) {
    print_allocator_stats("Physical allocator initial", &allocator);

    let Some(frame0) = allocator.allocate() else {
        serial::write_str("Physical allocator demo failed: alloc0\n");
        return;
    };
    let Some(frame1) = allocator.allocate() else {
        serial::write_str("Physical allocator demo failed: alloc1\n");
        return;
    };
    let Some(frame2) = allocator.allocate() else {
        serial::write_str("Physical allocator demo failed: alloc2\n");
        return;
    };

    serial::write_str("Physical allocator allocated: ");
    serial::write_u64_hex(frame0.start());
    serial::write_str(" ");
    serial::write_u64_hex(frame1.start());
    serial::write_str(" ");
    serial::write_u64_hex(frame2.start());
    serial::write_str("\n");

    if allocator.free(frame1).is_err() {
        serial::write_str("Physical allocator demo failed: free\n");
        return;
    }

    serial::write_str("Physical allocator freed: ");
    serial::write_u64_hex(frame1.start());
    serial::write_str("\n");

    let Some(reused) = allocator.allocate() else {
        serial::write_str("Physical allocator demo failed: reuse\n");
        return;
    };

    serial::write_str("Physical allocator reused: ");
    serial::write_u64_hex(reused.start());
    serial::write_str("\n");

    print_allocator_stats("Physical allocator final", &allocator);

    if reused.start() == frame1.start() {
        serial::write_str("Physical allocator demo ok\n");
    } else {
        serial::write_str("Physical allocator demo failed: reuse mismatch\n");
    }
}

fn run_virtual_memory_demo(allocator: &mut memory::FrameAllocator) {
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        serial::write_str("Virtual memory demo failed: HHDM unavailable\n");
        return;
    };

    serial::write_str("Limine HHDM offset: ");
    serial::write_u64_hex(hhdm_offset);
    serial::write_str("\n");

    let mut mapper = unsafe { paging::Mapper::active(hhdm_offset) };
    serial::write_str("Active PML4 physical: ");
    serial::write_u64_hex(mapper.root_table_physical());
    serial::write_str("\n");

    let Some(page0) = paging::kernel_heap_page(0) else {
        serial::write_str("Virtual memory demo failed: page0\n");
        return;
    };
    let Some(page1) = paging::kernel_heap_page(1) else {
        serial::write_str("Virtual memory demo failed: page1\n");
        return;
    };
    let Some(frame0) = allocator.allocate() else {
        serial::write_str("Virtual memory demo failed: frame0\n");
        return;
    };
    let Some(frame1) = allocator.allocate() else {
        serial::write_str("Virtual memory demo failed: frame1\n");
        return;
    };

    if let Err(error) = mapper.map_page(page0, frame0, allocator) {
        print_map_error("Virtual memory map failed for page0", error);
        return;
    }
    if let Err(error) = mapper.map_page(page1, frame1, allocator) {
        print_map_error("Virtual memory map failed for page1", error);
        return;
    }

    serial::write_str("Virtual memory mapped heap page: virt=");
    serial::write_u64_hex(page0);
    serial::write_str(" phys=");
    serial::write_u64_hex(frame0.start());
    serial::write_str("\n");
    serial::write_str("Virtual memory mapped heap page: virt=");
    serial::write_u64_hex(page1);
    serial::write_str(" phys=");
    serial::write_u64_hex(frame1.start());
    serial::write_str("\n");

    unsafe {
        let ptr0 = page0 as *mut u64;
        let ptr1 = page1 as *mut u64;
        ptr0.write_volatile(0x4b525553545f4845);
        ptr1.write_volatile(0x41505f4d41505045);

        let value0 = ptr0.read_volatile();
        let value1 = ptr1.read_volatile();

        serial::write_str("Virtual memory heap readback: ");
        serial::write_u64_hex(value0);
        serial::write_str(" ");
        serial::write_u64_hex(value1);
        serial::write_str("\n");

        if value0 == 0x4b525553545f4845 && value1 == 0x41505f4d41505045 {
            serial::write_str("Virtual memory demo ok\n");
        } else {
            serial::write_str("Virtual memory demo failed: readback mismatch\n");
        }
    }

    print_allocator_stats("Physical allocator after virtual memory", allocator);
}

fn print_map_error(label: &str, error: paging::MapError) {
    serial::write_str(label);
    serial::write_str(": ");
    match error {
        paging::MapError::OutOfFrames => serial::write_str("out of frames"),
        paging::MapError::AlreadyMapped => serial::write_str("already mapped"),
        paging::MapError::HugePageEncountered => serial::write_str("huge page encountered"),
    }
    serial::write_str("\n");
}

fn print_allocator_stats(label: &str, allocator: &memory::FrameAllocator) {
    let stats = allocator.stats();

    serial::write_str(label);
    serial::write_str(": ranges=");
    serial::write_u64_dec(stats.range_count as u64);
    serial::write_str(" total_frames=");
    serial::write_u64_dec(stats.total_frames);
    serial::write_str(" allocated_frames=");
    serial::write_u64_dec(stats.allocated_frames);
    serial::write_str(" free_frames=");
    serial::write_u64_dec(stats.free_frames);
    serial::write_str(" recycled_frames=");
    serial::write_u64_dec(stats.recycled_frames as u64);
    serial::write_str("\n");
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
