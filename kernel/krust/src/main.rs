#![no_std]
#![no_main]

mod boot_manifest;
mod capability;
mod elf;
mod exceptions;
mod gdt;
mod ipc;
mod limine;
mod memory;
mod paging;
mod serial;
mod syscall;
mod usercopy;
mod userspace;

use core::arch::asm;
use core::panic::PanicInfo;

const MAX_BOOT_PROCESSES: usize = 16;

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

    let Some(boot_manifest) = print_boot_modules() else {
        return;
    };
    let mut allocator = match init_physical_allocator(&memory_map) {
        Some(allocator) => allocator,
        None => return,
    };

    run_physical_allocator_demo(&mut allocator);
    let Some(heap) = run_virtual_memory_demo(&mut allocator) else {
        return;
    };
    run_capability_table_demo(&allocator, heap);
    run_native_boot(&mut allocator, &boot_manifest);
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

#[derive(Clone, Copy)]
struct KernelHeapMapping {
    base: u64,
    length: u64,
}

fn run_virtual_memory_demo(allocator: &mut memory::FrameAllocator) -> Option<KernelHeapMapping> {
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        serial::write_str("Virtual memory demo failed: HHDM unavailable\n");
        return None;
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
        return None;
    };
    let Some(page1) = paging::kernel_heap_page(1) else {
        serial::write_str("Virtual memory demo failed: page1\n");
        return None;
    };
    let Some(frame0) = allocator.allocate() else {
        serial::write_str("Virtual memory demo failed: frame0\n");
        return None;
    };
    let Some(frame1) = allocator.allocate() else {
        serial::write_str("Virtual memory demo failed: frame1\n");
        return None;
    };

    if let Err(error) = mapper.map_page(page0, frame0, allocator) {
        print_map_error("Virtual memory map failed for page0", error);
        return None;
    }
    if let Err(error) = mapper.map_page(page1, frame1, allocator) {
        print_map_error("Virtual memory map failed for page1", error);
        return None;
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
    Some(KernelHeapMapping {
        base: paging::KERNEL_HEAP_BASE,
        length: paging::KERNEL_HEAP_PAGES as u64 * memory::FRAME_SIZE,
    })
}

fn run_capability_table_demo(allocator: &memory::FrameAllocator, heap: KernelHeapMapping) {
    let mut table = capability::CapabilityTable::new();
    let stats = allocator.stats();
    let manifest_module = find_boot_manifest_module();

    let kernel_process =
        match table.add_object(capability::KernelObjectKind::Process, "proc:kernel", 0, 0) {
            Ok(id) => id,
            Err(_) => {
                serial::write_str("Capability table demo failed: kernel process object\n");
                return;
            }
        };
    let bootstrap_thread = match table.add_object(
        capability::KernelObjectKind::Thread,
        "thread:bootstrap",
        0,
        0,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: bootstrap thread object\n");
            return;
        }
    };
    let serial_endpoint = match table.add_object(
        capability::KernelObjectKind::IpcEndpoint,
        "endpoint:serial-com1",
        0x3f8,
        8,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: serial endpoint object\n");
            return;
        }
    };
    let physical_memory = match table.add_object(
        capability::KernelObjectKind::MemoryObject,
        "mem:usable-frames",
        0,
        stats.total_frames * memory::FRAME_SIZE,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: physical memory object\n");
            return;
        }
    };
    let kernel_heap = match table.add_object(
        capability::KernelObjectKind::MemoryObject,
        "mem:kernel-heap",
        heap.base,
        heap.length,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: kernel heap object\n");
            return;
        }
    };
    let boot_module = match table.add_object(
        capability::KernelObjectKind::BootModule,
        "module:krustboot-manifest",
        manifest_module
            .map(|module| module.address as u64)
            .unwrap_or(0),
        manifest_module.map(|module| module.size).unwrap_or(0),
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: manifest module object\n");
            return;
        }
    };

    if table
        .grant(
            kernel_process,
            capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_CONTROL,
        )
        .is_err()
        || table
            .grant(
                bootstrap_thread,
                capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_CONTROL,
            )
            .is_err()
        || table
            .grant(
                serial_endpoint,
                capability::RIGHT_SEND | capability::RIGHT_CONTROL,
            )
            .is_err()
        || table
            .grant(
                physical_memory,
                capability::RIGHT_READ | capability::RIGHT_ALLOCATE,
            )
            .is_err()
        || table
            .grant(
                kernel_heap,
                capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_MAP,
            )
            .is_err()
        || table.grant(boot_module, capability::RIGHT_READ).is_err()
    {
        serial::write_str("Capability table demo failed: grant\n");
        return;
    }

    table.print();

    if table.object_count() == 6 && table.capability_count() == 6 {
        serial::write_str("Capability table demo ok\n");
    } else {
        serial::write_str("Capability table demo failed: count mismatch\n");
    }
}

fn run_native_boot(
    allocator: &mut memory::FrameAllocator,
    boot_manifest: &boot_manifest::Manifest<'static>,
) {
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        serial::write_str("Native userspace load failed: HHDM unavailable\n");
        return;
    };

    let mut images = [None; MAX_BOOT_PROCESSES];
    let mut index = 0;
    while index < boot_manifest.process_count() {
        let Some(process) = boot_manifest.process(index) else {
            serial::write_str("KrustBoot IPC plan failed: process gap\n");
            return;
        };
        let Some(image) = load_boot_process_image(process, hhdm_offset, allocator) else {
            return;
        };
        images[index] = Some(image);
        index += 1;
    }

    let Some(config) = build_boot_runtime_config(boot_manifest, &images) else {
        return;
    };
    let Some(manifest_module) = find_boot_manifest_module() else {
        serial::write_str("KrustBoot runtime init failed: manifest module missing\n");
        return;
    };
    let mut config = config;
    config.set_manifest_module(ipc::BootModuleConfig {
        name: "krustboot-manifest",
        base: manifest_module.address as u64,
        length: manifest_module.size,
    });

    if ipc::init_from_boot_config(config).is_err() {
        serial::write_str("Native runtime init failed from KrustBoot manifest\n");
        return;
    }

    let Some(initial) = ipc::initial_process_context() else {
        serial::write_str("Native runtime init failed: no initial process\n");
        return;
    };

    userspace::enter_initial_process(ipc::initial_process_name(), initial);
}

fn load_boot_process_image(
    process: boot_manifest::Process<'static>,
    hhdm_offset: u64,
    allocator: &mut memory::FrameAllocator,
) -> Option<userspace::UserImage> {
    let Some(module) = find_module_by_string(process.module_string.as_bytes()) else {
        serial::write_str("KrustBoot process module unavailable: process=");
        serial::write_str(process.name);
        serial::write_str(" module=");
        serial::write_str(process.module_string);
        serial::write_str("\n");
        return None;
    };

    serial::write_str("KrustBoot process module: process=");
    serial::write_str(process.name);
    serial::write_str(" path=");
    serial::write_c_string(module.path);
    serial::write_str(" string=");
    serial::write_str(process.module_string);
    serial::write_str(" bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");

    let bytes = unsafe { core::slice::from_raw_parts(module.address, module.size as usize) };
    match userspace::load(bytes, hhdm_offset, allocator) {
        Ok(image) => {
            serial::write_str("KrustBoot process ELF loaded: process=");
            serial::write_str(process.name);
            serial::write_str(" entry=");
            serial::write_u64_hex(image.entry);
            serial::write_str(" stack=");
            serial::write_u64_hex(image.stack_top);
            serial::write_str(" cr3=");
            serial::write_u64_hex(image.cr3);
            serial::write_str("\n");
            Some(image)
        }
        Err(error) => {
            userspace::print_load_error(error);
            None
        }
    }
}

fn build_boot_runtime_config(
    boot_manifest: &boot_manifest::Manifest<'static>,
    images: &[Option<userspace::UserImage>; MAX_BOOT_PROCESSES],
) -> Option<ipc::BootRuntimeConfig> {
    let mut config = ipc::BootRuntimeConfig::new();

    let mut index = 0;
    while index < boot_manifest.endpoint_count() {
        let endpoint = boot_manifest.endpoint(index)?;
        if config
            .add_endpoint(ipc::BootEndpointConfig {
                name: endpoint.name,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: endpoint table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.process_count() {
        let process = boot_manifest.process(index)?;
        let Some(image) = images[index] else {
            serial::write_str("KrustBoot runtime plan failed: process image gap\n");
            return None;
        };
        if config
            .add_process(ipc::BootProcessConfig {
                name: process.name,
                context: ipc::ProcessContext {
                    cr3: image.cr3,
                    entry: image.entry,
                    stack_top: image.stack_top,
                },
                initial: process.initial,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: process table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.store_object_count() {
        let object = boot_manifest.store_object(index)?;
        let Some(module) = find_module_by_string(object.module_string.as_bytes()) else {
            serial::write_str("KrustBoot store module unavailable: object=");
            serial::write_str(object.id);
            serial::write_str(" module=");
            serial::write_str(object.module_string);
            serial::write_str("\n");
            return None;
        };
        if config
            .add_store_object(ipc::BootStoreObjectConfig {
                id: object.id,
                base: module.address as u64,
                length: module.size,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: store object table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.state_volume_count() {
        let state = boot_manifest.state_volume(index)?;
        if config
            .add_state_volume(ipc::BootStateVolumeConfig { id: state.id })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: state volume table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.grant_count() {
        let grant = boot_manifest.grant(index)?;
        let rights = capability_rights_from_boot(grant.rights);
        if rights == 0 {
            serial::write_str("KrustBoot runtime plan failed: empty grant rights\n");
            return None;
        }
        if config
            .add_grant(ipc::BootGrantConfig {
                process_index: grant.process_index,
                cap_slot: grant.cap_slot,
                object_kind: grant.object_kind,
                object_index: grant.object_index,
                rights,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: grant table\n");
            return None;
        }
        index += 1;
    }

    Some(config)
}

fn capability_rights_from_boot(rights: u16) -> u64 {
    let mut out = 0;
    if rights & boot_manifest::RIGHT_SEND != 0 {
        out |= capability::RIGHT_SEND;
    }
    if rights & boot_manifest::RIGHT_RECEIVE != 0 {
        out |= capability::RIGHT_RECEIVE;
    }
    if rights & boot_manifest::RIGHT_READ != 0 {
        out |= capability::RIGHT_READ;
    }
    if rights & boot_manifest::RIGHT_WRITE != 0 {
        out |= capability::RIGHT_WRITE;
    }
    if rights & boot_manifest::RIGHT_SNAPSHOT != 0 {
        out |= capability::RIGHT_SNAPSHOT;
    }
    if rights & boot_manifest::RIGHT_RESTORE != 0 {
        out |= capability::RIGHT_RESTORE;
    }
    if rights & boot_manifest::RIGHT_CONTROL != 0 {
        out |= capability::RIGHT_CONTROL;
    }
    out
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

fn print_boot_modules() -> Option<boot_manifest::Manifest<'static>> {
    let Some(modules) = limine::modules() else {
        serial::write_str("Limine modules unavailable\n");
        return None;
    };

    serial::write_str("Limine modules: ");
    serial::write_u64_dec(modules.module_count());
    serial::write_str("\n");

    let mut parsed_manifest = None;
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

            if c_string_eq(module.string, boot_manifest::MODULE_STRING) {
                parsed_manifest = parse_boot_manifest_module(module);
            }
        }

        index += 1;
    }

    if parsed_manifest.is_none() {
        serial::write_str("KrustBoot manifest unavailable\n");
    }

    parsed_manifest
}

fn find_boot_manifest_module() -> Option<&'static limine::File> {
    find_module_by_string(boot_manifest::MODULE_STRING)
}

fn find_module_by_string(expected: &[u8]) -> Option<&'static limine::File> {
    let modules = limine::modules()?;

    let mut index = 0;
    while index < modules.module_count() {
        if let Some(module) = modules.module(index)
            && c_string_eq(module.string, expected)
        {
            return Some(module);
        }

        index += 1;
    }

    None
}

fn parse_boot_manifest_module(
    module: &'static limine::File,
) -> Option<boot_manifest::Manifest<'static>> {
    serial::write_str("KrustBoot manifest module: ");
    serial::write_c_string(module.path);
    serial::write_str(" bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");

    let bytes = unsafe { core::slice::from_raw_parts(module.address, module.size as usize) };
    match boot_manifest::parse(bytes) {
        Ok(manifest) => {
            print_boot_manifest(&manifest);
            Some(manifest)
        }
        Err(error) => {
            serial::write_str("KrustBoot manifest parse failed: ");
            print_boot_manifest_error(error);
            serial::write_str("\n");
            None
        }
    }
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

fn print_boot_manifest(manifest: &boot_manifest::Manifest<'static>) {
    serial::write_str("KrustBoot manifest generation: ");
    serial::write_str(manifest.generation_id());
    serial::write_str("\n");
    if !manifest.parent_generation_id().is_empty() {
        serial::write_str("KrustBoot parent generation: ");
        serial::write_str(manifest.parent_generation_id());
        serial::write_str("\n");
    }

    serial::write_str("KrustBoot boot modules: ");
    serial::write_u64_dec(manifest.boot_module_count() as u64);
    serial::write_str("\n");
    let mut index = 0;
    while index < manifest.boot_module_count() {
        if let Some(module) = manifest.boot_module(index) {
            serial::write_str("  boot_module[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] name=");
            serial::write_str(module.name);
            serial::write_str(" string=");
            serial::write_str(module.module_string);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot processes: ");
    serial::write_u64_dec(manifest.process_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.process_count() {
        if let Some(process) = manifest.process(index) {
            serial::write_str("  process[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] name=");
            serial::write_str(process.name);
            serial::write_str(" module=");
            serial::write_str(process.module_string);
            serial::write_str(" initial=");
            serial::write_str(if process.initial { "yes" } else { "no" });
            serial::write_str(" service=");
            serial::write_str(process.service_id);
            serial::write_str(" restart=");
            serial::write_u64_dec(process.restart_policy as u64);
            if !process.health_kind.is_empty() {
                serial::write_str(" health=");
                serial::write_str(process.health_kind);
            }
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot endpoints: ");
    serial::write_u64_dec(manifest.endpoint_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.endpoint_count() {
        if let Some(endpoint) = manifest.endpoint(index) {
            serial::write_str("  endpoint[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] name=");
            serial::write_str(endpoint.name);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot grants: ");
    serial::write_u64_dec(manifest.grant_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.grant_count() {
        if let Some(grant) = manifest.grant(index) {
            let process = manifest.process(grant.process_index);
            serial::write_str("  grant[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] process=");
            serial::write_str(process.map(|process| process.name).unwrap_or("<bad>"));
            serial::write_str(" cap[");
            serial::write_u64_dec(grant.cap_slot);
            serial::write_str("] ");
            print_boot_grant_object(manifest, grant.object_kind, grant.object_index);
            serial::write_str(" rights=");
            print_boot_grant_rights(grant.rights);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot store objects: ");
    serial::write_u64_dec(manifest.store_object_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.store_object_count() {
        if let Some(object) = manifest.store_object(index) {
            serial::write_str("  store_object[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(object.id);
            serial::write_str(" module=");
            serial::write_str(object.module_string);
            serial::write_str(" hash=");
            serial::write_str(object.hash);
            serial::write_str(" size=");
            serial::write_u64_dec(object.size);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot state volumes: ");
    serial::write_u64_dec(manifest.state_volume_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.state_volume_count() {
        if let Some(state) = manifest.state_volume(index) {
            serial::write_str("  state_volume[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(state.id);
            serial::write_str("\n");
        }
        index += 1;
    }
}

fn print_boot_grant_object(
    manifest: &boot_manifest::Manifest<'static>,
    object_kind: u16,
    object_index: usize,
) {
    match object_kind {
        boot_manifest::OBJECT_ENDPOINT => {
            serial::write_str("endpoint=");
            serial::write_str(
                manifest
                    .endpoint(object_index)
                    .map(|endpoint| endpoint.name)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_STORE => {
            serial::write_str("store-object=");
            serial::write_str(
                manifest
                    .store_object(object_index)
                    .map(|object| object.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_STATE => {
            serial::write_str("state-volume=");
            serial::write_str(
                manifest
                    .state_volume(object_index)
                    .map(|state| state.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_TIMER => {
            serial::write_str("timer=monotonic-timer");
        }
        _ => serial::write_str("object=<bad>"),
    }
}

fn print_boot_grant_rights(rights: u16) {
    let mut wrote = false;
    if rights & boot_manifest::RIGHT_SEND != 0 {
        serial::write_str("send");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_RECEIVE != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("receive");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_READ != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("read");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_WRITE != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("write");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_SNAPSHOT != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("snapshot");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_RESTORE != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("restore");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_CONTROL != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("control");
        wrote = true;
    }
    if !wrote {
        serial::write_str("none");
    }
}

fn print_boot_manifest_error(error: boot_manifest::ParseError) {
    match error {
        boot_manifest::ParseError::Truncated => serial::write_str("truncated"),
        boot_manifest::ParseError::BadMagic => serial::write_str("bad magic"),
        boot_manifest::ParseError::UnsupportedVersion => serial::write_str("unsupported version"),
        boot_manifest::ParseError::TooManyBootModules => serial::write_str("too many boot modules"),
        boot_manifest::ParseError::TooManyProcesses => serial::write_str("too many processes"),
        boot_manifest::ParseError::TooManyEndpoints => serial::write_str("too many endpoints"),
        boot_manifest::ParseError::TooManyGrants => serial::write_str("too many grants"),
        boot_manifest::ParseError::TooManyStoreObjects => {
            serial::write_str("too many store objects")
        }
        boot_manifest::ParseError::TooManyStateVolumes => {
            serial::write_str("too many state volumes")
        }
        boot_manifest::ParseError::InvalidString => serial::write_str("invalid string"),
        boot_manifest::ParseError::InvalidReference => serial::write_str("invalid reference"),
        boot_manifest::ParseError::InvalidRights => serial::write_str("invalid rights"),
        boot_manifest::ParseError::InvalidObjectKind => serial::write_str("invalid object kind"),
        boot_manifest::ParseError::TrailingBytes => serial::write_str("trailing bytes"),
    }
}
