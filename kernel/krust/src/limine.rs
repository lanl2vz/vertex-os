use core::cell::UnsafeCell;
use core::ptr;

const COMMON_MAGIC_0: u64 = 0xc7b1dd30df4c8b88;
const COMMON_MAGIC_1: u64 = 0x0a82e883a194f07b;

const REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8f4b39de7d1ae,
    0xfab91a6940fcb9cf,
    0x785c6ed015d3e316,
    0x181e920a7852b9d9,
];
const REQUESTS_END_MARKER: [u64; 2] = [0xadc0e0531bb10d03, 0x9572709f31764c62];

const MEMMAP_REQUEST_ID: [u64; 4] = [
    COMMON_MAGIC_0,
    COMMON_MAGIC_1,
    0x67cf3d9d378a806f,
    0xe304acdfc50c3c62,
];
const HHDM_REQUEST_ID: [u64; 4] = [
    COMMON_MAGIC_0,
    COMMON_MAGIC_1,
    0x48dcf1cb8ad2b852,
    0x63984e959a98244b,
];
const MODULE_REQUEST_ID: [u64; 4] = [
    COMMON_MAGIC_0,
    COMMON_MAGIC_1,
    0x3e7e279702be32af,
    0xca1c4f3bd1280cee,
];

pub const MEMMAP_USABLE: u64 = 0;
pub const MEMMAP_RESERVED: u64 = 1;
pub const MEMMAP_ACPI_RECLAIMABLE: u64 = 2;
pub const MEMMAP_ACPI_NVS: u64 = 3;
pub const MEMMAP_BAD_MEMORY: u64 = 4;
pub const MEMMAP_BOOTLOADER_RECLAIMABLE: u64 = 5;
pub const MEMMAP_EXECUTABLE_AND_MODULES: u64 = 6;
pub const MEMMAP_FRAMEBUFFER: u64 = 7;
pub const MEMMAP_RESERVED_MAPPED: u64 = 8;

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static REQUESTS_START: [u64; 4] = REQUESTS_START_MARKER;

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new(0);

#[used]
#[unsafe(link_section = ".requests")]
static MEMMAP_REQUEST: Request<MemmapResponse> = Request::new(MEMMAP_REQUEST_ID);

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: Request<HhdmResponse> = Request::new(HHDM_REQUEST_ID);

#[used]
#[unsafe(link_section = ".requests")]
static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static REQUESTS_END: [u64; 2] = REQUESTS_END_MARKER;

#[repr(transparent)]
struct BaseRevision(UnsafeCell<[u64; 3]>);

unsafe impl Sync for BaseRevision {}

impl BaseRevision {
    const fn new(revision: u64) -> Self {
        Self(UnsafeCell::new([
            0xf9562b2d5c95a6c8,
            0x6a7b384944536bdc,
            revision,
        ]))
    }

    fn is_supported(&self) -> bool {
        unsafe { ptr::addr_of!((*self.0.get())[2]).read_volatile() == 0 }
    }
}

#[repr(C)]
struct Request<T> {
    id: [u64; 4],
    revision: u64,
    response: ResponsePtr<T>,
}

impl<T> Request<T> {
    const fn new(id: [u64; 4]) -> Self {
        Self {
            id,
            revision: 0,
            response: ResponsePtr::null(),
        }
    }

    fn response(&self) -> Option<&'static T> {
        self.response.get()
    }
}

#[repr(transparent)]
struct ResponsePtr<T>(UnsafeCell<*const T>);

unsafe impl<T> Sync for ResponsePtr<T> {}

impl<T> ResponsePtr<T> {
    const fn null() -> Self {
        Self(UnsafeCell::new(ptr::null()))
    }

    fn get(&self) -> Option<&'static T> {
        let response = unsafe { self.0.get().read_volatile() };
        unsafe { response.as_ref() }
    }
}

#[repr(C)]
pub struct Uuid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

#[repr(C)]
pub struct File {
    pub revision: u64,
    pub address: *const u8,
    pub size: u64,
    pub path: *const u8,
    pub string: *const u8,
    pub media_type: u32,
    pub unused: u32,
    pub tftp_ip: u32,
    pub tftp_port: u32,
    pub partition_index: u32,
    pub mbr_disk_id: u32,
    pub gpt_disk_uuid: Uuid,
    pub gpt_part_uuid: Uuid,
    pub part_uuid: Uuid,
}

#[repr(C)]
pub struct MemmapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u64,
}

#[repr(C)]
struct MemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *const *const MemmapEntry,
}

#[repr(C)]
struct HhdmResponse {
    revision: u64,
    offset: u64,
}

#[repr(C)]
struct ModuleResponse {
    revision: u64,
    module_count: u64,
    modules: *const *const File,
}

#[repr(C)]
struct ModuleRequest {
    id: [u64; 4],
    revision: u64,
    response: ResponsePtr<ModuleResponse>,
    internal_module_count: u64,
    internal_modules: *const *const u8,
}

unsafe impl Sync for ModuleRequest {}

impl ModuleRequest {
    const fn new() -> Self {
        Self {
            id: MODULE_REQUEST_ID,
            revision: 0,
            response: ResponsePtr::null(),
            internal_module_count: 0,
            internal_modules: ptr::null(),
        }
    }

    fn response(&self) -> Option<&'static ModuleResponse> {
        self.response.get()
    }
}

pub struct MemoryMap {
    response: &'static MemmapResponse,
}

impl MemoryMap {
    pub fn entry_count(&self) -> u64 {
        self.response.entry_count
    }

    pub fn entry(&self, index: u64) -> Option<&'static MemmapEntry> {
        if index >= self.response.entry_count {
            return None;
        }

        let entry_ptr = unsafe { self.response.entries.add(index as usize).read() };
        unsafe { entry_ptr.as_ref() }
    }
}

pub struct Modules {
    response: &'static ModuleResponse,
}

impl Modules {
    pub fn module_count(&self) -> u64 {
        self.response.module_count
    }

    pub fn module(&self, index: u64) -> Option<&'static File> {
        if index >= self.response.module_count {
            return None;
        }

        let module_ptr = unsafe { self.response.modules.add(index as usize).read() };
        unsafe { module_ptr.as_ref() }
    }
}

pub fn base_revision_supported() -> bool {
    BASE_REVISION.is_supported()
}

pub fn memory_map() -> Option<MemoryMap> {
    MEMMAP_REQUEST
        .response()
        .map(|response| MemoryMap { response })
}

pub fn hhdm_offset() -> Option<u64> {
    HHDM_REQUEST.response().map(|response| response.offset)
}

pub fn modules() -> Option<Modules> {
    MODULE_REQUEST
        .response()
        .map(|response| Modules { response })
}

pub fn memmap_type_name(entry_type: u64) -> &'static str {
    match entry_type {
        MEMMAP_USABLE => "usable",
        MEMMAP_RESERVED => "reserved",
        MEMMAP_ACPI_RECLAIMABLE => "acpi-reclaimable",
        MEMMAP_ACPI_NVS => "acpi-nvs",
        MEMMAP_BAD_MEMORY => "bad-memory",
        MEMMAP_BOOTLOADER_RECLAIMABLE => "bootloader-reclaimable",
        MEMMAP_EXECUTABLE_AND_MODULES => "executable-and-modules",
        MEMMAP_FRAMEBUFFER => "framebuffer",
        MEMMAP_RESERVED_MAPPED => "reserved-mapped",
        _ => "unknown",
    }
}
