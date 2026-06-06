use crate::{
    kernel::{InitError, IpcError},
    serial,
};

use super::VfsName;

pub(crate) const MAX_VERTEXFS_FILES: usize = 16;
pub(crate) const MAX_VERTEXFS_FILE_BYTES: usize = 512;
pub(crate) const VERTEXFS_MODULE_STRING: &[u8] = b"vertexfs-v1";
pub(crate) const VERTEXFS_SECTOR_SIZE: usize = 512;
pub(crate) const VERTEXFS_SECTORS: usize = 64;
pub(crate) const VERTEXFS_IMAGE_BYTES: usize = VERTEXFS_SECTOR_SIZE * VERTEXFS_SECTORS;
const VERTEXFS_SUPERBLOCK_MAGIC: &[u8; 16] = b"VERTEXFSV1\0\0\0\0\0\0";
const VERTEXFS_INODE_TABLE_MAGIC: &[u8; 16] = b"VFSINODEV1\0\0\0\0\0\0";
const VERTEXFS_DIRECTORY_MAGIC: &[u8; 16] = b"VFSDIRV1\0\0\0\0\0\0\0\0";
const VERTEXFS_FREE_MAP_MAGIC: &[u8; 16] = b"VFSFREEV1\0\0\0\0\0\0\0";
const VERTEXFS_JOURNAL_MAGIC: &[u8; 16] = b"VFSJOURNALV1\0\0\0\0";
const VERTEXFS_VERSION: u16 = 1;
const VERTEXFS_CHECKSUM_OFFSET: usize = 20;
const VERTEXFS_FEATURE_METADATA_V1: u32 = 1;
const VERTEXFS_FEATURE_DIRECTORY_CHECKSUMS: u32 = 1 << 1;
const VERTEXFS_FEATURE_FREE_SPACE_CHECKSUMS: u32 = 1 << 2;
const VERTEXFS_FEATURE_JOURNAL_V1: u32 = 1 << 3;
const VERTEXFS_FEATURE_FLAGS: u32 = VERTEXFS_FEATURE_METADATA_V1
    | VERTEXFS_FEATURE_DIRECTORY_CHECKSUMS
    | VERTEXFS_FEATURE_FREE_SPACE_CHECKSUMS
    | VERTEXFS_FEATURE_JOURNAL_V1;
const VERTEXFS_GENERATION_OFFSET: usize = 32;
const VERTEXFS_SECTION_TABLE_OFFSET: usize = 128;
const VERTEXFS_SECTION_RECORD_LEN: usize = 16;
pub(crate) const VERTEXFS_INODE_TABLE_SECTOR: u64 = 1;
pub(crate) const VERTEXFS_INODE_TABLE_SECTORS: u64 = 2;
pub(crate) const VERTEXFS_DIRECTORY_SECTOR: u64 =
    VERTEXFS_INODE_TABLE_SECTOR + VERTEXFS_INODE_TABLE_SECTORS;
pub(crate) const VERTEXFS_DIRECTORY_SECTORS: u64 = 2;
pub(crate) const VERTEXFS_FREE_MAP_SECTOR: u64 =
    VERTEXFS_DIRECTORY_SECTOR + VERTEXFS_DIRECTORY_SECTORS;
pub(crate) const VERTEXFS_JOURNAL_SECTOR: u64 = VERTEXFS_FREE_MAP_SECTOR + 1;
const VERTEXFS_DATA_SECTOR: u64 = VERTEXFS_JOURNAL_SECTOR + 1;
const VERTEXFS_DATA_SECTORS: u64 = (VERTEXFS_SECTORS as u64) - VERTEXFS_DATA_SECTOR;
const VERTEXFS_INODE_ENTRY_OFFSET: usize = 32;
const VERTEXFS_INODE_ENTRY_LEN: usize = 64;
const VERTEXFS_INODE_TABLE_BYTES: usize =
    VERTEXFS_SECTOR_SIZE * VERTEXFS_INODE_TABLE_SECTORS as usize;
const VERTEXFS_DIRECTORY_ENTRY_OFFSET: usize = 32;
const VERTEXFS_DIRECTORY_ENTRY_LEN: usize = 64;
const VERTEXFS_DIRECTORY_NAME_BYTES: usize = VERTEXFS_DIRECTORY_ENTRY_LEN - 12;
const VERTEXFS_DIRECTORY_BYTES: usize = VERTEXFS_SECTOR_SIZE * VERTEXFS_DIRECTORY_SECTORS as usize;
const VERTEXFS_INODE_ROOT: u32 = 1;
const VERTEXFS_INODE_README: u32 = 2;
pub(crate) const VERTEXFS_INODE_APP_DIR: u32 = 3;
const VERTEXFS_INODE_APP_A: u32 = 4;
const VERTEXFS_BASE_INODE_COUNT: usize = 4;
const VERTEXFS_BASE_DIRECTORY_COUNT: usize = 3;
const VERTEXFS_INODE_ENTRY_CAPACITY: usize =
    (VERTEXFS_INODE_TABLE_BYTES - VERTEXFS_INODE_ENTRY_OFFSET) / VERTEXFS_INODE_ENTRY_LEN;
const VERTEXFS_DIRECTORY_ENTRY_CAPACITY: usize =
    (VERTEXFS_DIRECTORY_BYTES - VERTEXFS_DIRECTORY_ENTRY_OFFSET) / VERTEXFS_DIRECTORY_ENTRY_LEN;
const VERTEXFS_DYNAMIC_INODE_FIRST: u32 = 5;
const VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST: u64 = VERTEXFS_DATA_SECTOR + 2;
pub(crate) const VERTEXFS_DYNAMIC_FILE_CAPACITY: usize =
    VERTEXFS_INODE_ENTRY_CAPACITY - VERTEXFS_BASE_INODE_COUNT;
const VERTEXFS_DIRECTORY_DYNAMIC_FILE_CAPACITY: usize =
    VERTEXFS_DIRECTORY_ENTRY_CAPACITY - VERTEXFS_BASE_DIRECTORY_COUNT;
const VERTEXFS_KIND_DIR: u16 = 1;
const VERTEXFS_KIND_FILE: u16 = 2;
const VERTEXFS_JOURNAL_STATE_CLEAN: u16 = 0;
const VERTEXFS_JOURNAL_STATE_PENDING: u16 = 1;
pub(crate) const VERTEXFS_JOURNAL_PAYLOAD_OFFSET: usize = 64;
pub(crate) const VERTEXFS_SYNC_MAX_DEVICE_WRITES: usize = 8;
pub(crate) const VERTEXDISK_VERTEXFS_IMAGE_SECTOR: u64 = 49_209;

#[derive(Clone, Copy)]
pub(crate) struct VertexFsBootFiles<'a> {
    pub(crate) generation: &'a [u8],
    pub(crate) readme: VertexFsBootFile<'a>,
    pub(crate) app_a: VertexFsBootFile<'a>,
    pub(crate) journal_replayed: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct VertexFsBootFile<'a> {
    pub(crate) inode: VertexFsInode,
    pub(crate) payload: &'a [u8],
}

#[derive(Clone, Copy)]
struct VertexFsParsedInodes {
    readme: VertexFsInode,
    app_a: VertexFsInode,
    dynamic: [Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
}

#[derive(Clone, Copy)]
pub(crate) struct VertexFsInode {
    pub(crate) id: u32,
    pub(crate) kind: u16,
    pub(crate) size: u64,
    pub(crate) first_sector: u64,
    pub(crate) sector_count: u32,
    pub(crate) checksum: u32,
    pub(crate) parent: u32,
}

#[derive(Clone, Copy)]
struct VertexFsJournalRecord<'a> {
    target_inode: u32,
    payload: &'a [u8],
}

#[derive(Clone, Copy)]
pub(crate) struct VfsVertexFsFile {
    pub(crate) name: &'static str,
    pub(crate) vfs_name: VfsName,
    pub(crate) inode_id: u32,
    pub(crate) parent_inode_id: u32,
    pub(crate) first_sector: u64,
    pub(crate) sector_count: u32,
    pub(crate) bytes: [u8; MAX_VERTEXFS_FILE_BYTES],
    pub(crate) len: usize,
    pub(crate) dirty: bool,
    pub(crate) checksum: u32,
}

#[derive(Clone, Copy)]
pub(crate) enum VertexFsSyncResult {
    Journaled {
        inode_id: u32,
        checksum: u32,
        write_count: usize,
    },
    Cached {
        checksum: u32,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct VertexFsDeviceWrite {
    pub(crate) sector: u64,
    pub(crate) bytes: [u8; VERTEXFS_SECTOR_SIZE],
}

impl VertexFsDeviceWrite {
    pub(crate) const fn empty() -> Self {
        Self {
            sector: 0,
            bytes: [0; VERTEXFS_SECTOR_SIZE],
        }
    }
}

impl VfsVertexFsFile {
    pub(crate) const fn empty() -> Self {
        Self {
            name: "",
            vfs_name: VfsName::empty(),
            inode_id: 0,
            parent_inode_id: 0,
            first_sector: 0,
            sector_count: 0,
            bytes: [0; MAX_VERTEXFS_FILE_BYTES],
            len: 0,
            dirty: false,
            checksum: 0,
        }
    }

    pub(crate) fn new(
        name: &'static str,
        initial: &[u8],
        inode: Option<VertexFsInode>,
    ) -> Result<Self, InitError> {
        if initial.len() > MAX_VERTEXFS_FILE_BYTES {
            return Err(InitError::InvalidBootManifest);
        }
        let mut file = Self::empty();
        file.name = name;
        file.vfs_name = VfsName::from_static(name)?;
        if let Some(inode) = inode {
            file.inode_id = inode.id;
            file.parent_inode_id = inode.parent;
            file.first_sector = inode.first_sector;
            file.sector_count = inode.sector_count;
        }
        let mut index = 0;
        while index < initial.len() {
            file.bytes[index] = initial[index];
            index += 1;
        }
        file.len = initial.len();
        file.checksum = vertexfs_checksum32(initial);
        Ok(file)
    }
}

pub(crate) fn parse_vertexfs_image(image: &[u8]) -> Result<VertexFsBootFiles<'_>, InitError> {
    let superblock = vertexfs_sector(image, 0)?;
    if !vertexfs_magic_matches(superblock, VERTEXFS_SUPERBLOCK_MAGIC) {
        return reject_vertexfs_image("bad superblock");
    }
    if read_u16_le(superblock, 16) != VERTEXFS_VERSION {
        return reject_vertexfs_image("unsupported superblock version");
    }
    if read_u16_le(superblock, 18) as usize != VERTEXFS_SECTOR_SIZE {
        return reject_vertexfs_image("unsupported sector size");
    }
    if !vertexfs_checksum_valid(superblock) {
        return reject_vertexfs_image("superblock checksum mismatch");
    }
    if read_u32_le(superblock, 24) as usize != VERTEXFS_SECTORS {
        return reject_vertexfs_image("unsupported sector count");
    }
    if read_u32_le(superblock, 28) != VERTEXFS_FEATURE_FLAGS {
        return reject_vertexfs_image("unsupported feature flags");
    }
    if !vertexfs_section_matches(
        superblock,
        0,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    ) || !vertexfs_section_matches(
        superblock,
        1,
        VERTEXFS_DIRECTORY_SECTOR,
        VERTEXFS_DIRECTORY_SECTORS,
    ) || !vertexfs_section_matches(superblock, 2, VERTEXFS_FREE_MAP_SECTOR, 1)
        || !vertexfs_section_matches(superblock, 3, VERTEXFS_JOURNAL_SECTOR, 1)
        || !vertexfs_section_matches(superblock, 4, VERTEXFS_DATA_SECTOR, VERTEXFS_DATA_SECTORS)
    {
        return reject_vertexfs_image("unsupported section layout");
    }
    let generation = vertexfs_fixed_string(superblock, VERTEXFS_GENERATION_OFFSET, 64)?;

    let journal = vertexfs_parse_journal(image)?;
    let inodes = vertexfs_parse_inode_table(image)?;
    vertexfs_validate_directory(image, &inodes.dynamic)?;
    let (readme, readme_first, readme_end) = vertexfs_file_payload(image, inodes.readme)?;
    let (base_app_a, app_a_first, app_a_end) =
        vertexfs_recoverable_file_payload(image, inodes.app_a, journal)?;
    if readme_first < app_a_end && app_a_first < readme_end {
        return reject_vertexfs_image("overlapping file extents");
    }
    let mut dynamic_extents = [(0u64, 0u64); VERTEXFS_DYNAMIC_FILE_CAPACITY];
    let mut dynamic_extent_count = 0;
    let mut dynamic_index = 0;
    while dynamic_index < VERTEXFS_DYNAMIC_FILE_CAPACITY {
        let Some(dynamic) = inodes.dynamic[dynamic_index] else {
            dynamic_index += 1;
            continue;
        };
        let (_, dynamic_first, dynamic_end) = vertexfs_file_payload(image, dynamic)?;
        if (dynamic_first < readme_end && readme_first < dynamic_end)
            || (dynamic_first < app_a_end && app_a_first < dynamic_end)
        {
            return reject_vertexfs_image("overlapping file extents");
        }
        let mut prior = 0;
        while prior < dynamic_extent_count {
            let (prior_first, prior_end) = dynamic_extents[prior];
            if dynamic_first < prior_end && prior_first < dynamic_end {
                return reject_vertexfs_image("overlapping file extents");
            }
            prior += 1;
        }
        dynamic_extents[dynamic_extent_count] = (dynamic_first, dynamic_end);
        dynamic_extent_count += 1;
        dynamic_index += 1;
    }
    vertexfs_validate_free_map(image, inodes.readme, inodes.app_a, &inodes.dynamic)?;
    let (app_a, journal_replayed) = vertexfs_replay_journal(inodes.app_a, base_app_a, journal)?;

    Ok(VertexFsBootFiles {
        generation,
        readme: VertexFsBootFile {
            inode: inodes.readme,
            payload: readme,
        },
        app_a: VertexFsBootFile {
            inode: inodes.app_a,
            payload: app_a,
        },
        journal_replayed,
    })
}

pub(crate) fn vertexfs_checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        checksum = checksum.wrapping_add((bytes[index] as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum
}

pub(crate) fn vertexfs_dynamic_inode_at(index: usize) -> Result<u32, IpcError> {
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(VERTEXFS_DYNAMIC_INODE_FIRST + index as u32)
}

pub(crate) fn vertexfs_dynamic_data_sector_at(index: usize) -> Result<u64, IpcError> {
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST + index as u64)
}

pub(crate) fn vertexfs_image_has_inode(image: &[u8], inode_id: u32) -> Result<bool, IpcError> {
    let sector = vertexfs_image_section(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    let count = read_u16_le(sector, 18) as usize;
    let mut index = 0;
    while index < count {
        let offset = vertexfs_inode_offset(index)?;
        if read_u32_le(sector, offset) == inode_id {
            return Ok(true);
        }
        index += 1;
    }
    Ok(false)
}

pub(crate) fn vertexfs_image_sector(image: &[u8], sector: u64) -> Result<&[u8], IpcError> {
    let sector_index = usize::try_from(sector).map_err(|_| IpcError::VfsNoSpace)?;
    let start = sector_index
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let end = start
        .checked_add(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    image.get(start..end).ok_or(IpcError::VfsNoSpace)
}

pub(crate) fn write_vertexfs_journal_pending(
    image: &mut [u8],
    inode_id: u32,
    payload: &[u8],
) -> Result<(), IpcError> {
    let sector = vertexfs_image_sector_mut(image, VERTEXFS_JOURNAL_SECTOR)?;
    sector.fill(0);
    copy_bytes(
        &mut sector[..VERTEXFS_JOURNAL_MAGIC.len()],
        VERTEXFS_JOURNAL_MAGIC,
    );
    write_u16_le(sector, 16, VERTEXFS_VERSION);
    write_u16_le(sector, 18, VERTEXFS_JOURNAL_STATE_PENDING);
    write_u32_le(sector, 24, inode_id);
    write_u32_le(sector, 28, payload.len() as u32);
    write_u32_le(sector, 32, vertexfs_checksum32(payload));
    copy_bytes(
        &mut sector
            [VERTEXFS_JOURNAL_PAYLOAD_OFFSET..VERTEXFS_JOURNAL_PAYLOAD_OFFSET + payload.len()],
        payload,
    );
    write_vertexfs_sector_checksum(sector);
    Ok(())
}

pub(crate) fn write_vertexfs_file_extent(
    image: &mut [u8],
    file: VfsVertexFsFile,
) -> Result<(), IpcError> {
    let start = file
        .first_sector
        .checked_mul(VERTEXFS_SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(IpcError::VfsNoSpace)?;
    let extent_len = file
        .sector_count
        .checked_mul(VERTEXFS_SECTOR_SIZE as u32)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(IpcError::VfsNoSpace)?;
    let end = start.checked_add(extent_len).ok_or(IpcError::VfsNoSpace)?;
    let Some(extent) = image.get_mut(start..end) else {
        return Err(IpcError::VfsNoSpace);
    };
    extent.fill(0);
    copy_bytes(&mut extent[..file.len], &file.bytes[..file.len]);
    Ok(())
}

pub(crate) fn write_vertexfs_inode_record(
    image: &mut [u8],
    file: VfsVertexFsFile,
    checksum: u32,
) -> Result<(), IpcError> {
    let sector = vertexfs_image_section_mut(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    let offset = vertexfs_inode_offset_by_id(sector, file.inode_id)?;
    write_u64_le(sector, offset + 8, file.len as u64);
    write_u32_le(sector, offset + 28, checksum);
    write_vertexfs_sector_checksum(sector);
    Ok(())
}

pub(crate) fn write_vertexfs_dynamic_metadata(
    image: &mut [u8],
    file: VfsVertexFsFile,
    checksum: u32,
) -> Result<(), IpcError> {
    let dynamic_index = vertexfs_dynamic_index_for_inode(file.inode_id)?;
    if file.parent_inode_id != VERTEXFS_INODE_APP_DIR
        || file.first_sector != vertexfs_dynamic_data_sector_at(dynamic_index)?
        || file.sector_count != 1
    {
        return Err(IpcError::VfsUnsupported);
    }
    let inode_entry_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
    let directory_entry_index = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
    let expected_inode_count = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
    let expected_directory_count = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
    {
        let sector = vertexfs_image_section_mut(
            image,
            VERTEXFS_INODE_TABLE_SECTOR,
            VERTEXFS_INODE_TABLE_SECTORS,
        )?;
        if read_u16_le(sector, 18) as usize != expected_inode_count {
            return Err(IpcError::VfsUnsupported);
        }
        write_u16_le(sector, 18, (expected_inode_count + 1) as u16);
        let offset = vertexfs_inode_offset(inode_entry_index)?;
        write_u32_le(sector, offset, file.inode_id);
        write_u16_le(sector, offset + 4, VERTEXFS_KIND_FILE);
        write_u16_le(sector, offset + 6, 0);
        write_u64_le(sector, offset + 8, file.len as u64);
        write_u64_le(sector, offset + 16, file.first_sector);
        write_u32_le(sector, offset + 24, file.sector_count);
        write_u32_le(sector, offset + 28, checksum);
        write_u32_le(sector, offset + 32, file.parent_inode_id);
        write_vertexfs_fixed_vfs_name(sector, offset + 36, 28, file.vfs_name)?;
        write_vertexfs_sector_checksum(sector);
    }
    {
        let sector = vertexfs_image_section_mut(
            image,
            VERTEXFS_DIRECTORY_SECTOR,
            VERTEXFS_DIRECTORY_SECTORS,
        )?;
        if read_u16_le(sector, 18) as usize != expected_directory_count {
            return Err(IpcError::VfsUnsupported);
        }
        write_u16_le(sector, 18, (expected_directory_count + 1) as u16);
        let offset = vertexfs_directory_offset(directory_entry_index)?;
        write_u32_le(sector, offset, file.parent_inode_id);
        write_u32_le(sector, offset + 4, file.inode_id);
        write_u16_le(sector, offset + 8, VERTEXFS_KIND_FILE);
        write_u16_le(sector, offset + 10, 0);
        write_vertexfs_fixed_vfs_name(
            sector,
            offset + 12,
            VERTEXFS_DIRECTORY_NAME_BYTES,
            file.vfs_name,
        )?;
        write_vertexfs_sector_checksum(sector);
    }
    {
        let sector = vertexfs_image_sector_mut(image, VERTEXFS_FREE_MAP_SECTOR)?;
        let sector_index =
            usize::try_from(file.first_sector).map_err(|_| IpcError::VfsUnsupported)?;
        let Some(byte) = sector.get_mut(32 + sector_index) else {
            return Err(IpcError::VfsUnsupported);
        };
        if *byte != 0 {
            return Err(IpcError::VfsNoSpace);
        }
        *byte = 1;
        write_vertexfs_sector_checksum(sector);
    }
    Ok(())
}

pub(crate) fn write_vertexfs_journal_clean(image: &mut [u8]) -> Result<(), IpcError> {
    let sector = vertexfs_image_sector_mut(image, VERTEXFS_JOURNAL_SECTOR)?;
    sector.fill(0);
    copy_bytes(
        &mut sector[..VERTEXFS_JOURNAL_MAGIC.len()],
        VERTEXFS_JOURNAL_MAGIC,
    );
    write_u16_le(sector, 16, VERTEXFS_VERSION);
    write_u16_le(sector, 18, VERTEXFS_JOURNAL_STATE_CLEAN);
    write_vertexfs_sector_checksum(sector);
    Ok(())
}

pub(crate) fn vertexfs_device_absolute_sector(vertexfs_sector: u64) -> Result<u64, IpcError> {
    if vertexfs_sector >= VERTEXFS_SECTORS as u64 {
        return Err(IpcError::VfsUnsupported);
    }
    VERTEXDISK_VERTEXFS_IMAGE_SECTOR
        .checked_add(vertexfs_sector)
        .ok_or(IpcError::VfsUnsupported)
}

fn vertexfs_parse_inode_table(image: &[u8]) -> Result<VertexFsParsedInodes, InitError> {
    let sector = vertexfs_section(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    if !vertexfs_magic_matches(sector, VERTEXFS_INODE_TABLE_MAGIC) {
        return reject_vertexfs_image("bad inode table");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("inode table metadata invalid");
    }
    let count = read_u16_le(sector, 18) as usize;
    if count < VERTEXFS_BASE_INODE_COUNT
        || count > VERTEXFS_BASE_INODE_COUNT + VERTEXFS_DYNAMIC_FILE_CAPACITY
    {
        return reject_vertexfs_image("inode table count mismatch");
    }

    let root = vertexfs_read_inode(sector, 0);
    if root.id != VERTEXFS_INODE_ROOT
        || root.kind != VERTEXFS_KIND_DIR
        || root.size != 0
        || root.first_sector != 0
        || root.sector_count != 0
        || root.checksum != 0
        || root.parent != 0
        || !vertexfs_inode_reserved_zero(sector, 0)
        || !vertexfs_inode_name_eq(sector, 0, b"/")
    {
        return reject_vertexfs_image("root inode mismatch");
    }

    let readme = vertexfs_read_inode(sector, 1);
    if readme.id != VERTEXFS_INODE_README
        || readme.kind != VERTEXFS_KIND_FILE
        || readme.parent != VERTEXFS_INODE_ROOT
        || !vertexfs_inode_reserved_zero(sector, 1)
        || !vertexfs_inode_name_eq(sector, 1, b"readme")
    {
        return reject_vertexfs_image("readme inode mismatch");
    }

    let app_dir = vertexfs_read_inode(sector, 2);
    if app_dir.id != VERTEXFS_INODE_APP_DIR
        || app_dir.kind != VERTEXFS_KIND_DIR
        || app_dir.size != 0
        || app_dir.first_sector != 0
        || app_dir.sector_count != 0
        || app_dir.checksum != 0
        || app_dir.parent != VERTEXFS_INODE_ROOT
        || !vertexfs_inode_reserved_zero(sector, 2)
        || !vertexfs_inode_name_eq(sector, 2, b"app")
    {
        return reject_vertexfs_image("app directory inode mismatch");
    }

    let app_a = vertexfs_read_inode(sector, 3);
    if app_a.id != VERTEXFS_INODE_APP_A
        || app_a.kind != VERTEXFS_KIND_FILE
        || app_a.parent != VERTEXFS_INODE_APP_DIR
        || !vertexfs_inode_reserved_zero(sector, 3)
        || !vertexfs_inode_name_eq(sector, 3, b"a")
    {
        return reject_vertexfs_image("app/a inode mismatch");
    }

    let mut dynamic = [None; VERTEXFS_DYNAMIC_FILE_CAPACITY];
    let dynamic_count = count - VERTEXFS_BASE_INODE_COUNT;
    let mut dynamic_index = 0;
    while dynamic_index < dynamic_count {
        let inode_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let inode = vertexfs_read_inode(sector, inode_index);
        if inode.id != VERTEXFS_DYNAMIC_INODE_FIRST + dynamic_index as u32
            || inode.kind != VERTEXFS_KIND_FILE
            || inode.parent != VERTEXFS_INODE_APP_DIR
            || inode.first_sector != VERTEXFS_DYNAMIC_DATA_SECTOR_FIRST + dynamic_index as u64
            || inode.sector_count != 1
            || !vertexfs_inode_reserved_zero(sector, inode_index)
            || vertexfs_fixed_string(sector, vertexfs_inode_name_offset(inode_index), 28).is_err()
        {
            return reject_vertexfs_image("dynamic inode mismatch");
        }
        dynamic[dynamic_index] = Some(inode);
        dynamic_index += 1;
    }

    Ok(VertexFsParsedInodes {
        readme,
        app_a,
        dynamic,
    })
}

fn vertexfs_validate_directory(
    image: &[u8],
    dynamic: &[Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
) -> Result<(), InitError> {
    let sector = vertexfs_section(image, VERTEXFS_DIRECTORY_SECTOR, VERTEXFS_DIRECTORY_SECTORS)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_DIRECTORY_MAGIC) {
        return reject_vertexfs_image("bad directory");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("directory metadata invalid");
    }
    let count = read_u16_le(sector, 18) as usize;
    if count < VERTEXFS_BASE_DIRECTORY_COUNT
        || count > VERTEXFS_BASE_DIRECTORY_COUNT + VERTEXFS_DIRECTORY_DYNAMIC_FILE_CAPACITY
    {
        return reject_vertexfs_image("directory count mismatch");
    }
    let mut dynamic_count = 0;
    while dynamic_count < VERTEXFS_DYNAMIC_FILE_CAPACITY && dynamic[dynamic_count].is_some() {
        dynamic_count += 1;
    }
    if count != VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_count {
        return reject_vertexfs_image("directory count mismatch");
    }
    if !vertexfs_directory_entry_eq(
        sector,
        0,
        VERTEXFS_INODE_ROOT,
        VERTEXFS_INODE_README,
        VERTEXFS_KIND_FILE,
        b"readme",
    ) || !vertexfs_directory_entry_eq(
        sector,
        1,
        VERTEXFS_INODE_ROOT,
        VERTEXFS_INODE_APP_DIR,
        VERTEXFS_KIND_DIR,
        b"app",
    ) || !vertexfs_directory_entry_eq(
        sector,
        2,
        VERTEXFS_INODE_APP_DIR,
        VERTEXFS_INODE_APP_A,
        VERTEXFS_KIND_FILE,
        b"a",
    ) {
        return reject_vertexfs_image("directory entry mismatch");
    }
    let inode_sector = vertexfs_section(
        image,
        VERTEXFS_INODE_TABLE_SECTOR,
        VERTEXFS_INODE_TABLE_SECTORS,
    )?;
    let mut dynamic_index = 0;
    while dynamic_index < dynamic_count {
        let Some(dynamic_inode) = dynamic[dynamic_index] else {
            return reject_vertexfs_image("dynamic directory entry mismatch");
        };
        let inode_index = VERTEXFS_BASE_INODE_COUNT + dynamic_index;
        let directory_index = VERTEXFS_BASE_DIRECTORY_COUNT + dynamic_index;
        let inode_name =
            vertexfs_fixed_string(inode_sector, vertexfs_inode_name_offset(inode_index), 28)?;
        if !vertexfs_directory_entry_eq(
            sector,
            directory_index,
            VERTEXFS_INODE_APP_DIR,
            dynamic_inode.id,
            VERTEXFS_KIND_FILE,
            inode_name,
        ) {
            return reject_vertexfs_image("dynamic directory entry mismatch");
        }
        dynamic_index += 1;
    }
    Ok(())
}

fn vertexfs_replay_journal<'a>(
    app_a_inode: VertexFsInode,
    base_app_a: &'a [u8],
    journal: Option<VertexFsJournalRecord<'a>>,
) -> Result<(&'a [u8], bool), InitError> {
    let Some(record) = journal else {
        return Ok((base_app_a, false));
    };
    if record.target_inode != VERTEXFS_INODE_APP_A || app_a_inode.id != VERTEXFS_INODE_APP_A {
        return reject_vertexfs_image("journal target unsupported");
    }
    if record.payload.len() > MAX_VERTEXFS_FILE_BYTES {
        return reject_vertexfs_image("journal payload too large");
    }
    if record.payload.len() as u64 > app_a_inode.sector_count as u64 * VERTEXFS_SECTOR_SIZE as u64 {
        return reject_vertexfs_image("journal payload exceeds target extent");
    }
    Ok((record.payload, true))
}

fn vertexfs_parse_journal(image: &[u8]) -> Result<Option<VertexFsJournalRecord<'_>>, InitError> {
    let sector = vertexfs_sector(image, VERTEXFS_JOURNAL_SECTOR)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_JOURNAL_MAGIC) {
        return reject_vertexfs_image("bad journal");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("journal metadata invalid");
    }
    let state = read_u16_le(sector, 18);
    if state == VERTEXFS_JOURNAL_STATE_CLEAN {
        return Ok(None);
    }
    if state != VERTEXFS_JOURNAL_STATE_PENDING {
        return reject_vertexfs_image("journal state unsupported");
    }
    let target_inode = read_u32_le(sector, 24);
    let payload_len = read_u32_le(sector, 28) as usize;
    let payload_checksum = read_u32_le(sector, 32);
    let Some(end) = VERTEXFS_JOURNAL_PAYLOAD_OFFSET.checked_add(payload_len) else {
        return reject_vertexfs_image("journal payload overflow");
    };
    let Some(payload) = sector.get(VERTEXFS_JOURNAL_PAYLOAD_OFFSET..end) else {
        return reject_vertexfs_image("journal payload out of bounds");
    };
    if vertexfs_checksum32(payload) != payload_checksum {
        return reject_vertexfs_image("journal payload checksum mismatch");
    }
    Ok(Some(VertexFsJournalRecord {
        target_inode,
        payload,
    }))
}

fn vertexfs_file_payload(
    image: &[u8],
    inode: VertexFsInode,
) -> Result<(&[u8], u64, u64), InitError> {
    let (data, first_sector, end_sector) = vertexfs_file_extent_payload(image, inode)?;
    if vertexfs_checksum32(data) != inode.checksum {
        return reject_vertexfs_image("file checksum mismatch");
    }
    Ok((data, first_sector, end_sector))
}

fn vertexfs_recoverable_file_payload<'a>(
    image: &'a [u8],
    inode: VertexFsInode,
    journal: Option<VertexFsJournalRecord<'a>>,
) -> Result<(&'a [u8], u64, u64), InitError> {
    let (data, first_sector, end_sector) = vertexfs_file_extent_payload(image, inode)?;
    if vertexfs_checksum32(data) == inode.checksum {
        return Ok((data, first_sector, end_sector));
    }
    if let Some(record) = journal
        && record.target_inode == inode.id
        && vertexfs_extent_starts_with(image, inode, record.payload)
    {
        return Ok((record.payload, first_sector, end_sector));
    }
    reject_vertexfs_image("file checksum mismatch")
}

fn vertexfs_file_extent_payload(
    image: &[u8],
    inode: VertexFsInode,
) -> Result<(&[u8], u64, u64), InitError> {
    if inode.sector_count == 0 {
        return reject_vertexfs_image("file has no extent");
    }
    let Some(end_sector) = inode.first_sector.checked_add(inode.sector_count as u64) else {
        return reject_vertexfs_image("file extent overflow");
    };
    if inode.first_sector < VERTEXFS_DATA_SECTOR
        || end_sector > VERTEXFS_DATA_SECTOR + VERTEXFS_DATA_SECTORS
    {
        return reject_vertexfs_image("file extent outside data section");
    }
    let max_len = inode.sector_count as u64 * VERTEXFS_SECTOR_SIZE as u64;
    if inode.size > max_len || inode.size as usize > MAX_VERTEXFS_FILE_BYTES {
        return reject_vertexfs_image("file length invalid");
    }
    let Ok(start_sector) = usize::try_from(inode.first_sector) else {
        return reject_vertexfs_image("file sector overflow");
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("file offset overflow");
    };
    let Ok(len) = usize::try_from(inode.size) else {
        return reject_vertexfs_image("file length overflow");
    };
    let Some(end) = start.checked_add(len) else {
        return reject_vertexfs_image("file data overflow");
    };
    let Some(data) = image.get(start..end) else {
        return reject_vertexfs_image("file data out of bounds");
    };
    Ok((data, inode.first_sector, end_sector))
}

fn vertexfs_extent_starts_with(image: &[u8], inode: VertexFsInode, payload: &[u8]) -> bool {
    let Some(start_sector) = usize::try_from(inode.first_sector).ok() else {
        return false;
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return false;
    };
    let Some(end) = start.checked_add(payload.len()) else {
        return false;
    };
    image
        .get(start..end)
        .map(|stored| stored == payload)
        .unwrap_or(false)
}

fn vertexfs_validate_free_map(
    image: &[u8],
    readme: VertexFsInode,
    app_a: VertexFsInode,
    dynamic: &[Option<VertexFsInode>; VERTEXFS_DYNAMIC_FILE_CAPACITY],
) -> Result<(), InitError> {
    let sector = vertexfs_sector(image, VERTEXFS_FREE_MAP_SECTOR)?;
    if !vertexfs_magic_matches(sector, VERTEXFS_FREE_MAP_MAGIC) {
        return reject_vertexfs_image("bad free-space map");
    }
    if read_u16_le(sector, 16) != VERTEXFS_VERSION || !vertexfs_checksum_valid(sector) {
        return reject_vertexfs_image("free-space metadata invalid");
    }
    if read_u16_le(sector, 18) as usize != VERTEXFS_SECTORS {
        return reject_vertexfs_image("free-space sector count mismatch");
    }

    let mut index = 0;
    while index < VERTEXFS_SECTORS {
        let allocated = sector[32 + index];
        if allocated != 0 && allocated != 1 {
            return reject_vertexfs_image("free-space allocation byte invalid");
        }
        let sector_number = index as u64;
        let mut dynamic_allocated = false;
        let mut dynamic_index = 0;
        while dynamic_index < VERTEXFS_DYNAMIC_FILE_CAPACITY {
            if let Some(inode) = dynamic[dynamic_index]
                && vertexfs_inode_covers_sector(inode, sector_number)
            {
                dynamic_allocated = true;
                break;
            }
            dynamic_index += 1;
        }
        let expected = sector_number == 0
            || vertexfs_sector_in_section(
                sector_number,
                VERTEXFS_INODE_TABLE_SECTOR,
                VERTEXFS_INODE_TABLE_SECTORS,
            )
            || vertexfs_sector_in_section(
                sector_number,
                VERTEXFS_DIRECTORY_SECTOR,
                VERTEXFS_DIRECTORY_SECTORS,
            )
            || sector_number == VERTEXFS_FREE_MAP_SECTOR
            || sector_number == VERTEXFS_JOURNAL_SECTOR
            || vertexfs_inode_covers_sector(readme, sector_number)
            || vertexfs_inode_covers_sector(app_a, sector_number)
            || dynamic_allocated;
        if (allocated == 1) != expected {
            return reject_vertexfs_image("free-space metadata mismatch");
        }
        index += 1;
    }
    Ok(())
}

fn vertexfs_sector(image: &[u8], sector: u64) -> Result<&[u8], InitError> {
    let Ok(sector_index) = usize::try_from(sector) else {
        return reject_vertexfs_image("sector overflow");
    };
    let Some(offset) = sector_index.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("sector offset overflow");
    };
    let Some(end) = offset.checked_add(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("sector end overflow");
    };
    let Some(bytes) = image.get(offset..end) else {
        return reject_vertexfs_image("sector out of bounds");
    };
    Ok(bytes)
}

fn vertexfs_section(
    image: &[u8],
    first_sector: u64,
    sector_count: u64,
) -> Result<&[u8], InitError> {
    let Ok(start_sector) = usize::try_from(first_sector) else {
        return reject_vertexfs_image("section overflow");
    };
    let Ok(sector_count) = usize::try_from(sector_count) else {
        return reject_vertexfs_image("section overflow");
    };
    let Some(start) = start_sector.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("section offset overflow");
    };
    let Some(len) = sector_count.checked_mul(VERTEXFS_SECTOR_SIZE) else {
        return reject_vertexfs_image("section length overflow");
    };
    let Some(end) = start.checked_add(len) else {
        return reject_vertexfs_image("section end overflow");
    };
    let Some(bytes) = image.get(start..end) else {
        return reject_vertexfs_image("section out of bounds");
    };
    Ok(bytes)
}

fn vertexfs_image_sector_mut(image: &mut [u8], sector: u64) -> Result<&mut [u8], IpcError> {
    let sector_index = usize::try_from(sector).map_err(|_| IpcError::VfsNoSpace)?;
    let start = sector_index
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let end = start
        .checked_add(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    image.get_mut(start..end).ok_or(IpcError::VfsNoSpace)
}

fn vertexfs_image_section(
    image: &[u8],
    first_sector: u64,
    sector_count: u64,
) -> Result<&[u8], IpcError> {
    let start_sector = usize::try_from(first_sector).map_err(|_| IpcError::VfsNoSpace)?;
    let sector_count = usize::try_from(sector_count).map_err(|_| IpcError::VfsNoSpace)?;
    let start = start_sector
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let len = sector_count
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let end = start.checked_add(len).ok_or(IpcError::VfsNoSpace)?;
    image.get(start..end).ok_or(IpcError::VfsNoSpace)
}

fn vertexfs_image_section_mut(
    image: &mut [u8],
    first_sector: u64,
    sector_count: u64,
) -> Result<&mut [u8], IpcError> {
    let start_sector = usize::try_from(first_sector).map_err(|_| IpcError::VfsNoSpace)?;
    let sector_count = usize::try_from(sector_count).map_err(|_| IpcError::VfsNoSpace)?;
    let start = start_sector
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let len = sector_count
        .checked_mul(VERTEXFS_SECTOR_SIZE)
        .ok_or(IpcError::VfsNoSpace)?;
    let end = start.checked_add(len).ok_or(IpcError::VfsNoSpace)?;
    image.get_mut(start..end).ok_or(IpcError::VfsNoSpace)
}

fn vertexfs_read_inode(sector: &[u8], index: usize) -> VertexFsInode {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN;
    VertexFsInode {
        id: read_u32_le(sector, offset),
        kind: read_u16_le(sector, offset + 4),
        size: read_u64_le(sector, offset + 8),
        first_sector: read_u64_le(sector, offset + 16),
        sector_count: read_u32_le(sector, offset + 24),
        checksum: read_u32_le(sector, offset + 28),
        parent: read_u32_le(sector, offset + 32),
    }
}

fn vertexfs_inode_name_eq(sector: &[u8], index: usize, expected: &[u8]) -> bool {
    let offset = vertexfs_inode_name_offset(index);
    vertexfs_fixed_string_eq(sector, offset, 28, expected)
}

fn vertexfs_inode_reserved_zero(sector: &[u8], index: usize) -> bool {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN + 6;
    read_u16_le(sector, offset) == 0
}

fn vertexfs_inode_name_offset(index: usize) -> usize {
    VERTEXFS_INODE_ENTRY_OFFSET + index * VERTEXFS_INODE_ENTRY_LEN + 36
}

fn vertexfs_directory_entry_eq(
    sector: &[u8],
    index: usize,
    parent: u32,
    child: u32,
    kind: u16,
    name: &[u8],
) -> bool {
    let offset = VERTEXFS_DIRECTORY_ENTRY_OFFSET + index * VERTEXFS_DIRECTORY_ENTRY_LEN;
    read_u32_le(sector, offset) == parent
        && read_u32_le(sector, offset + 4) == child
        && read_u16_le(sector, offset + 8) == kind
        && read_u16_le(sector, offset + 10) == 0
        && vertexfs_fixed_string_eq(sector, offset + 12, VERTEXFS_DIRECTORY_NAME_BYTES, name)
}

fn vertexfs_inode_covers_sector(inode: VertexFsInode, sector: u64) -> bool {
    inode
        .first_sector
        .checked_add(inode.sector_count as u64)
        .is_some_and(|end| sector >= inode.first_sector && sector < end)
}

fn vertexfs_sector_in_section(sector: u64, first_sector: u64, sector_count: u64) -> bool {
    first_sector
        .checked_add(sector_count)
        .is_some_and(|end| sector >= first_sector && sector < end)
}

fn vertexfs_section_matches(
    superblock: &[u8],
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> bool {
    let offset = VERTEXFS_SECTION_TABLE_OFFSET + index * VERTEXFS_SECTION_RECORD_LEN;
    read_u64_le(superblock, offset) == first_sector
        && read_u64_le(superblock, offset + 8) == sector_count
}

fn vertexfs_magic_matches(sector: &[u8], magic: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < magic.len() {
        if sector[index] != magic[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn vertexfs_fixed_string(buffer: &[u8], offset: usize, max_len: usize) -> Result<&[u8], InitError> {
    if offset
        .checked_add(max_len)
        .is_none_or(|end| end > buffer.len())
    {
        return reject_vertexfs_image("fixed string bounds invalid");
    }
    let mut len = 0;
    while len < max_len && buffer[offset + len] != 0 {
        let byte = buffer[offset + len];
        if !byte.is_ascii_graphic() && byte != b' ' {
            return reject_vertexfs_image("fixed string byte invalid");
        }
        len += 1;
    }
    if len == 0 {
        return reject_vertexfs_image("fixed string empty");
    }
    let mut index = len;
    while index < max_len {
        if buffer[offset + index] != 0 {
            return reject_vertexfs_image("fixed string not zero padded");
        }
        index += 1;
    }
    Ok(&buffer[offset..offset + len])
}

fn vertexfs_fixed_string_eq(buffer: &[u8], offset: usize, max_len: usize, expected: &[u8]) -> bool {
    if expected.is_empty()
        || expected.len() > max_len
        || offset
            .checked_add(max_len)
            .is_none_or(|end| end > buffer.len())
    {
        return false;
    }
    let mut index = 0;
    while index < expected.len() {
        if buffer[offset + index] != expected[index] {
            return false;
        }
        index += 1;
    }
    while index < max_len {
        if buffer[offset + index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn vertexfs_checksum_valid(bytes: &[u8]) -> bool {
    let stored = read_u32_le(bytes, VERTEXFS_CHECKSUM_OFFSET);
    let mut checksum = 0u32;
    let mut index = 0;
    while index < bytes.len() {
        let byte = if index >= VERTEXFS_CHECKSUM_OFFSET && index < VERTEXFS_CHECKSUM_OFFSET + 4 {
            0
        } else {
            bytes[index]
        };
        checksum = checksum.wrapping_add((byte as u32).wrapping_mul(index as u32 + 1));
        index += 1;
    }
    checksum == stored
}

fn write_vertexfs_sector_checksum(sector: &mut [u8]) {
    write_u32_le(sector, VERTEXFS_CHECKSUM_OFFSET, 0);
    let checksum = vertexfs_checksum32(sector);
    write_u32_le(sector, VERTEXFS_CHECKSUM_OFFSET, checksum);
}

fn vertexfs_inode_offset_by_id(sector: &[u8], inode_id: u32) -> Result<usize, IpcError> {
    let count = read_u16_le(sector, 18) as usize;
    let mut index = 0;
    while index < count {
        let offset = vertexfs_inode_offset(index)?;
        if read_u32_le(sector, offset) == inode_id {
            return Ok(offset);
        }
        index += 1;
    }
    Err(IpcError::VfsUnsupported)
}

fn vertexfs_dynamic_index_for_inode(inode_id: u32) -> Result<usize, IpcError> {
    if inode_id < VERTEXFS_DYNAMIC_INODE_FIRST {
        return Err(IpcError::VfsUnsupported);
    }
    let index = usize::try_from(inode_id - VERTEXFS_DYNAMIC_INODE_FIRST)
        .map_err(|_| IpcError::VfsUnsupported)?;
    if index >= VERTEXFS_DYNAMIC_FILE_CAPACITY {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(index)
}

fn vertexfs_inode_offset(index: usize) -> Result<usize, IpcError> {
    let offset = VERTEXFS_INODE_ENTRY_OFFSET
        .checked_add(
            index
                .checked_mul(VERTEXFS_INODE_ENTRY_LEN)
                .ok_or(IpcError::VfsUnsupported)?,
        )
        .ok_or(IpcError::VfsUnsupported)?;
    if offset
        .checked_add(VERTEXFS_INODE_ENTRY_LEN)
        .is_none_or(|end| end > VERTEXFS_INODE_TABLE_BYTES)
    {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(offset)
}

fn vertexfs_directory_offset(index: usize) -> Result<usize, IpcError> {
    let offset = VERTEXFS_DIRECTORY_ENTRY_OFFSET
        .checked_add(
            index
                .checked_mul(VERTEXFS_DIRECTORY_ENTRY_LEN)
                .ok_or(IpcError::VfsUnsupported)?,
        )
        .ok_or(IpcError::VfsUnsupported)?;
    if offset
        .checked_add(VERTEXFS_DIRECTORY_ENTRY_LEN)
        .is_none_or(|end| end > VERTEXFS_DIRECTORY_BYTES)
    {
        return Err(IpcError::VfsUnsupported);
    }
    Ok(offset)
}

fn write_vertexfs_fixed_vfs_name(
    sector: &mut [u8],
    offset: usize,
    max_len: usize,
    name: VfsName,
) -> Result<(), IpcError> {
    if name.len == 0
        || name.len > max_len
        || offset
            .checked_add(max_len)
            .is_none_or(|end| end > sector.len())
    {
        return Err(IpcError::VfsUnsupported);
    }
    let mut index = 0;
    while index < max_len {
        sector[offset + index] = 0;
        index += 1;
    }
    copy_bytes(&mut sector[offset..offset + name.len], name.as_bytes());
    Ok(())
}

fn copy_bytes(destination: &mut [u8], source: &[u8]) {
    let mut index = 0;
    while index < source.len() {
        destination[index] = source[index];
        index += 1;
    }
}

fn reject_vertexfs_image<T>(reason: &str) -> Result<T, InitError> {
    serial::write_str("Krust VertexFS v1 image rejected: ");
    serial::write_str(reason);
    serial::write_str("\n");
    Err(InitError::InvalidBootManifest)
}

fn read_u16_le(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([source[offset], source[offset + 1]])
}

fn read_u32_le(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        source[offset],
        source[offset + 1],
        source[offset + 2],
        source[offset + 3],
    ])
}

fn read_u64_le(source: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        source[offset],
        source[offset + 1],
        source[offset + 2],
        source[offset + 3],
        source[offset + 4],
        source[offset + 5],
        source[offset + 6],
        source[offset + 7],
    ])
}

fn write_u16_le(destination: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    destination[offset] = bytes[0];
    destination[offset + 1] = bytes[1];
}

fn write_u32_le(destination: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}

fn write_u64_le(destination: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        destination[offset + index] = bytes[index];
        index += 1;
    }
}
