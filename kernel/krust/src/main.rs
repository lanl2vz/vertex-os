#![no_std]
#![no_main]

mod arena;
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
mod timer;
mod usercopy;
mod userspace;

use core::panic::PanicInfo;
use core::{arch::asm, cell::UnsafeCell};

const MAX_BOOT_PROCESSES: usize = 16;
const DMA_KERNEL_ALLOCATED_BASE: u64 = u64::MAX;
const VERTEXDISK_MODULE_STRING: &[u8] = b"vertexdisk-store";
const VERTEX_DISK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
const VERTEX_DISK_VERSION: u16 = 1;
const VERTEX_DISK_SECTOR_SIZE: usize = 512;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = 20;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = 24;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = 32;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = 16;
const VERTEX_DISK_STORE_INDEX_SECTION: usize = 1;
const VERTEX_DISK_STORE_DATA_SECTION: usize = 2;
const STORE_ENTRY_OFFSET: usize = 32;
const STORE_ENTRY_LEN: usize = 144;

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static SELECTED_BOOT_CONFIG: Global<ipc::BootRuntimeConfig> =
    Global(UnsafeCell::new(ipc::BootRuntimeConfig::new()));
static FALLBACK_BOOT_CONFIG: Global<ipc::BootRuntimeConfig> =
    Global(UnsafeCell::new(ipc::BootRuntimeConfig::new()));
static BAD_GENERATION_BOOT_CONFIG: Global<ipc::BootRuntimeConfig> =
    Global(UnsafeCell::new(ipc::BootRuntimeConfig::new()));

struct BootManifests {
    selected: &'static boot_manifest::Manifest<'static>,
    fallback: Option<&'static boot_manifest::Manifest<'static>>,
    bad_generation: Option<&'static boot_manifest::Manifest<'static>>,
}

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

    let Some(boot_manifests) = print_boot_modules() else {
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
    run_typed_arena_demo(heap);
    ipc::run_fifo_regression();
    run_native_boot(&mut allocator, &boot_manifests);
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
    let com1_ports = match table.add_object(
        capability::KernelObjectKind::IoPortRange,
        "io:com1",
        0x3f8,
        8,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: COM1 I/O object\n");
            return;
        }
    };
    let pci_config_ports = match table.add_object(
        capability::KernelObjectKind::IoPortRange,
        "io:pci-config",
        0x0cf8,
        8,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: PCI config I/O object\n");
            return;
        }
    };
    let virtio_ports = match table.add_object(
        capability::KernelObjectKind::IoPortRange,
        "io:virtio-blk0",
        0xc000,
        0x1000,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: virtio I/O object\n");
            return;
        }
    };
    let virtio_irq = match table.add_object(
        capability::KernelObjectKind::InterruptLine,
        "irq:virtio-blk0",
        11,
        1,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: virtio IRQ object\n");
            return;
        }
    };
    let virtio_dma = match table.add_object(
        capability::KernelObjectKind::DmaRegion,
        "dma:virtio-blk0",
        0,
        0x4000,
    ) {
        Ok(id) => id,
        Err(_) => {
            serial::write_str("Capability table demo failed: virtio DMA object\n");
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
        || table
            .grant(com1_ports, capability::RIGHT_READ | capability::RIGHT_WRITE)
            .is_err()
        || table
            .grant(
                pci_config_ports,
                capability::RIGHT_READ | capability::RIGHT_WRITE,
            )
            .is_err()
        || table
            .grant(
                virtio_ports,
                capability::RIGHT_READ | capability::RIGHT_WRITE,
            )
            .is_err()
        || table.grant(virtio_irq, capability::RIGHT_LISTEN).is_err()
        || table
            .grant(
                virtio_dma,
                capability::RIGHT_READ | capability::RIGHT_WRITE | capability::RIGHT_MAP,
            )
            .is_err()
    {
        serial::write_str("Capability table demo failed: grant\n");
        return;
    }

    table.print();

    if table.object_count() == 11 && table.capability_count() == 11 {
        serial::write_str("Capability table demo ok\n");
    } else {
        serial::write_str("Capability table demo failed: count mismatch\n");
    }
}

#[derive(Clone, Copy)]
struct DemoEndpoint {
    _id: u64,
}

#[derive(Clone, Copy)]
struct DemoProcess {
    _id: u64,
}

fn run_typed_arena_demo(heap: KernelHeapMapping) {
    let mut kernel_heap = arena::KernelHeap::new(heap.base, heap.length);
    let endpoint_arena_mem = kernel_heap.alloc(4096, 4096);
    let process_arena_mem = kernel_heap.alloc(4096, 4096);
    if endpoint_arena_mem.is_none() || process_arena_mem.is_none() {
        serial::write_str("Kernel heap arena allocation failed\n");
        return;
    }

    let mut endpoints = arena::TypedArena::<DemoEndpoint, 32>::new();
    let mut processes = arena::TypedArena::<DemoProcess, 32>::new();
    let mut endpoint_handles = [None; 32];
    let mut process_handles = [None; 32];

    let mut index = 0;
    while index < 32 {
        endpoint_handles[index] = match endpoints.alloc(DemoEndpoint { _id: index as u64 }) {
            Ok(handle) => Some(handle),
            Err(_) => {
                serial::write_str("Typed endpoint arena failed before 32 allocations\n");
                return;
            }
        };
        process_handles[index] = match processes.alloc(DemoProcess { _id: index as u64 }) {
            Ok(handle) => Some(handle),
            Err(_) => {
                serial::write_str("Typed process arena failed before 32 allocations\n");
                return;
            }
        };
        index += 1;
    }

    if endpoints.alloc(DemoEndpoint { _id: 99 }).is_ok()
        || processes.alloc(DemoProcess { _id: 99 }).is_ok()
    {
        serial::write_str("Typed arena capacity failure test failed\n");
        return;
    }

    let Some(freed_endpoint) = endpoint_handles[7] else {
        serial::write_str("Typed endpoint arena missing handle\n");
        return;
    };
    let Some(freed_process) = process_handles[7] else {
        serial::write_str("Typed process arena missing handle\n");
        return;
    };
    if endpoints.free(freed_endpoint).is_err() || processes.free(freed_process).is_err() {
        serial::write_str("Typed arena free failed\n");
        return;
    }
    let reused_endpoint = match endpoints.alloc(DemoEndpoint { _id: 77 }) {
        Ok(handle) => handle,
        Err(_) => {
            serial::write_str("Typed endpoint arena reuse failed\n");
            return;
        }
    };
    let reused_process = match processes.alloc(DemoProcess { _id: 77 }) {
        Ok(handle) => handle,
        Err(_) => {
            serial::write_str("Typed process arena reuse failed\n");
            return;
        }
    };

    if reused_endpoint.index() == freed_endpoint.index()
        && reused_process.index() == freed_process.index()
        && endpoints.live() == 32
        && processes.live() == 32
    {
        serial::write_str("Kernel heap arena allocation ok\n");
        serial::write_str("Typed endpoint arena created 32 endpoints\n");
        serial::write_str("Typed process arena created 32 processes\n");
        serial::write_str("Typed arena free and reuse ok\n");
        serial::write_str("Typed arena allocation failure returned controlled error\n");
        serial::write_str("Typed object arenas no silent overwrite ok\n");
    } else {
        serial::write_str("Typed arena reuse mismatch\n");
    }
}

fn run_native_boot(allocator: &mut memory::FrameAllocator, boot_manifests: &BootManifests) {
    let Some(config) = prepare_native_boot_config(
        allocator,
        boot_manifests.selected,
        boot_manifest::MODULE_STRING,
        "krustboot-manifest",
        &SELECTED_BOOT_CONFIG,
    ) else {
        serial::write_str("Native runtime init failed from KrustBoot manifest\n");
        serial::write_str("Native service activation failed\n");
        return;
    };
    if ipc::register_generation_config(config).is_err() {
        serial::write_str("KrustBoot selected generation registration failed\n");
        return;
    }

    if let Some(fallback_manifest) = boot_manifests.fallback {
        if fallback_manifest.generation_id() == boot_manifests.selected.generation_id() {
            serial::write_str(
                "KrustBoot fallback manifest matches selected generation; ignoring\n",
            );
        } else if let Some(fallback_config) = prepare_native_boot_config(
            allocator,
            fallback_manifest,
            boot_manifest::FALLBACK_MODULE_STRING,
            "krustboot-fallback-manifest",
            &FALLBACK_BOOT_CONFIG,
        ) {
            if ipc::register_generation_config(fallback_config).is_err() {
                serial::write_str("KrustBoot fallback generation registration failed\n");
                return;
            }
            ipc::set_rollback_boot_config(fallback_config);
            serial::write_str("KrustBoot fallback generation ready: ");
            serial::write_str(fallback_manifest.generation_id());
            serial::write_str("\n");
        } else {
            serial::write_str("KrustBoot fallback runtime plan unavailable\n");
            return;
        }
    }

    if let Some(bad_generation_manifest) = boot_manifests.bad_generation {
        if bad_generation_manifest.generation_id() == boot_manifests.selected.generation_id()
            || boot_manifests
                .fallback
                .map(|fallback| fallback.generation_id() == bad_generation_manifest.generation_id())
                .unwrap_or(false)
        {
            serial::write_str(
                "KrustBoot bad generation manifest matches an active generation; ignoring\n",
            );
        } else if let Some(bad_generation_config) = prepare_native_boot_config(
            allocator,
            bad_generation_manifest,
            boot_manifest::BAD_GENERATION_MODULE_STRING,
            "krustboot-bad-generation-manifest",
            &BAD_GENERATION_BOOT_CONFIG,
        ) {
            if ipc::register_generation_config(bad_generation_config).is_err() {
                serial::write_str("KrustBoot bad generation registration failed\n");
                return;
            }
            serial::write_str("KrustBoot bad generation ready: ");
            serial::write_str(bad_generation_manifest.generation_id());
            serial::write_str("\n");
        } else {
            serial::write_str("KrustBoot bad generation runtime plan unavailable\n");
            return;
        }
    }

    if ipc::init_from_boot_config(config).is_err() {
        serial::write_str("Native runtime init failed from KrustBoot manifest\n");
        return;
    }
    ipc::install_frame_allocator(allocator as *mut memory::FrameAllocator);

    let Some(initial) = ipc::initial_process_context() else {
        serial::write_str("Native runtime init failed: no initial process\n");
        return;
    };

    userspace::enter_initial_process(ipc::initial_process_name(), initial);
}

fn prepare_native_boot_config(
    allocator: &mut memory::FrameAllocator,
    boot_manifest: &boot_manifest::Manifest<'static>,
    manifest_module_string: &[u8],
    manifest_module_name: &'static str,
    config_slot: &'static Global<ipc::BootRuntimeConfig>,
) -> Option<&'static ipc::BootRuntimeConfig> {
    let Some(hhdm_offset) = limine::hhdm_offset() else {
        serial::write_str("Native userspace load failed: HHDM unavailable\n");
        return None;
    };

    let mut images = [None; MAX_BOOT_PROCESSES];
    let mut restart_images = [None; MAX_BOOT_PROCESSES];
    let mut index = 0;
    while index < boot_manifest.process_count() {
        let Some(process) = boot_manifest.process(index) else {
            serial::write_str("KrustBoot IPC plan failed: process gap\n");
            return None;
        };
        let Some(image) = load_boot_process_image(boot_manifest, process, hhdm_offset, allocator)
        else {
            return None;
        };
        images[index] = Some(image);
        let Some(restart_image) =
            load_boot_process_image(boot_manifest, process, hhdm_offset, allocator)
        else {
            return None;
        };
        restart_images[index] = Some(restart_image);
        index += 1;
    }

    let config = unsafe { &mut *config_slot.0.get() };
    *config = ipc::BootRuntimeConfig::new();
    config.set_generation_id(boot_manifest.generation_id());
    build_boot_runtime_config(
        boot_manifest,
        &images,
        &restart_images,
        hhdm_offset,
        allocator,
        config,
    )?;
    let Some(_manifest_module) = find_module_by_string(manifest_module_string) else {
        serial::write_str("KrustBoot runtime init failed: manifest module missing\n");
        return None;
    };
    config.set_manifest_module(ipc::BootModuleConfig {
        name: manifest_module_name,
        base: boot_manifest.source_base(),
        length: boot_manifest.source_len(),
    });
    Some(unsafe { &*config_slot.0.get() })
}

fn load_boot_process_image(
    boot_manifest: &boot_manifest::Manifest<'static>,
    process: boot_manifest::Process<'static>,
    hhdm_offset: u64,
    allocator: &mut memory::FrameAllocator,
) -> Option<userspace::UserImage> {
    let Some(object) = store_object_for_module(boot_manifest, process.module_string) else {
        serial::write_str("KrustBoot process store object unavailable: process=");
        serial::write_str(process.name);
        serial::write_str(" module=");
        serial::write_str(process.module_string);
        serial::write_str("\n");
        return None;
    };
    let Some(store_object) = load_native_store_object(object.id) else {
        serial::write_str("KrustBoot native store object unavailable for process: process=");
        serial::write_str(process.name);
        serial::write_str(" object=");
        serial::write_str(object.id);
        serial::write_str("\n");
        return None;
    };
    if store_object.bytes.len() as u64 != object.size {
        serial::write_str("Krust process executable size mismatch: process=");
        serial::write_str(process.name);
        serial::write_str(" object=");
        serial::write_str(object.id);
        serial::write_str(" expected=");
        serial::write_u64_dec(object.size);
        serial::write_str(" actual=");
        serial::write_u64_dec(store_object.bytes.len() as u64);
        serial::write_str("\n");
        return None;
    }

    serial::write_str("Krust process executable store object: process=");
    serial::write_str(process.name);
    serial::write_str(" object=");
    serial::write_str(object.id);
    serial::write_str(" identity=store:blake3:");
    serial::write_str(object.hash);
    serial::write_str(" bytes=");
    serial::write_u64_dec(store_object.bytes.len() as u64);
    serial::write_str("\n");

    if checksum32(store_object.bytes) != store_object.checksum {
        serial::write_str("Krust process executable checksum mismatch: process=");
        serial::write_str(process.name);
        serial::write_str(" object=");
        serial::write_str(object.id);
        serial::write_str("\n");
        serial::write_str("vertex-inspect security event: store hash mismatch object=");
        serial::write_str(object.id);
        serial::write_str("\n");
        return None;
    }
    if !store_hash_matches(store_object.bytes, object.hash) {
        serial::write_str("Krust process executable hash mismatch: process=");
        serial::write_str(process.name);
        serial::write_str(" object=");
        serial::write_str(object.id);
        serial::write_str("\n");
        serial::write_str("vertex-inspect security event: store hash mismatch object=");
        serial::write_str(object.id);
        serial::write_str("\n");
        return None;
    }
    serial::write_str("store hash verified before process creation: process=");
    serial::write_str(process.name);
    serial::write_str("\n");
    match userspace::load(store_object.bytes, hhdm_offset, allocator) {
        Ok(image) => {
            serial::write_str("Krust process image loaded from native store: process=");
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
    restart_images: &[Option<userspace::UserImage>; MAX_BOOT_PROCESSES],
    hhdm_offset: u64,
    allocator: &mut memory::FrameAllocator,
    config: &mut ipc::BootRuntimeConfig,
) -> Option<()> {
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
        let Some(restart_image) = restart_images[index] else {
            serial::write_str("KrustBoot runtime plan failed: restart image gap\n");
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
                restart_context: ipc::ProcessContext {
                    cr3: restart_image.cr3,
                    entry: restart_image.entry,
                    stack_top: restart_image.stack_top,
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
        let Some(store_object) = load_native_store_object(object.id) else {
            serial::write_str("KrustBoot native store object unavailable: object=");
            serial::write_str(object.id);
            serial::write_str("\n");
            return None;
        };
        if store_object.bytes.len() as u64 != object.size {
            serial::write_str("Krust native store size mismatch: object=");
            serial::write_str(object.id);
            serial::write_str(" expected=");
            serial::write_u64_dec(object.size);
            serial::write_str(" actual=");
            serial::write_u64_dec(store_object.bytes.len() as u64);
            serial::write_str("\n");
            return None;
        }
        serial::write_str("Krust native store indexed object: object=");
        serial::write_str(object.id);
        serial::write_str(" identity=store:blake3:");
        serial::write_str(object.hash);
        serial::write_str(" bytes=");
        serial::write_u64_dec(store_object.bytes.len() as u64);
        serial::write_str("\n");
        if config
            .add_store_object(ipc::BootStoreObjectConfig {
                id: object.id,
                base: store_object.bytes.as_ptr() as u64,
                length: store_object.bytes.len() as u64,
                hash: object.hash,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: store object table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.network_port_count() {
        let port = boot_manifest.network_port(index)?;
        if config
            .add_network_port(ipc::BootNetworkPortConfig { id: port.id })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: network port table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.io_port_count() {
        let port = boot_manifest.io_port(index)?;
        if config
            .add_io_port(ipc::BootIoPortRangeConfig {
                id: port.id,
                base: port.base,
                length: port.length,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: io port table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.mmio_region_count() {
        let region = boot_manifest.mmio_region(index)?;
        if config
            .add_mmio_region(ipc::BootMmioRegionConfig {
                id: region.id,
                base: region.base,
                length: region.length,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: mmio region table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.interrupt_line_count() {
        let line = boot_manifest.interrupt_line(index)?;
        if config
            .add_interrupt_line(ipc::BootInterruptLineConfig {
                id: line.id,
                line: line.line,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: interrupt line table\n");
            return None;
        }
        index += 1;
    }

    index = 0;
    while index < boot_manifest.dma_region_count() {
        let region = boot_manifest.dma_region(index)?;
        let base = if region.base == DMA_KERNEL_ALLOCATED_BASE {
            allocate_dma_region(region.id, region.length, hhdm_offset, allocator)?
        } else if region.base == 0 {
            serial::write_str("KrustBoot runtime plan failed: dma region base zero\n");
            return None;
        } else {
            region.base
        };
        if config
            .add_dma_region(ipc::BootDmaRegionConfig {
                id: region.id,
                base,
                length: region.length,
            })
            .is_err()
        {
            serial::write_str("KrustBoot runtime plan failed: dma region table\n");
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

    Some(())
}

fn allocate_dma_region(
    id: &'static str,
    length: u64,
    hhdm_offset: u64,
    allocator: &mut memory::FrameAllocator,
) -> Option<u64> {
    let frames = length
        .checked_add(memory::FRAME_SIZE - 1)?
        .checked_div(memory::FRAME_SIZE)?;
    let frame = allocator.allocate_contiguous(frames)?;
    unsafe {
        core::ptr::write_bytes(
            (hhdm_offset + frame.start()) as *mut u8,
            0,
            (frames * memory::FRAME_SIZE) as usize,
        );
    }
    serial::write_str("KrustBoot allocated DMA region: id=");
    serial::write_str(id);
    serial::write_str(" base=");
    serial::write_u64_hex(frame.start());
    serial::write_str(" length=");
    serial::write_u64_hex(frames * memory::FRAME_SIZE);
    serial::write_str("\n");
    Some(frame.start())
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
    if rights & boot_manifest::RIGHT_BIND != 0 {
        out |= capability::RIGHT_BIND;
    }
    if rights & boot_manifest::RIGHT_LISTEN != 0 {
        out |= capability::RIGHT_LISTEN;
    }
    if rights & boot_manifest::RIGHT_MAP != 0 {
        out |= capability::RIGHT_MAP;
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

fn print_boot_modules() -> Option<BootManifests> {
    let Some(modules) = limine::modules() else {
        serial::write_str("Limine modules unavailable\n");
        return None;
    };

    serial::write_str("Limine modules: ");
    serial::write_u64_dec(modules.module_count());
    serial::write_str("\n");

    let mut selected_manifest = None;
    let mut fallback_manifest = None;
    let mut bad_generation_manifest = None;
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
                selected_manifest = parse_boot_manifest_module(module, false);
            } else if c_string_eq(module.string, boot_manifest::FALLBACK_MODULE_STRING) {
                fallback_manifest = parse_boot_manifest_module(module, true);
            } else if c_string_eq(module.string, boot_manifest::BAD_GENERATION_MODULE_STRING) {
                bad_generation_manifest = parse_bad_generation_manifest_module(module);
            }
        }

        index += 1;
    }

    if selected_manifest.is_none() {
        serial::write_str("KrustBoot manifest unavailable\n");
    }

    selected_manifest.map(|selected| BootManifests {
        selected,
        fallback: fallback_manifest,
        bad_generation: bad_generation_manifest,
    })
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

fn store_object_for_module(
    manifest: &boot_manifest::Manifest<'static>,
    module_string: &str,
) -> Option<boot_manifest::StoreObject<'static>> {
    let mut index = 0;
    while index < manifest.store_object_count() {
        if let Some(object) = manifest.store_object(index)
            && object.module_string == module_string
        {
            return Some(object);
        }
        index += 1;
    }
    None
}

struct NativeStoreObject {
    bytes: &'static [u8],
    checksum: u32,
}

fn load_native_store_object(id: &str) -> Option<NativeStoreObject> {
    let disk = native_vertexdisk_bytes()?;
    let superblock = disk.get(..VERTEX_DISK_SECTOR_SIZE)?;
    if !valid_vertexdisk_superblock(superblock) {
        serial::write_str("Krust native VertexDisk superblock rejected\n");
        return None;
    }

    let (store_index_start, store_index_count) =
        vertexdisk_section(superblock, VERTEX_DISK_STORE_INDEX_SECTION)?;
    let (store_data_start, store_data_count) =
        vertexdisk_section(superblock, VERTEX_DISK_STORE_DATA_SECTION)?;
    let index = vertexdisk_section_bytes(disk, store_index_start, store_index_count)?;
    if !valid_store_index(index) {
        serial::write_str("Krust native store index rejected\n");
        return None;
    }

    let count = read_u16(index, 18) as usize;
    let mut item = 0;
    while item < count {
        let offset = STORE_ENTRY_OFFSET + item * STORE_ENTRY_LEN;
        if offset + STORE_ENTRY_LEN > index.len() {
            serial::write_str("Krust native store index bounds invalid\n");
            return None;
        }
        if fixed_string_eq(index, offset, id.as_bytes()) {
            let data_sector = read_u64(index, offset + 64);
            let byte_len = read_u32(index, offset + 72) as usize;
            let checksum = read_u32(index, offset + 76);
            if !store_entry_bounds_valid(data_sector, byte_len, store_data_start, store_data_count)
            {
                serial::write_str("Krust native store object bounds invalid: object=");
                serial::write_str(id);
                serial::write_str("\n");
                return None;
            }
            let data_offset = sector_byte_offset(data_sector)?;
            let bytes = disk.get(data_offset..data_offset.checked_add(byte_len)?)?;
            return Some(NativeStoreObject { bytes, checksum });
        }
        item += 1;
    }

    serial::write_str("Krust native store object missing: object=");
    serial::write_str(id);
    serial::write_str("\n");
    None
}

fn native_vertexdisk_bytes() -> Option<&'static [u8]> {
    let module = find_module_by_string(VERTEXDISK_MODULE_STRING)?;
    Some(unsafe { core::slice::from_raw_parts(module.address, module.size as usize) })
}

fn valid_vertexdisk_superblock(sector: &[u8]) -> bool {
    if sector.len() < VERTEX_DISK_SECTOR_SIZE
        || !starts_with(sector, VERTEX_DISK_MAGIC)
        || read_u16(sector, 16) != VERTEX_DISK_VERSION
        || read_u16(sector, 18) != VERTEX_DISK_SECTOR_SIZE as u16
        || !metadata_checksum_valid(sector)
    {
        return false;
    }

    let total_sectors = read_u32(sector, VERTEX_DISK_TOTAL_SECTORS_OFFSET) as u64;
    let mut section = 0;
    while section <= VERTEX_DISK_STORE_DATA_SECTION {
        let Some((start, count)) = vertexdisk_section(sector, section) else {
            return false;
        };
        if count == 0
            || start
                .checked_add(count)
                .is_none_or(|end| end > total_sectors)
        {
            return false;
        }
        section += 1;
    }
    true
}

fn valid_store_index(index: &[u8]) -> bool {
    starts_with(index, STORE_INDEX_MAGIC)
        && read_u16(index, 16) == VERTEX_DISK_VERSION
        && metadata_checksum_valid(index)
}

fn vertexdisk_section(sector: &[u8], section: usize) -> Option<(u64, u64)> {
    let offset = VERTEX_DISK_SECTION_TABLE_OFFSET + section * VERTEX_DISK_SECTION_RECORD_LEN;
    if offset + 16 > sector.len() {
        return None;
    }
    Some((read_u64(sector, offset), read_u64(sector, offset + 8)))
}

fn vertexdisk_section_bytes(disk: &'static [u8], start: u64, count: u64) -> Option<&'static [u8]> {
    let offset = sector_byte_offset(start)?;
    let len = usize::try_from(count)
        .ok()?
        .checked_mul(VERTEX_DISK_SECTOR_SIZE)?;
    disk.get(offset..offset.checked_add(len)?)
}

fn store_entry_bounds_valid(
    data_sector: u64,
    byte_len: usize,
    store_data_start: u64,
    store_data_count: u64,
) -> bool {
    if byte_len == 0 {
        return false;
    }
    let sectors = sectors_for_len(byte_len) as u64;
    data_sector >= store_data_start
        && data_sector
            .checked_add(sectors)
            .is_some_and(|end| end <= store_data_start + store_data_count)
}

fn sector_byte_offset(sector: u64) -> Option<usize> {
    usize::try_from(sector)
        .ok()?
        .checked_mul(VERTEX_DISK_SECTOR_SIZE)
}

fn sectors_for_len(len: usize) -> usize {
    len.div_ceil(VERTEX_DISK_SECTOR_SIZE).max(1)
}

fn metadata_checksum_valid(bytes: &[u8]) -> bool {
    if bytes.len() < VERTEX_DISK_CHECKSUM_OFFSET + 4 {
        return false;
    }
    let stored = read_u32(bytes, VERTEX_DISK_CHECKSUM_OFFSET);
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        let byte =
            if index >= VERTEX_DISK_CHECKSUM_OFFSET && index < VERTEX_DISK_CHECKSUM_OFFSET + 4 {
                0
            } else {
                bytes[index]
            };
        checksum = checksum.wrapping_add((byte as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum == stored
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        checksum = checksum.wrapping_add((bytes[index] as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum
}

fn fixed_string_eq(buffer: &[u8], offset: usize, value: &[u8]) -> bool {
    if offset + 64 > buffer.len() || value.len() > 64 {
        return false;
    }
    let mut index = 0;
    while index < value.len() {
        if buffer[offset + index] != value[index] {
            return false;
        }
        index += 1;
    }
    value.len() == 64 || buffer[offset + value.len()] == 0
}

fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    if bytes.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if bytes[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn store_hash_matches(bytes: &[u8], expected: &str) -> bool {
    if expected.len() != 64 {
        return false;
    }
    let mut actual = [0u8; 64];
    store_hash_hex(blake3::hash(bytes).as_bytes(), &mut actual);
    actual == expected.as_bytes()
}

fn store_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0xf) as usize];
        index += 1;
    }
}

fn parse_boot_manifest_module(
    module: &'static limine::File,
    fallback: bool,
) -> Option<&'static boot_manifest::Manifest<'static>> {
    serial::write_str("KrustBoot manifest module: ");
    serial::write_c_string(module.path);
    serial::write_str(" bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");

    let bytes = unsafe { core::slice::from_raw_parts(module.address, module.size as usize) };
    let parsed = if fallback {
        boot_manifest::parse_fallback(bytes)
    } else {
        boot_manifest::parse_selected(bytes)
    };
    match parsed {
        Ok(manifest) => {
            if !fallback {
                print_boot_manifest(manifest);
            }
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

fn parse_bad_generation_manifest_module(
    module: &'static limine::File,
) -> Option<&'static boot_manifest::Manifest<'static>> {
    serial::write_str("KrustBoot bad generation manifest module: ");
    serial::write_c_string(module.path);
    serial::write_str(" bytes=");
    serial::write_u64_dec(module.size);
    serial::write_str("\n");

    let bytes = unsafe { core::slice::from_raw_parts(module.address, module.size as usize) };
    match boot_manifest::parse_bad_generation(bytes) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            serial::write_str("KrustBoot bad generation manifest parse failed: ");
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
    if manifest.layout_version() != 0 {
        serial::write_str("KrustBoot Manifest v");
        serial::write_u64_dec(manifest.layout_version() as u64);
        serial::write_str(" records: ");
        serial::write_u64_dec(manifest.record_count() as u64);
        serial::write_str("\n");
    }
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

    serial::write_str("KrustBoot network ports: ");
    serial::write_u64_dec(manifest.network_port_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.network_port_count() {
        if let Some(port) = manifest.network_port(index) {
            serial::write_str("  network_port[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(port.id);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot io port ranges: ");
    serial::write_u64_dec(manifest.io_port_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.io_port_count() {
        if let Some(port) = manifest.io_port(index) {
            serial::write_str("  io_port[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(port.id);
            serial::write_str(" base=");
            serial::write_u64_hex(port.base);
            serial::write_str(" length=");
            serial::write_u64_hex(port.length);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot mmio regions: ");
    serial::write_u64_dec(manifest.mmio_region_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.mmio_region_count() {
        if let Some(region) = manifest.mmio_region(index) {
            serial::write_str("  mmio_region[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(region.id);
            serial::write_str(" base=");
            serial::write_u64_hex(region.base);
            serial::write_str(" length=");
            serial::write_u64_hex(region.length);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot interrupt lines: ");
    serial::write_u64_dec(manifest.interrupt_line_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.interrupt_line_count() {
        if let Some(line) = manifest.interrupt_line(index) {
            serial::write_str("  interrupt_line[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(line.id);
            serial::write_str(" line=");
            serial::write_u64_dec(line.line);
            serial::write_str("\n");
        }
        index += 1;
    }

    serial::write_str("KrustBoot dma regions: ");
    serial::write_u64_dec(manifest.dma_region_count() as u64);
    serial::write_str("\n");
    index = 0;
    while index < manifest.dma_region_count() {
        if let Some(region) = manifest.dma_region(index) {
            serial::write_str("  dma_region[");
            serial::write_u64_dec(index as u64);
            serial::write_str("] id=");
            serial::write_str(region.id);
            serial::write_str(" base=");
            serial::write_u64_hex(region.base);
            serial::write_str(" length=");
            serial::write_u64_hex(region.length);
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
            serial::write_str("unsupported-state-object=");
            serial::write_u64_dec(object_index as u64);
        }
        boot_manifest::OBJECT_TIMER => {
            serial::write_str("timer=monotonic-timer");
        }
        boot_manifest::OBJECT_NETWORK_PORT => {
            serial::write_str("network-port=");
            serial::write_str(
                manifest
                    .network_port(object_index)
                    .map(|port| port.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_IO_PORT_RANGE => {
            serial::write_str("io-port=");
            serial::write_str(
                manifest
                    .io_port(object_index)
                    .map(|port| port.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_MMIO_REGION => {
            serial::write_str("mmio-region=");
            serial::write_str(
                manifest
                    .mmio_region(object_index)
                    .map(|region| region.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_INTERRUPT_LINE => {
            serial::write_str("interrupt-line=");
            serial::write_str(
                manifest
                    .interrupt_line(object_index)
                    .map(|line| line.id)
                    .unwrap_or("<bad>"),
            );
        }
        boot_manifest::OBJECT_DMA_REGION => {
            serial::write_str("dma-region=");
            serial::write_str(
                manifest
                    .dma_region(object_index)
                    .map(|region| region.id)
                    .unwrap_or("<bad>"),
            );
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
    if rights & boot_manifest::RIGHT_MAP != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("map");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_CONTROL != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("control");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_BIND != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("bind");
        wrote = true;
    }
    if rights & boot_manifest::RIGHT_LISTEN != 0 {
        if wrote {
            serial::write_str("|");
        }
        serial::write_str("listen");
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
        boot_manifest::ParseError::TooManyNetworkPorts => {
            serial::write_str("too many network ports")
        }
        boot_manifest::ParseError::TooManyIoPortRanges => {
            serial::write_str("too many io port ranges")
        }
        boot_manifest::ParseError::TooManyMmioRegions => serial::write_str("too many mmio regions"),
        boot_manifest::ParseError::TooManyInterruptLines => {
            serial::write_str("too many interrupt lines")
        }
        boot_manifest::ParseError::TooManyDmaRegions => serial::write_str("too many dma regions"),
        boot_manifest::ParseError::InvalidString => serial::write_str("invalid string"),
        boot_manifest::ParseError::InvalidReference => serial::write_str("invalid reference"),
        boot_manifest::ParseError::InvalidRights => serial::write_str("invalid rights"),
        boot_manifest::ParseError::InvalidObjectKind => serial::write_str("invalid object kind"),
        boot_manifest::ParseError::UnsupportedStateVolumes => {
            serial::write_str("unsupported state volumes")
        }
        boot_manifest::ParseError::TrailingBytes => serial::write_str("trailing bytes"),
        boot_manifest::ParseError::BadChecksum => serial::write_str("bad checksum"),
        boot_manifest::ParseError::BadRecordTable => serial::write_str("bad record table"),
        boot_manifest::ParseError::OutOfBoundsRecord => serial::write_str("out-of-bounds record"),
    }
}
