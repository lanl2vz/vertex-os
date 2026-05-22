use crate::memory::{FRAME_SIZE, FrameAllocator, PhysicalFrame};

const ENTRY_COUNT: usize = 512;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER_ACCESSIBLE: u64 = 1 << 2;
const HUGE_PAGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

pub const KERNEL_HEAP_BASE: u64 = 0xffffff40_00000000;
pub const KERNEL_HEAP_PAGES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfFrames,
    AlreadyMapped,
    HugePageEncountered,
}

pub struct Mapper {
    hhdm_offset: u64,
    root_table: *mut PageTable,
}

pub struct AddressSpace {
    hhdm_offset: u64,
    root_frame: PhysicalFrame,
    root_table: *mut PageTable,
}

#[derive(Clone, Copy)]
pub struct PageFlags {
    bits: u64,
}

#[repr(C, align(4096))]
struct PageTable {
    entries: [PageTableEntry; ENTRY_COUNT],
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct PageTableEntry(u64);

impl PageTableEntry {
    const fn empty() -> Self {
        Self(0)
    }

    fn is_present(self) -> bool {
        self.0 & PRESENT != 0
    }

    fn is_huge(self) -> bool {
        self.0 & HUGE_PAGE != 0
    }

    fn address(self) -> u64 {
        self.0 & ADDRESS_MASK
    }

    fn set(&mut self, frame: PhysicalFrame, flags: u64) {
        self.0 = frame.start() | flags;
    }

    fn add_flags(&mut self, flags: u64) {
        self.0 |= flags;
    }
}

impl PageFlags {
    pub const fn kernel_writable_no_execute() -> Self {
        Self {
            bits: PRESENT | WRITABLE | NO_EXECUTE,
        }
    }

    pub const fn user(writable: bool, executable: bool) -> Self {
        let mut bits = PRESENT | USER_ACCESSIBLE;

        if writable {
            bits |= WRITABLE;
        }
        if !executable {
            bits |= NO_EXECUTE;
        }

        Self { bits }
    }

    fn user_accessible(self) -> bool {
        self.bits & USER_ACCESSIBLE != 0
    }

    fn table_bits(self) -> u64 {
        let mut bits = PRESENT | WRITABLE;

        if self.user_accessible() {
            bits |= USER_ACCESSIBLE;
        }

        bits
    }
}

impl Mapper {
    pub unsafe fn active(hhdm_offset: u64) -> Self {
        let root_phys = read_cr3() & ADDRESS_MASK;

        Self {
            hhdm_offset,
            root_table: phys_to_virt(hhdm_offset, root_phys),
        }
    }

    pub fn root_table_physical(&self) -> u64 {
        read_cr3() & ADDRESS_MASK
    }

    pub fn map_page(
        &mut self,
        virtual_address: u64,
        frame: PhysicalFrame,
        allocator: &mut FrameAllocator,
    ) -> Result<(), MapError> {
        map_page_in_table(
            self.hhdm_offset,
            self.root_table,
            virtual_address,
            frame,
            PageFlags::kernel_writable_no_execute(),
            allocator,
        )
    }
}

impl AddressSpace {
    pub fn new_from_active_kernel_mappings(
        hhdm_offset: u64,
        allocator: &mut FrameAllocator,
    ) -> Result<Self, MapError> {
        let root_frame = allocator.allocate().ok_or(MapError::OutOfFrames)?;
        let root_table = phys_to_virt(hhdm_offset, root_frame.start());
        let active_root = phys_to_virt(hhdm_offset, read_cr3() & ADDRESS_MASK);

        unsafe {
            zero_table(root_table);

            let mut index = ENTRY_COUNT / 2;
            while index < ENTRY_COUNT {
                (*root_table).entries[index] = (*active_root).entries[index];
                index += 1;
            }
        }

        Ok(Self {
            hhdm_offset,
            root_frame,
            root_table,
        })
    }

    pub fn root_table_physical(&self) -> u64 {
        self.root_frame.start()
    }

    pub fn map_page(
        &mut self,
        virtual_address: u64,
        frame: PhysicalFrame,
        flags: PageFlags,
        allocator: &mut FrameAllocator,
    ) -> Result<(), MapError> {
        map_page_in_table(
            self.hhdm_offset,
            self.root_table,
            virtual_address,
            frame,
            flags,
            allocator,
        )
    }
}

fn map_page_in_table(
    hhdm_offset: u64,
    root_table: *mut PageTable,
    virtual_address: u64,
    frame: PhysicalFrame,
    flags: PageFlags,
    allocator: &mut FrameAllocator,
) -> Result<(), MapError> {
    let indexes = page_indexes(virtual_address);
    let table = ensure_next_table(hhdm_offset, root_table, indexes[0], flags, allocator)?;
    let table = ensure_next_table(hhdm_offset, table, indexes[1], flags, allocator)?;
    let table = ensure_next_table(hhdm_offset, table, indexes[2], flags, allocator)?;

    let entry = unsafe { &mut (*table).entries[indexes[3]] };
    if entry.is_present() {
        return Err(MapError::AlreadyMapped);
    }

    entry.set(frame, flags.bits);
    unsafe {
        invlpg(virtual_address);
    }
    Ok(())
}

fn ensure_next_table(
    hhdm_offset: u64,
    table: *mut PageTable,
    index: usize,
    flags: PageFlags,
    allocator: &mut FrameAllocator,
) -> Result<*mut PageTable, MapError> {
    let entry = unsafe { &mut (*table).entries[index] };

    if entry.is_present() {
        if entry.is_huge() {
            return Err(MapError::HugePageEncountered);
        }

        if flags.user_accessible() {
            entry.add_flags(USER_ACCESSIBLE);
        }

        return Ok(phys_to_virt(hhdm_offset, entry.address()));
    }

    let frame = allocator.allocate().ok_or(MapError::OutOfFrames)?;
    let next_table = phys_to_virt(hhdm_offset, frame.start());
    unsafe {
        zero_table(next_table);
    }
    entry.set(frame, flags.table_bits());
    Ok(next_table)
}

pub fn kernel_heap_page(index: usize) -> Option<u64> {
    if index >= KERNEL_HEAP_PAGES {
        return None;
    }

    Some(KERNEL_HEAP_BASE + (index as u64 * FRAME_SIZE))
}

fn page_indexes(virtual_address: u64) -> [usize; 4] {
    [
        ((virtual_address >> 39) & 0x1ff) as usize,
        ((virtual_address >> 30) & 0x1ff) as usize,
        ((virtual_address >> 21) & 0x1ff) as usize,
        ((virtual_address >> 12) & 0x1ff) as usize,
    ]
}

fn phys_to_virt(hhdm_offset: u64, physical_address: u64) -> *mut PageTable {
    (hhdm_offset + physical_address) as *mut PageTable
}

unsafe fn zero_table(table: *mut PageTable) {
    let mut index = 0;
    while index < ENTRY_COUNT {
        unsafe {
            (*table).entries[index] = PageTableEntry::empty();
        }
        index += 1;
    }
}

fn read_cr3() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

unsafe fn invlpg(virtual_address: u64) {
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) virtual_address, options(nostack, preserves_flags));
    }
}
