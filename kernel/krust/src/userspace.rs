use core::ptr;

use crate::{
    elf::{self, Elf, ProgramHeader},
    gdt, ipc, memory,
    memory::{FRAME_SIZE, FrameAllocator},
    paging::{self, AddressSpace, PageFlags},
    serial, syscall,
};

const USER_CANONICAL_LIMIT: u64 = 0x0000_8000_0000_0000;
const USER_STACK_TOP: u64 = 0x0000_7000_0000_0000;
const USER_STACK_PAGES: usize = 4;

#[derive(Clone, Copy)]
pub struct UserImage {
    pub cr3: u64,
    pub entry: u64,
    pub stack_top: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    Elf(elf::ElfError),
    AddressSpace(paging::MapError),
    Map(paging::MapError),
    OutOfFrames,
    BadSegment,
}

pub fn load(
    bytes: &[u8],
    hhdm_offset: u64,
    allocator: &mut FrameAllocator,
) -> Result<UserImage, LoadError> {
    let elf = Elf::parse(bytes).map_err(LoadError::Elf)?;
    let mut address_space = AddressSpace::new_from_active_kernel_mappings(hhdm_offset, allocator)
        .map_err(LoadError::AddressSpace)?;

    let mut index = 0;
    while index < elf.program_header_count() {
        if let Some(header) = elf.program_header(index)
            && header.typ == elf::PT_LOAD
        {
            load_segment(bytes, hhdm_offset, allocator, &mut address_space, header)?;
        }

        index += 1;
    }

    map_user_stack(hhdm_offset, allocator, &mut address_space)?;

    Ok(UserImage {
        cr3: address_space.root_table_physical(),
        entry: elf.entry(),
        stack_top: USER_STACK_TOP,
    })
}

#[allow(dead_code)]
pub fn enter(image: UserImage) -> ! {
    print_image("Krust userspace ELF loaded", image);

    gdt::init();
    serial::write_str("GDT initialized\n");
    syscall::init();
    serial::write_str("Syscall path initialized\n");

    serial::write_str("Entering Krust userspace\n");
    unsafe {
        gdt::enter_user_mode(image.cr3, image.entry, image.stack_top);
    }
}

pub fn enter_ipc_demo(initial: ipc::ProcessContext) -> ! {
    gdt::init();
    serial::write_str("GDT initialized\n");
    syscall::init();
    serial::write_str("Syscall path initialized\n");

    serial::write_str("Entering IPC sender userspace\n");
    unsafe {
        gdt::enter_user_mode(initial.cr3, initial.entry, initial.stack_top);
    }
}

fn print_image(label: &str, image: UserImage) {
    serial::write_str(label);
    serial::write_str(": entry=");
    serial::write_u64_hex(image.entry);
    serial::write_str(" stack=");
    serial::write_u64_hex(image.stack_top);
    serial::write_str(" cr3=");
    serial::write_u64_hex(image.cr3);
    serial::write_str("\n");
}

pub fn print_load_error(error: LoadError) {
    serial::write_str("Userspace load failed: ");
    match error {
        LoadError::Elf(error) => print_elf_error(error),
        LoadError::AddressSpace(error) => {
            serial::write_str("address space ");
            print_map_error(error);
        }
        LoadError::Map(error) => {
            serial::write_str("map ");
            print_map_error(error);
        }
        LoadError::OutOfFrames => serial::write_str("out of frames"),
        LoadError::BadSegment => serial::write_str("bad segment"),
    }
    serial::write_str("\n");
}

fn load_segment(
    bytes: &[u8],
    hhdm_offset: u64,
    allocator: &mut FrameAllocator,
    address_space: &mut AddressSpace,
    header: ProgramHeader,
) -> Result<(), LoadError> {
    if header.memsz < header.filesz || header.vaddr >= USER_CANONICAL_LIMIT {
        return Err(LoadError::BadSegment);
    }

    let file_end = header
        .offset
        .checked_add(header.filesz)
        .ok_or(LoadError::BadSegment)?;
    if file_end > bytes.len() as u64 {
        return Err(LoadError::BadSegment);
    }

    let page_start = align_down(header.vaddr, FRAME_SIZE);
    let segment_end = header
        .vaddr
        .checked_add(header.memsz)
        .ok_or(LoadError::BadSegment)?;
    let page_end = align_up(segment_end, FRAME_SIZE).ok_or(LoadError::BadSegment)?;
    if page_end > USER_CANONICAL_LIMIT {
        return Err(LoadError::BadSegment);
    }

    let flags = PageFlags::user(header.flags & elf::PF_W != 0, header.flags & elf::PF_X != 0);
    let mut page = page_start;
    while page < page_end {
        let frame = allocator.allocate().ok_or(LoadError::OutOfFrames)?;
        zero_frame(hhdm_offset, frame);

        let copy_start = max(page, header.vaddr);
        let copy_end = min(page + FRAME_SIZE, header.vaddr + header.filesz);
        if copy_start < copy_end {
            copy_segment_bytes(
                bytes,
                hhdm_offset,
                frame,
                header,
                page,
                copy_start,
                copy_end,
            )?;
        }

        address_space
            .map_page(page, frame, flags, allocator)
            .map_err(LoadError::Map)?;
        page += FRAME_SIZE;
    }

    Ok(())
}

fn map_user_stack(
    hhdm_offset: u64,
    allocator: &mut FrameAllocator,
    address_space: &mut AddressSpace,
) -> Result<(), LoadError> {
    let stack_bottom = USER_STACK_TOP - (USER_STACK_PAGES as u64 * FRAME_SIZE);
    let mut page = stack_bottom;

    while page < USER_STACK_TOP {
        let frame = allocator.allocate().ok_or(LoadError::OutOfFrames)?;
        zero_frame(hhdm_offset, frame);
        address_space
            .map_page(page, frame, PageFlags::user(true, false), allocator)
            .map_err(LoadError::Map)?;
        page += FRAME_SIZE;
    }

    Ok(())
}

fn copy_segment_bytes(
    bytes: &[u8],
    hhdm_offset: u64,
    frame: memory::PhysicalFrame,
    header: ProgramHeader,
    page: u64,
    copy_start: u64,
    copy_end: u64,
) -> Result<(), LoadError> {
    let copy_len = usize::try_from(copy_end - copy_start).map_err(|_| LoadError::BadSegment)?;
    let source_offset = header
        .offset
        .checked_add(copy_start - header.vaddr)
        .ok_or(LoadError::BadSegment)?;
    let source_offset = usize::try_from(source_offset).map_err(|_| LoadError::BadSegment)?;
    let destination_offset = copy_start - page;
    let destination = (hhdm_offset + frame.start() + destination_offset) as *mut u8;

    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr().add(source_offset), destination, copy_len);
    }

    Ok(())
}

fn zero_frame(hhdm_offset: u64, frame: memory::PhysicalFrame) {
    unsafe {
        ptr::write_bytes(
            (hhdm_offset + frame.start()) as *mut u8,
            0,
            FRAME_SIZE as usize,
        );
    }
}

fn print_elf_error(error: elf::ElfError) {
    match error {
        elf::ElfError::TooSmall => serial::write_str("elf too small"),
        elf::ElfError::BadMagic => serial::write_str("bad elf magic"),
        elf::ElfError::UnsupportedClass => serial::write_str("unsupported elf class"),
        elf::ElfError::UnsupportedEndian => serial::write_str("unsupported elf endian"),
        elf::ElfError::UnsupportedVersion => serial::write_str("unsupported elf version"),
        elf::ElfError::UnsupportedType => serial::write_str("unsupported elf type"),
        elf::ElfError::UnsupportedMachine => serial::write_str("unsupported elf machine"),
        elf::ElfError::BadProgramHeaders => serial::write_str("bad elf program headers"),
    }
}

fn print_map_error(error: paging::MapError) {
    match error {
        paging::MapError::OutOfFrames => serial::write_str("out of frames"),
        paging::MapError::AlreadyMapped => serial::write_str("already mapped"),
        paging::MapError::HugePageEncountered => serial::write_str("huge page encountered"),
    }
}

fn align_up(value: u64, align: u64) -> Option<u64> {
    Some(value.checked_add(align - 1)? & !(align - 1))
}

fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn min(left: u64, right: u64) -> u64 {
    if left < right { left } else { right }
}

fn max(left: u64, right: u64) -> u64 {
    if left > right { left } else { right }
}
