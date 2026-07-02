use std::collections::{BTreeMap, BTreeSet};

use vertex_ir::GenerationManifest;

const SECTOR_SIZE: usize = 512;
const SECTORS: usize = 64;
const SUPERBLOCK_MAGIC: &[u8; 16] = b"VERTEXFSV1\0\0\0\0\0\0";
const INODE_TABLE_MAGIC: &[u8; 16] = b"VFSINODEV1\0\0\0\0\0\0";
const DIRECTORY_MAGIC: &[u8; 16] = b"VFSDIRV1\0\0\0\0\0\0\0\0";
const FREE_MAP_MAGIC: &[u8; 16] = b"VFSFREEV1\0\0\0\0\0\0\0";
const JOURNAL_MAGIC: &[u8; 16] = b"VFSJOURNALV1\0\0\0\0";
const SUPERBLOCK_MAGIC_V2: &[u8; 16] = b"VERTEXFSV2\0\0\0\0\0\0";
const INODE_TABLE_MAGIC_V2: &[u8; 16] = b"VFSINODEV2\0\0\0\0\0\0";
const DIRECTORY_MAGIC_V2: &[u8; 16] = b"VFSDIRV2\0\0\0\0\0\0\0\0";
const FREE_MAP_MAGIC_V2: &[u8; 16] = b"VFSFREEV2\0\0\0\0\0\0\0";
const JOURNAL_MAGIC_V2: &[u8; 16] = b"VFSJOURNALV2\0\0\0\0";
const VERSION: u16 = 1;
const VERSION_V2: u16 = 2;
const CHECKSUM_OFFSET: usize = 20;
const FEATURE_METADATA_V1: u32 = 1;
const FEATURE_DIRECTORY_CHECKSUMS: u32 = 1 << 1;
const FEATURE_FREE_SPACE_CHECKSUMS: u32 = 1 << 2;
const FEATURE_JOURNAL_V1: u32 = 1 << 3;
const FEATURE_FLAGS: u32 = FEATURE_METADATA_V1
    | FEATURE_DIRECTORY_CHECKSUMS
    | FEATURE_FREE_SPACE_CHECKSUMS
    | FEATURE_JOURNAL_V1;
const FEATURE_METADATA_V2: u32 = 1;
const FEATURE_JOURNAL_V2: u32 = 1 << 3;
const FEATURE_PAYLOAD_CHECKSUMS: u32 = 1 << 4;
const FEATURE_FLAGS_V2: u32 = FEATURE_METADATA_V2
    | FEATURE_DIRECTORY_CHECKSUMS
    | FEATURE_FREE_SPACE_CHECKSUMS
    | FEATURE_JOURNAL_V2
    | FEATURE_PAYLOAD_CHECKSUMS;
const GENERATION_OFFSET: usize = 32;
const V2_VOLUME_OFFSET: usize = 32;
const V2_GENERATION_OFFSET: usize = 96;
const SECTION_TABLE_OFFSET: usize = 128;
const SECTION_TABLE_OFFSET_V2: usize = 192;
const SECTION_RECORD_LEN: usize = 16;
const INODE_TABLE_SECTOR: u64 = 1;
const INODE_TABLE_SECTORS: u64 = 2;
const DIRECTORY_SECTOR: u64 = INODE_TABLE_SECTOR + INODE_TABLE_SECTORS;
const DIRECTORY_SECTORS: u64 = 2;
const FREE_MAP_SECTOR: u64 = DIRECTORY_SECTOR + DIRECTORY_SECTORS;
const JOURNAL_SECTOR: u64 = FREE_MAP_SECTOR + 1;
const DATA_SECTOR: u64 = JOURNAL_SECTOR + 1;
const DATA_SECTORS: u64 = (SECTORS as u64) - DATA_SECTOR;
const INODE_TABLE_SECTOR_V2: u64 = 1;
const INODE_TABLE_SECTORS_V2: u64 = 8;
const DIRECTORY_SECTOR_V2: u64 = INODE_TABLE_SECTOR_V2 + INODE_TABLE_SECTORS_V2;
const DIRECTORY_SECTORS_V2: u64 = 8;
const FREE_MAP_SECTOR_V2: u64 = DIRECTORY_SECTOR_V2 + DIRECTORY_SECTORS_V2;
const JOURNAL_SECTOR_V2: u64 = FREE_MAP_SECTOR_V2 + 1;
const DATA_SECTOR_V2: u64 = JOURNAL_SECTOR_V2 + 1;
const DATA_SECTORS_V2: u64 = (SECTORS as u64) - DATA_SECTOR_V2;
const INODE_ENTRY_OFFSET: usize = 32;
const INODE_ENTRY_LEN: usize = 64;
const INODE_TABLE_BYTES: usize = SECTOR_SIZE * INODE_TABLE_SECTORS as usize;
const INODE_TABLE_BYTES_V2: usize = SECTOR_SIZE * INODE_TABLE_SECTORS_V2 as usize;
const DIRECTORY_ENTRY_OFFSET: usize = 32;
const DIRECTORY_ENTRY_LEN: usize = 64;
const DIRECTORY_NAME_BYTES: usize = DIRECTORY_ENTRY_LEN - 12;
const DIRECTORY_BYTES: usize = SECTOR_SIZE * DIRECTORY_SECTORS as usize;
const DIRECTORY_BYTES_V2: usize = SECTOR_SIZE * DIRECTORY_SECTORS_V2 as usize;
const INODE_ROOT: u32 = 1;
const INODE_README: u32 = 2;
const INODE_APP_DIR: u32 = 3;
const INODE_APP_A: u32 = 4;
const BASE_INODE_COUNT: usize = 4;
const BASE_DIRECTORY_COUNT: usize = 3;
const INODE_ENTRY_CAPACITY: usize = (INODE_TABLE_BYTES - INODE_ENTRY_OFFSET) / INODE_ENTRY_LEN;
const DIRECTORY_ENTRY_CAPACITY: usize =
    (DIRECTORY_BYTES - DIRECTORY_ENTRY_OFFSET) / DIRECTORY_ENTRY_LEN;
const INODE_ENTRY_CAPACITY_V2: usize =
    (INODE_TABLE_BYTES_V2 - INODE_ENTRY_OFFSET) / INODE_ENTRY_LEN;
const DIRECTORY_ENTRY_CAPACITY_V2: usize =
    (DIRECTORY_BYTES_V2 - DIRECTORY_ENTRY_OFFSET) / DIRECTORY_ENTRY_LEN;
const DYNAMIC_FILE_CAPACITY: usize = INODE_ENTRY_CAPACITY - BASE_INODE_COUNT;
const DIRECTORY_DYNAMIC_FILE_CAPACITY: usize = DIRECTORY_ENTRY_CAPACITY - BASE_DIRECTORY_COUNT;
const DYNAMIC_FILE_CAPACITY_V2: usize = DATA_SECTORS_V2 as usize - 2;
const KIND_DIR: u16 = 1;
const KIND_FILE: u16 = 2;
const README_SECTOR: u64 = DATA_SECTOR;
const APP_A_SECTOR: u64 = DATA_SECTOR + 1;
const JOURNAL_STATE_CLEAN: u16 = 0;
const JOURNAL_STATE_PENDING: u16 = 1;
const JOURNAL_PAYLOAD_OFFSET: usize = 64;
const MAX_JOURNAL_FILE_BYTES: usize = SECTOR_SIZE - JOURNAL_PAYLOAD_OFFSET;
const README_BYTES: &[u8] = b"VertexFS v1 root\n";
const README_BYTES_V2: &[u8] = b"VertexFS v2 root\n";
const APP_A_BYTES: &[u8] = b"vertexfs:a=1\n";
const APP_A_REPLAY_BYTES: &[u8] = b"vertexfs:a=2\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexFsFormat {
    V1,
    V2,
}

impl VertexFsFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "v1" => Ok(Self::V1),
            "v2" => Ok(Self::V2),
            other => Err(format!(
                "unsupported VertexFS format {other}; expected v1 or v2"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
}

pub struct Report {
    pub format: VertexFsFormat,
    pub generation: String,
    pub feature_flags: u32,
    pub directories: usize,
    pub files: Vec<FileReport>,
}

pub struct FileReport {
    pub path: String,
    pub len: u64,
    pub checksum: u32,
    pub first_sector: u64,
    pub sector_count: u32,
}

#[derive(Clone)]
struct Inode {
    id: u32,
    kind: u16,
    size: u64,
    first_sector: u64,
    sector_count: u32,
    checksum: u32,
    parent: u32,
    name: String,
}

#[derive(Clone)]
struct DirectoryEntry {
    parent: u32,
    child: u32,
    kind: u16,
    name: String,
}

struct JournalRecord<'a> {
    target_inode: u32,
    payload: &'a [u8],
}

pub fn create_image_with_format(
    manifest: &GenerationManifest,
    format: VertexFsFormat,
) -> Result<Vec<u8>, String> {
    match format {
        VertexFsFormat::V1 => create_image_v1(manifest),
        VertexFsFormat::V2 => create_image_v2(manifest),
    }
}

fn create_image_v1(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let mut image = vec![0u8; SECTOR_SIZE * SECTORS];
    write_superblock(&mut image, &manifest.generation.id)?;
    write_inode_table(&mut image)?;
    write_directory(&mut image)?;
    write_journal_clean(&mut image)?;
    write_file_payloads(&mut image);
    write_free_map(&mut image);
    Ok(image)
}

pub fn inspect(bytes: &[u8]) -> Result<Report, String> {
    verify(bytes)
}

pub fn verify(bytes: &[u8]) -> Result<Report, String> {
    if looks_like_v2(bytes) {
        return verify_v2(bytes);
    }
    let superblock = parse_superblock(bytes)?;
    let inodes = parse_inode_table(bytes)?;
    let directory = parse_directory(bytes)?;
    let journal = parse_journal(bytes)?;
    let free_map = parse_free_map(bytes)?;
    verify_directory_graph(&inodes, &directory)?;
    verify_journal(&inodes, journal.as_ref())?;
    let files = verify_file_extents(bytes, &inodes, &free_map, journal.as_ref())?;
    let directories = inodes
        .values()
        .filter(|inode| inode.kind == KIND_DIR)
        .count();
    Ok(Report {
        format: VertexFsFormat::V1,
        generation: superblock.generation,
        feature_flags: superblock.feature_flags,
        directories,
        files,
    })
}

pub fn corrupt(bytes: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    if looks_like_v2(bytes) {
        return corrupt_v2(bytes, mode);
    }
    let mut out = bytes.to_vec();
    match mode {
        "bad-superblock" => {
            let byte = out
                .first_mut()
                .ok_or_else(|| "cannot corrupt empty VertexFS image".to_owned())?;
            *byte = b'X';
        }
        "bad-directory" => {
            let section = section_mut(&mut out, DIRECTORY_SECTOR, DIRECTORY_SECTORS)?;
            section[DIRECTORY_ENTRY_OFFSET + 12] ^= 1;
        }
        "overlapping-extents" => {
            let sector = section_mut(&mut out, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
            let readme_offset = inode_offset(1)?;
            let app_a_offset = inode_offset(3)?;
            let readme_size = read_u64(sector, readme_offset + 8)?;
            let readme_sector = read_u64(sector, readme_offset + 16)?;
            let readme_sector_count = read_u32(sector, readme_offset + 24)?;
            let readme_checksum = read_u32(sector, readme_offset + 28)?;
            write_u64(sector, app_a_offset + 8, readme_size)?;
            write_u64(sector, app_a_offset + 16, readme_sector)?;
            write_u32(sector, app_a_offset + 24, readme_sector_count)?;
            write_u32(sector, app_a_offset + 28, readme_checksum)?;
            write_checksum(sector)?;
        }
        "free-space-overlap" => {
            let sector = sector_mut(&mut out, FREE_MAP_SECTOR)?;
            sector[32 + APP_A_SECTOR as usize] = 0;
            write_checksum(sector)?;
        }
        "interrupted-journal" | "journal-checkpoint-after-journal" => {
            let _ = verify(bytes)?;
            write_journal_pending(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
        }
        "journal-checkpoint-after-data" => {
            let _ = verify(bytes)?;
            let inodes = parse_inode_table(bytes)?;
            let inode = inodes
                .get(&INODE_APP_A)
                .ok_or_else(|| "VertexFS checkpoint rejected: missing /app/a inode".to_owned())?;
            write_journal_pending(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
            write_file_payload_for_inode(&mut out, inode, APP_A_REPLAY_BYTES)?;
        }
        "journal-checkpoint-after-inode" => {
            let _ = verify(bytes)?;
            let inodes = parse_inode_table(bytes)?;
            let inode = inodes
                .get(&INODE_APP_A)
                .ok_or_else(|| "VertexFS checkpoint rejected: missing /app/a inode".to_owned())?;
            write_journal_pending(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
            write_file_payload_for_inode(&mut out, inode, APP_A_REPLAY_BYTES)?;
            write_inode_payload_metadata(&mut out, inode.id, APP_A_REPLAY_BYTES)?;
        }
        other => {
            return Err(format!(
                "unknown VertexFS corruption mode {other}; expected bad-superblock, bad-directory, overlapping-extents, free-space-overlap, interrupted-journal, journal-checkpoint-after-journal, journal-checkpoint-after-data, or journal-checkpoint-after-inode"
            ));
        }
    }
    Ok(out)
}

pub fn update_file(bytes: &[u8], path: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
    if looks_like_v2(bytes) {
        return update_file_v2(bytes, path, payload);
    }
    let _ = verify(bytes)?;
    let inodes = parse_inode_table(bytes)?;
    let directory = parse_directory(bytes)?;
    verify_directory_graph(&inodes, &directory)?;
    if parse_journal(bytes)?.is_some() {
        return Err("VertexFS update rejected: pending journal present".to_owned());
    }
    let inode = inode_by_path(&inodes, path)?;
    if inode.kind != KIND_FILE {
        return Err("VertexFS update rejected: target is not a file".to_owned());
    }
    if payload.len() > MAX_JOURNAL_FILE_BYTES {
        return Err("VertexFS update rejected: payload too large for journal".to_owned());
    }
    if payload.len() as u64 > inode.sector_count as u64 * SECTOR_SIZE as u64 {
        return Err("VertexFS update rejected: payload exceeds file extent".to_owned());
    }

    let mut out = bytes.to_vec();
    write_journal_pending(&mut out, inode.id, payload)?;
    write_file_payload_for_inode(&mut out, inode, payload)?;
    write_inode_payload_metadata(&mut out, inode.id, payload)?;
    write_journal_clean(&mut out)?;
    let _ = verify(&out)?;
    Ok(out)
}

fn create_image_v2(manifest: &GenerationManifest) -> Result<Vec<u8>, String> {
    let mut image = vec![0u8; SECTOR_SIZE * SECTORS];
    write_superblock_v2(&mut image, &manifest.generation.id)?;
    write_inode_table_v2(&mut image)?;
    write_directory_v2(&mut image)?;
    write_journal_clean_v2(&mut image)?;
    write_file_payloads_v2(&mut image);
    write_free_map_v2(&mut image);
    Ok(image)
}

fn looks_like_v2(bytes: &[u8]) -> bool {
    bytes.starts_with(SUPERBLOCK_MAGIC_V2)
        || bytes
            .get(16..18)
            .map(|version| version == VERSION_V2.to_le_bytes())
            .unwrap_or(false)
}

fn verify_v2(bytes: &[u8]) -> Result<Report, String> {
    let superblock = parse_superblock_v2(bytes)?;
    let inodes = parse_inode_table_v2(bytes)?;
    let directory = parse_directory_v2(bytes)?;
    let journal = parse_journal_v2(bytes)?;
    let free_map = parse_free_map_v2(bytes)?;
    verify_directory_graph(&inodes, &directory)?;
    verify_journal(&inodes, journal.as_ref())?;
    let files = verify_file_extents_v2(bytes, &inodes, &free_map, journal.as_ref())?;
    let directories = inodes
        .values()
        .filter(|inode| inode.kind == KIND_DIR)
        .count();
    Ok(Report {
        format: VertexFsFormat::V2,
        generation: superblock.generation,
        feature_flags: superblock.feature_flags,
        directories,
        files,
    })
}

fn corrupt_v2(bytes: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let mut out = bytes.to_vec();
    match mode {
        "bad-superblock" => {
            let byte = out
                .first_mut()
                .ok_or_else(|| "cannot corrupt empty VertexFS image".to_owned())?;
            *byte = b'X';
        }
        "unsupported-feature-flag" => {
            let sector = sector_mut(&mut out, 0)?;
            let flags = read_u32(sector, 28)?;
            write_u32(sector, 28, flags | 0x8000_0000)?;
            write_checksum(sector)?;
        }
        "bad-inode" => {
            let section = section_mut(&mut out, INODE_TABLE_SECTOR_V2, INODE_TABLE_SECTORS_V2)?;
            let offset = inode_offset_v2(0)?;
            write_u32(section, offset + 32, 99)?;
            write_checksum(section)?;
        }
        "bad-directory" => {
            let section = section_mut(&mut out, DIRECTORY_SECTOR_V2, DIRECTORY_SECTORS_V2)?;
            let offset = directory_offset_v2(2)?;
            write_u32(section, offset + 4, 99)?;
            write_checksum(section)?;
        }
        "bad-free-map" | "free-space-overlap" => {
            let sector = sector_mut(&mut out, FREE_MAP_SECTOR_V2)?;
            sector[32 + (DATA_SECTOR_V2 + 1) as usize] = 0;
            write_checksum(sector)?;
        }
        "bad-journal" => {
            let sector = sector_mut(&mut out, JOURNAL_SECTOR_V2)?;
            sector[0] = b'X';
        }
        "overlapping-extents" => {
            let section = section_mut(&mut out, INODE_TABLE_SECTOR_V2, INODE_TABLE_SECTORS_V2)?;
            let readme_offset = inode_offset_v2(1)?;
            let app_a_offset = inode_offset_v2(3)?;
            let readme_size = read_u64(section, readme_offset + 8)?;
            let readme_sector = read_u64(section, readme_offset + 16)?;
            let readme_sector_count = read_u32(section, readme_offset + 24)?;
            let readme_checksum = read_u32(section, readme_offset + 28)?;
            write_u64(section, app_a_offset + 8, readme_size)?;
            write_u64(section, app_a_offset + 16, readme_sector)?;
            write_u32(section, app_a_offset + 24, readme_sector_count)?;
            write_u32(section, app_a_offset + 28, readme_checksum)?;
            write_checksum(section)?;
        }
        "interrupted-journal" | "journal-checkpoint-after-journal" => {
            let _ = verify_v2(bytes)?;
            write_journal_pending_v2(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
        }
        "journal-checkpoint-after-data" => {
            let _ = verify_v2(bytes)?;
            let inodes = parse_inode_table_v2(bytes)?;
            let inode = inodes.get(&INODE_APP_A).ok_or_else(|| {
                "VertexFS v2 checkpoint rejected: missing /app/a inode".to_owned()
            })?;
            write_journal_pending_v2(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
            write_file_payload_for_inode(&mut out, inode, APP_A_REPLAY_BYTES)?;
        }
        "journal-checkpoint-after-inode" => {
            let _ = verify_v2(bytes)?;
            let inodes = parse_inode_table_v2(bytes)?;
            let inode = inodes.get(&INODE_APP_A).ok_or_else(|| {
                "VertexFS v2 checkpoint rejected: missing /app/a inode".to_owned()
            })?;
            write_journal_pending_v2(&mut out, INODE_APP_A, APP_A_REPLAY_BYTES)?;
            write_file_payload_for_inode(&mut out, inode, APP_A_REPLAY_BYTES)?;
            write_inode_payload_metadata_v2(&mut out, inode.id, APP_A_REPLAY_BYTES)?;
        }
        other => {
            return Err(format!(
                "unknown VertexFS v2 corruption mode {other}; expected bad-superblock, unsupported-feature-flag, bad-inode, bad-directory, bad-free-map, bad-journal, overlapping-extents, free-space-overlap, interrupted-journal, journal-checkpoint-after-journal, journal-checkpoint-after-data, or journal-checkpoint-after-inode"
            ));
        }
    }
    Ok(out)
}

fn update_file_v2(bytes: &[u8], path: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
    let _ = verify_v2(bytes)?;
    let inodes = parse_inode_table_v2(bytes)?;
    let directory = parse_directory_v2(bytes)?;
    verify_directory_graph(&inodes, &directory)?;
    if parse_journal_v2(bytes)?.is_some() {
        return Err("VertexFS v2 update rejected: pending journal present".to_owned());
    }
    let inode = inode_by_path(&inodes, path)?;
    if inode.kind != KIND_FILE {
        return Err("VertexFS v2 update rejected: target is not a file".to_owned());
    }
    if payload.len() > MAX_JOURNAL_FILE_BYTES {
        return Err("VertexFS v2 update rejected: payload too large for journal".to_owned());
    }
    if payload.len() as u64 > inode.sector_count as u64 * SECTOR_SIZE as u64 {
        return Err("VertexFS v2 update rejected: payload exceeds file extent".to_owned());
    }

    let mut out = bytes.to_vec();
    write_journal_pending_v2(&mut out, inode.id, payload)?;
    write_file_payload_for_inode(&mut out, inode, payload)?;
    write_inode_payload_metadata_v2(&mut out, inode.id, payload)?;
    write_journal_clean_v2(&mut out)?;
    let _ = verify_v2(&out)?;
    Ok(out)
}

pub fn sector_size() -> usize {
    SECTOR_SIZE
}

pub fn sectors() -> usize {
    SECTORS
}

fn write_superblock(image: &mut [u8], generation: &str) -> Result<(), String> {
    let sector = sector_mut(image, 0)?;
    sector[..SUPERBLOCK_MAGIC.len()].copy_from_slice(SUPERBLOCK_MAGIC);
    write_u16(sector, 16, VERSION)?;
    write_u16(sector, 18, SECTOR_SIZE as u16)?;
    write_u32(sector, 24, SECTORS as u32)?;
    write_u32(sector, 28, FEATURE_FLAGS)?;
    write_fixed_str(sector, GENERATION_OFFSET, generation)?;
    write_section(sector, 0, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
    write_section(sector, 1, DIRECTORY_SECTOR, DIRECTORY_SECTORS)?;
    write_section(sector, 2, FREE_MAP_SECTOR, 1)?;
    write_section(sector, 3, JOURNAL_SECTOR, 1)?;
    write_section(sector, 4, DATA_SECTOR, DATA_SECTORS)?;
    write_checksum(sector)
}

fn write_superblock_v2(image: &mut [u8], generation: &str) -> Result<(), String> {
    let sector = sector_mut(image, 0)?;
    sector[..SUPERBLOCK_MAGIC_V2.len()].copy_from_slice(SUPERBLOCK_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2)?;
    write_u16(sector, 18, SECTOR_SIZE as u16)?;
    write_u32(sector, 24, SECTORS as u32)?;
    write_u32(sector, 28, FEATURE_FLAGS_V2)?;
    write_fixed_str(sector, V2_VOLUME_OFFSET, "vol:vertexfs-root")?;
    write_fixed_str(sector, V2_GENERATION_OFFSET, generation)?;
    write_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        0,
        INODE_TABLE_SECTOR_V2,
        INODE_TABLE_SECTORS_V2,
    )?;
    write_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        1,
        DIRECTORY_SECTOR_V2,
        DIRECTORY_SECTORS_V2,
    )?;
    write_section_at(sector, SECTION_TABLE_OFFSET_V2, 2, FREE_MAP_SECTOR_V2, 1)?;
    write_section_at(sector, SECTION_TABLE_OFFSET_V2, 3, JOURNAL_SECTOR_V2, 1)?;
    write_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        4,
        DATA_SECTOR_V2,
        DATA_SECTORS_V2,
    )?;
    write_checksum(sector)
}

fn write_inode_table(image: &mut [u8]) -> Result<(), String> {
    let sector = section_mut(image, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
    sector[..INODE_TABLE_MAGIC.len()].copy_from_slice(INODE_TABLE_MAGIC);
    write_u16(sector, 16, VERSION)?;
    write_u16(sector, 18, BASE_INODE_COUNT as u16)?;
    write_inode(
        sector,
        0,
        &Inode {
            id: INODE_ROOT,
            kind: KIND_DIR,
            size: 0,
            first_sector: 0,
            sector_count: 0,
            checksum: 0,
            parent: 0,
            name: "/".to_owned(),
        },
    )?;
    write_inode(
        sector,
        1,
        &Inode {
            id: INODE_README,
            kind: KIND_FILE,
            size: README_BYTES.len() as u64,
            first_sector: README_SECTOR,
            sector_count: 1,
            checksum: checksum32(README_BYTES),
            parent: INODE_ROOT,
            name: "readme".to_owned(),
        },
    )?;
    write_inode(
        sector,
        2,
        &Inode {
            id: INODE_APP_DIR,
            kind: KIND_DIR,
            size: 0,
            first_sector: 0,
            sector_count: 0,
            checksum: 0,
            parent: INODE_ROOT,
            name: "app".to_owned(),
        },
    )?;
    write_inode(
        sector,
        3,
        &Inode {
            id: INODE_APP_A,
            kind: KIND_FILE,
            size: APP_A_BYTES.len() as u64,
            first_sector: APP_A_SECTOR,
            sector_count: 1,
            checksum: checksum32(APP_A_BYTES),
            parent: INODE_APP_DIR,
            name: "a".to_owned(),
        },
    )?;
    write_checksum(sector)
}

fn write_inode_table_v2(image: &mut [u8]) -> Result<(), String> {
    let sector = section_mut(image, INODE_TABLE_SECTOR_V2, INODE_TABLE_SECTORS_V2)?;
    sector[..INODE_TABLE_MAGIC_V2.len()].copy_from_slice(INODE_TABLE_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2)?;
    write_u16(sector, 18, BASE_INODE_COUNT as u16)?;
    write_inode_v2(
        sector,
        0,
        &Inode {
            id: INODE_ROOT,
            kind: KIND_DIR,
            size: 0,
            first_sector: 0,
            sector_count: 0,
            checksum: 0,
            parent: 0,
            name: "/".to_owned(),
        },
    )?;
    write_inode_v2(
        sector,
        1,
        &Inode {
            id: INODE_README,
            kind: KIND_FILE,
            size: README_BYTES_V2.len() as u64,
            first_sector: DATA_SECTOR_V2,
            sector_count: 1,
            checksum: checksum32(README_BYTES_V2),
            parent: INODE_ROOT,
            name: "readme".to_owned(),
        },
    )?;
    write_inode_v2(
        sector,
        2,
        &Inode {
            id: INODE_APP_DIR,
            kind: KIND_DIR,
            size: 0,
            first_sector: 0,
            sector_count: 0,
            checksum: 0,
            parent: INODE_ROOT,
            name: "app".to_owned(),
        },
    )?;
    write_inode_v2(
        sector,
        3,
        &Inode {
            id: INODE_APP_A,
            kind: KIND_FILE,
            size: APP_A_BYTES.len() as u64,
            first_sector: DATA_SECTOR_V2 + 1,
            sector_count: 1,
            checksum: checksum32(APP_A_BYTES),
            parent: INODE_APP_DIR,
            name: "a".to_owned(),
        },
    )?;
    write_checksum(sector)
}

fn write_directory(image: &mut [u8]) -> Result<(), String> {
    let sector = section_mut(image, DIRECTORY_SECTOR, DIRECTORY_SECTORS)?;
    sector[..DIRECTORY_MAGIC.len()].copy_from_slice(DIRECTORY_MAGIC);
    write_u16(sector, 16, VERSION)?;
    write_u16(sector, 18, BASE_DIRECTORY_COUNT as u16)?;
    write_directory_entry(
        sector,
        0,
        &DirectoryEntry {
            parent: INODE_ROOT,
            child: INODE_README,
            kind: KIND_FILE,
            name: "readme".to_owned(),
        },
    )?;
    write_directory_entry(
        sector,
        1,
        &DirectoryEntry {
            parent: INODE_ROOT,
            child: INODE_APP_DIR,
            kind: KIND_DIR,
            name: "app".to_owned(),
        },
    )?;
    write_directory_entry(
        sector,
        2,
        &DirectoryEntry {
            parent: INODE_APP_DIR,
            child: INODE_APP_A,
            kind: KIND_FILE,
            name: "a".to_owned(),
        },
    )?;
    write_checksum(sector)
}

fn write_directory_v2(image: &mut [u8]) -> Result<(), String> {
    let sector = section_mut(image, DIRECTORY_SECTOR_V2, DIRECTORY_SECTORS_V2)?;
    sector[..DIRECTORY_MAGIC_V2.len()].copy_from_slice(DIRECTORY_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2)?;
    write_u16(sector, 18, BASE_DIRECTORY_COUNT as u16)?;
    write_directory_entry_v2(
        sector,
        0,
        &DirectoryEntry {
            parent: INODE_ROOT,
            child: INODE_README,
            kind: KIND_FILE,
            name: "readme".to_owned(),
        },
    )?;
    write_directory_entry_v2(
        sector,
        1,
        &DirectoryEntry {
            parent: INODE_ROOT,
            child: INODE_APP_DIR,
            kind: KIND_DIR,
            name: "app".to_owned(),
        },
    )?;
    write_directory_entry_v2(
        sector,
        2,
        &DirectoryEntry {
            parent: INODE_APP_DIR,
            child: INODE_APP_A,
            kind: KIND_FILE,
            name: "a".to_owned(),
        },
    )?;
    write_checksum(sector)
}

fn write_journal_clean(image: &mut [u8]) -> Result<(), String> {
    let sector = sector_mut(image, JOURNAL_SECTOR)?;
    sector[..JOURNAL_MAGIC.len()].copy_from_slice(JOURNAL_MAGIC);
    write_u16(sector, 16, VERSION)?;
    write_u16(sector, 18, JOURNAL_STATE_CLEAN)?;
    write_checksum(sector)
}

fn write_journal_clean_v2(image: &mut [u8]) -> Result<(), String> {
    let sector = sector_mut(image, JOURNAL_SECTOR_V2)?;
    sector[..JOURNAL_MAGIC_V2.len()].copy_from_slice(JOURNAL_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2)?;
    write_u16(sector, 18, JOURNAL_STATE_CLEAN)?;
    write_checksum(sector)
}

fn write_journal_pending(
    image: &mut [u8],
    target_inode: u32,
    payload: &[u8],
) -> Result<(), String> {
    if payload.len() > SECTOR_SIZE - JOURNAL_PAYLOAD_OFFSET {
        return Err("VertexFS journal rejected: payload too large".to_owned());
    }
    let sector = sector_mut(image, JOURNAL_SECTOR)?;
    sector.fill(0);
    sector[..JOURNAL_MAGIC.len()].copy_from_slice(JOURNAL_MAGIC);
    write_u16(sector, 16, VERSION)?;
    write_u16(sector, 18, JOURNAL_STATE_PENDING)?;
    write_u32(sector, 24, target_inode)?;
    write_u32(sector, 28, payload.len() as u32)?;
    write_u32(sector, 32, checksum32(payload))?;
    sector[JOURNAL_PAYLOAD_OFFSET..JOURNAL_PAYLOAD_OFFSET + payload.len()].copy_from_slice(payload);
    write_checksum(sector)
}

fn write_journal_pending_v2(
    image: &mut [u8],
    target_inode: u32,
    payload: &[u8],
) -> Result<(), String> {
    if payload.len() > SECTOR_SIZE - JOURNAL_PAYLOAD_OFFSET {
        return Err("VertexFS v2 journal rejected: payload too large".to_owned());
    }
    let sector = sector_mut(image, JOURNAL_SECTOR_V2)?;
    sector.fill(0);
    sector[..JOURNAL_MAGIC_V2.len()].copy_from_slice(JOURNAL_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2)?;
    write_u16(sector, 18, JOURNAL_STATE_PENDING)?;
    write_u32(sector, 24, target_inode)?;
    write_u32(sector, 28, payload.len() as u32)?;
    write_u32(sector, 32, checksum32(payload))?;
    write_u64(sector, 40, 1)?;
    write_u32(sector, 48, target_inode)?;
    write_u32(sector, 52, payload.len() as u32)?;
    write_u32(sector, 56, checksum32(payload))?;
    sector[JOURNAL_PAYLOAD_OFFSET..JOURNAL_PAYLOAD_OFFSET + payload.len()].copy_from_slice(payload);
    write_checksum(sector)
}

fn write_file_payloads(image: &mut [u8]) {
    write_bytes(image, README_SECTOR, README_BYTES);
    write_bytes(image, APP_A_SECTOR, APP_A_BYTES);
}

fn write_file_payloads_v2(image: &mut [u8]) {
    write_bytes(image, DATA_SECTOR_V2, README_BYTES_V2);
    write_bytes(image, DATA_SECTOR_V2 + 1, APP_A_BYTES);
}

fn write_file_payload_for_inode(
    image: &mut [u8],
    inode: &Inode,
    payload: &[u8],
) -> Result<(), String> {
    let start = inode
        .first_sector
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS update rejected: file offset overflow".to_owned())?;
    let extent_len = inode
        .sector_count
        .checked_mul(SECTOR_SIZE as u32)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS update rejected: extent length overflow".to_owned())?;
    if payload.len() > extent_len {
        return Err("VertexFS update rejected: payload exceeds file extent".to_owned());
    }
    let end = start
        .checked_add(extent_len)
        .ok_or_else(|| "VertexFS update rejected: file extent overflow".to_owned())?;
    let data = image
        .get_mut(start..end)
        .ok_or_else(|| "VertexFS update rejected: file extent out of bounds".to_owned())?;
    data.fill(0);
    data[..payload.len()].copy_from_slice(payload);
    Ok(())
}

fn write_inode_payload_metadata(
    image: &mut [u8],
    inode_id: u32,
    payload: &[u8],
) -> Result<(), String> {
    let inode_sector = section_mut(image, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
    let inode_offset = inode_offset_by_id(inode_sector, inode_id)?;
    write_u64(inode_sector, inode_offset + 8, payload.len() as u64)?;
    write_u32(inode_sector, inode_offset + 28, checksum32(payload))?;
    write_checksum(inode_sector)
}

fn write_inode_payload_metadata_v2(
    image: &mut [u8],
    inode_id: u32,
    payload: &[u8],
) -> Result<(), String> {
    let inode_sector = section_mut(image, INODE_TABLE_SECTOR_V2, INODE_TABLE_SECTORS_V2)?;
    let inode_offset = inode_offset_by_id_v2(inode_sector, inode_id)?;
    write_u64(inode_sector, inode_offset + 8, payload.len() as u64)?;
    write_u32(inode_sector, inode_offset + 28, checksum32(payload))?;
    write_checksum(inode_sector)
}

fn write_free_map(image: &mut [u8]) {
    let sector = sector_mut(image, FREE_MAP_SECTOR).expect("fixed VertexFS free map sector");
    sector[..FREE_MAP_MAGIC.len()].copy_from_slice(FREE_MAP_MAGIC);
    write_u16(sector, 16, VERSION).expect("fixed VertexFS free map version");
    write_u16(sector, 18, SECTORS as u16).expect("fixed VertexFS sector count");
    for index in 0..SECTORS {
        sector[32 + index] = 0;
    }
    for allocated in [
        0,
        INODE_TABLE_SECTOR,
        INODE_TABLE_SECTOR + 1,
        DIRECTORY_SECTOR,
        DIRECTORY_SECTOR + 1,
        FREE_MAP_SECTOR,
        JOURNAL_SECTOR,
        README_SECTOR,
        APP_A_SECTOR,
    ] {
        sector[32 + allocated as usize] = 1;
    }
    write_checksum(sector).expect("fixed VertexFS free map checksum");
}

fn write_free_map_v2(image: &mut [u8]) {
    let sector = sector_mut(image, FREE_MAP_SECTOR_V2).expect("fixed VertexFS v2 free map sector");
    sector[..FREE_MAP_MAGIC_V2.len()].copy_from_slice(FREE_MAP_MAGIC_V2);
    write_u16(sector, 16, VERSION_V2).expect("fixed VertexFS v2 free map version");
    write_u16(sector, 18, SECTORS as u16).expect("fixed VertexFS v2 sector count");
    for index in 0..SECTORS {
        sector[32 + index] = 0;
    }
    for allocated in [
        0,
        INODE_TABLE_SECTOR_V2,
        INODE_TABLE_SECTOR_V2 + 1,
        INODE_TABLE_SECTOR_V2 + 2,
        INODE_TABLE_SECTOR_V2 + 3,
        INODE_TABLE_SECTOR_V2 + 4,
        INODE_TABLE_SECTOR_V2 + 5,
        INODE_TABLE_SECTOR_V2 + 6,
        INODE_TABLE_SECTOR_V2 + 7,
        DIRECTORY_SECTOR_V2,
        DIRECTORY_SECTOR_V2 + 1,
        DIRECTORY_SECTOR_V2 + 2,
        DIRECTORY_SECTOR_V2 + 3,
        DIRECTORY_SECTOR_V2 + 4,
        DIRECTORY_SECTOR_V2 + 5,
        DIRECTORY_SECTOR_V2 + 6,
        DIRECTORY_SECTOR_V2 + 7,
        FREE_MAP_SECTOR_V2,
        JOURNAL_SECTOR_V2,
        DATA_SECTOR_V2,
        DATA_SECTOR_V2 + 1,
    ] {
        sector[32 + allocated as usize] = 1;
    }
    write_checksum(sector).expect("fixed VertexFS v2 free map checksum");
}

fn parse_superblock(bytes: &[u8]) -> Result<Superblock, String> {
    if bytes.len() != SECTOR_SIZE * SECTORS {
        return Err(format!(
            "VertexFS image size is {}; expected {}",
            bytes.len(),
            SECTOR_SIZE * SECTORS
        ));
    }
    let sector = sector(bytes, 0)?;
    if !sector.starts_with(SUPERBLOCK_MAGIC) {
        return Err("VertexFS superblock rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION {
        return Err("VertexFS superblock rejected: unsupported version".to_owned());
    }
    if read_u16(sector, 18)? != SECTOR_SIZE as u16 {
        return Err("VertexFS superblock rejected: unsupported sector size".to_owned());
    }
    if !checksum_valid(sector)? {
        return Err("VertexFS superblock rejected: checksum mismatch".to_owned());
    }
    if read_u32(sector, 24)? != SECTORS as u32 {
        return Err("VertexFS superblock rejected: unsupported sector count".to_owned());
    }
    let feature_flags = read_u32(sector, 28)?;
    if feature_flags != FEATURE_FLAGS {
        return Err("VertexFS superblock rejected: unsupported feature flags".to_owned());
    }
    verify_section(sector, 0, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
    verify_section(sector, 1, DIRECTORY_SECTOR, DIRECTORY_SECTORS)?;
    verify_section(sector, 2, FREE_MAP_SECTOR, 1)?;
    verify_section(sector, 3, JOURNAL_SECTOR, 1)?;
    verify_section(sector, 4, DATA_SECTOR, DATA_SECTORS)?;
    Ok(Superblock {
        generation: fixed_string_at(sector, GENERATION_OFFSET)?,
        feature_flags,
    })
}

fn parse_superblock_v2(bytes: &[u8]) -> Result<Superblock, String> {
    if bytes.len() != SECTOR_SIZE * SECTORS {
        return Err(format!(
            "VertexFS v2 image size is {}; expected {}",
            bytes.len(),
            SECTOR_SIZE * SECTORS
        ));
    }
    let sector = sector(bytes, 0)?;
    if !sector.starts_with(SUPERBLOCK_MAGIC_V2) {
        return Err("VertexFS v2 superblock rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION_V2 {
        return Err("VertexFS v2 superblock rejected: unsupported version".to_owned());
    }
    if read_u16(sector, 18)? != SECTOR_SIZE as u16 {
        return Err("VertexFS v2 superblock rejected: unsupported sector size".to_owned());
    }
    if !checksum_valid(sector)? {
        return Err("VertexFS v2 superblock rejected: checksum mismatch".to_owned());
    }
    if read_u32(sector, 24)? != SECTORS as u32 {
        return Err("VertexFS v2 superblock rejected: unsupported sector count".to_owned());
    }
    let feature_flags = read_u32(sector, 28)?;
    if feature_flags != FEATURE_FLAGS_V2 {
        return Err("VertexFS v2 superblock rejected: unsupported feature flags".to_owned());
    }
    let _volume = fixed_string_at(sector, V2_VOLUME_OFFSET)?;
    verify_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        0,
        INODE_TABLE_SECTOR_V2,
        INODE_TABLE_SECTORS_V2,
    )?;
    verify_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        1,
        DIRECTORY_SECTOR_V2,
        DIRECTORY_SECTORS_V2,
    )?;
    verify_section_at(sector, SECTION_TABLE_OFFSET_V2, 2, FREE_MAP_SECTOR_V2, 1)?;
    verify_section_at(sector, SECTION_TABLE_OFFSET_V2, 3, JOURNAL_SECTOR_V2, 1)?;
    verify_section_at(
        sector,
        SECTION_TABLE_OFFSET_V2,
        4,
        DATA_SECTOR_V2,
        DATA_SECTORS_V2,
    )?;
    Ok(Superblock {
        generation: fixed_string_at(sector, V2_GENERATION_OFFSET)?,
        feature_flags,
    })
}

fn parse_inode_table(bytes: &[u8]) -> Result<BTreeMap<u32, Inode>, String> {
    let sector = section(bytes, INODE_TABLE_SECTOR, INODE_TABLE_SECTORS)?;
    if !sector.starts_with(INODE_TABLE_MAGIC) {
        return Err("VertexFS inode table rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION || !checksum_valid(sector)? {
        return Err("VertexFS inode table rejected: metadata invalid".to_owned());
    }
    let count = read_u16(sector, 18)? as usize;
    if count > BASE_INODE_COUNT + DYNAMIC_FILE_CAPACITY {
        return Err("VertexFS inode table rejected: inode count exceeds section".to_owned());
    }
    let mut inodes = BTreeMap::new();
    for index in 0..count {
        let inode = read_inode(sector, index)?;
        if inodes.insert(inode.id, inode).is_some() {
            return Err("VertexFS inode table rejected: duplicate inode id".to_owned());
        }
    }
    let root = inodes
        .get(&INODE_ROOT)
        .ok_or_else(|| "VertexFS inode table rejected: missing root inode".to_owned())?;
    if root.kind != KIND_DIR || root.parent != 0 || root.name != "/" {
        return Err("VertexFS inode table rejected: invalid root inode".to_owned());
    }
    Ok(inodes)
}

fn parse_inode_table_v2(bytes: &[u8]) -> Result<BTreeMap<u32, Inode>, String> {
    let sector = section(bytes, INODE_TABLE_SECTOR_V2, INODE_TABLE_SECTORS_V2)?;
    if !sector.starts_with(INODE_TABLE_MAGIC_V2) {
        return Err("VertexFS v2 inode table rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION_V2 || !checksum_valid(sector)? {
        return Err("VertexFS v2 inode table rejected: metadata invalid".to_owned());
    }
    let count = read_u16(sector, 18)? as usize;
    if count > BASE_INODE_COUNT + DYNAMIC_FILE_CAPACITY_V2 || count > INODE_ENTRY_CAPACITY_V2 {
        return Err("VertexFS v2 inode table rejected: inode count exceeds section".to_owned());
    }
    let mut inodes = BTreeMap::new();
    for index in 0..count {
        let inode = read_inode_v2(sector, index)?;
        if inodes.insert(inode.id, inode).is_some() {
            return Err("VertexFS v2 inode table rejected: duplicate inode id".to_owned());
        }
    }
    let root = inodes
        .get(&INODE_ROOT)
        .ok_or_else(|| "VertexFS v2 inode table rejected: missing root inode".to_owned())?;
    if root.kind != KIND_DIR || root.parent != 0 || root.name != "/" {
        return Err("VertexFS v2 inode table rejected: invalid root inode".to_owned());
    }
    Ok(inodes)
}

fn parse_directory(bytes: &[u8]) -> Result<Vec<DirectoryEntry>, String> {
    let sector = section(bytes, DIRECTORY_SECTOR, DIRECTORY_SECTORS)?;
    if !sector.starts_with(DIRECTORY_MAGIC) {
        return Err("VertexFS directory block rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION || !checksum_valid(sector)? {
        return Err("VertexFS directory block rejected: metadata invalid".to_owned());
    }
    let count = read_u16(sector, 18)? as usize;
    if count > BASE_DIRECTORY_COUNT + DIRECTORY_DYNAMIC_FILE_CAPACITY {
        return Err("VertexFS directory block rejected: entry count exceeds section".to_owned());
    }
    let mut entries = Vec::new();
    for index in 0..count {
        entries.push(read_directory_entry(sector, index)?);
    }
    Ok(entries)
}

fn parse_directory_v2(bytes: &[u8]) -> Result<Vec<DirectoryEntry>, String> {
    let sector = section(bytes, DIRECTORY_SECTOR_V2, DIRECTORY_SECTORS_V2)?;
    if !sector.starts_with(DIRECTORY_MAGIC_V2) {
        return Err("VertexFS v2 directory block rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION_V2 || !checksum_valid(sector)? {
        return Err("VertexFS v2 directory block rejected: metadata invalid".to_owned());
    }
    let count = read_u16(sector, 18)? as usize;
    if count > BASE_DIRECTORY_COUNT + DYNAMIC_FILE_CAPACITY_V2
        || count > DIRECTORY_ENTRY_CAPACITY_V2
    {
        return Err("VertexFS v2 directory block rejected: entry count exceeds section".to_owned());
    }
    let mut entries = Vec::new();
    for index in 0..count {
        entries.push(read_directory_entry_v2(sector, index)?);
    }
    Ok(entries)
}

fn parse_journal(bytes: &[u8]) -> Result<Option<JournalRecord<'_>>, String> {
    let sector = sector(bytes, JOURNAL_SECTOR)?;
    if !sector.starts_with(JOURNAL_MAGIC) {
        return Err("VertexFS journal rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION || !checksum_valid(sector)? {
        return Err("VertexFS journal rejected: metadata invalid".to_owned());
    }
    let state = read_u16(sector, 18)?;
    if state == JOURNAL_STATE_CLEAN {
        return Ok(None);
    }
    if state != JOURNAL_STATE_PENDING {
        return Err("VertexFS journal rejected: unsupported state".to_owned());
    }
    let target_inode = read_u32(sector, 24)?;
    let payload_len = read_u32(sector, 28)? as usize;
    let payload_checksum = read_u32(sector, 32)?;
    let payload = sector
        .get(JOURNAL_PAYLOAD_OFFSET..JOURNAL_PAYLOAD_OFFSET + payload_len)
        .ok_or_else(|| "VertexFS journal rejected: payload out of bounds".to_owned())?;
    if checksum32(payload) != payload_checksum {
        return Err("VertexFS journal rejected: payload checksum mismatch".to_owned());
    }
    Ok(Some(JournalRecord {
        target_inode,
        payload,
    }))
}

fn parse_journal_v2(bytes: &[u8]) -> Result<Option<JournalRecord<'_>>, String> {
    let sector = sector(bytes, JOURNAL_SECTOR_V2)?;
    if !sector.starts_with(JOURNAL_MAGIC_V2) {
        return Err("VertexFS v2 journal rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION_V2 || !checksum_valid(sector)? {
        return Err("VertexFS v2 journal rejected: metadata invalid".to_owned());
    }
    let state = read_u16(sector, 18)?;
    if state == JOURNAL_STATE_CLEAN {
        return Ok(None);
    }
    if state != JOURNAL_STATE_PENDING {
        return Err("VertexFS v2 journal rejected: unsupported state".to_owned());
    }
    let target_inode = read_u32(sector, 24)?;
    let payload_len = read_u32(sector, 28)? as usize;
    let payload_checksum = read_u32(sector, 32)?;
    if read_u64(sector, 40)? == 0
        || read_u32(sector, 48)? != target_inode
        || read_u32(sector, 52)? as usize != payload_len
        || read_u32(sector, 56)? != payload_checksum
    {
        return Err("VertexFS v2 journal rejected: replay record mismatch".to_owned());
    }
    let payload = sector
        .get(JOURNAL_PAYLOAD_OFFSET..JOURNAL_PAYLOAD_OFFSET + payload_len)
        .ok_or_else(|| "VertexFS v2 journal rejected: payload out of bounds".to_owned())?;
    if checksum32(payload) != payload_checksum {
        return Err("VertexFS v2 journal rejected: payload checksum mismatch".to_owned());
    }
    Ok(Some(JournalRecord {
        target_inode,
        payload,
    }))
}

fn parse_free_map(bytes: &[u8]) -> Result<Vec<bool>, String> {
    let sector = sector(bytes, FREE_MAP_SECTOR)?;
    if !sector.starts_with(FREE_MAP_MAGIC) {
        return Err("VertexFS free-space map rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION || !checksum_valid(sector)? {
        return Err("VertexFS free-space map rejected: metadata invalid".to_owned());
    }
    if read_u16(sector, 18)? as usize != SECTORS {
        return Err("VertexFS free-space map rejected: sector count mismatch".to_owned());
    }
    let mut map = Vec::with_capacity(SECTORS);
    for index in 0..SECTORS {
        match sector[32 + index] {
            0 => map.push(false),
            1 => map.push(true),
            _ => {
                return Err("VertexFS free-space map rejected: invalid allocation byte".to_owned());
            }
        }
    }
    for required in [
        0,
        INODE_TABLE_SECTOR,
        INODE_TABLE_SECTOR + 1,
        DIRECTORY_SECTOR,
        DIRECTORY_SECTOR + 1,
        FREE_MAP_SECTOR,
        JOURNAL_SECTOR,
    ] {
        if !map[required as usize] {
            return Err("VertexFS free-space map rejected: metadata sector marked free".to_owned());
        }
    }
    Ok(map)
}

fn parse_free_map_v2(bytes: &[u8]) -> Result<Vec<bool>, String> {
    let sector = sector(bytes, FREE_MAP_SECTOR_V2)?;
    if !sector.starts_with(FREE_MAP_MAGIC_V2) {
        return Err("VertexFS v2 free-space map rejected: bad magic".to_owned());
    }
    if read_u16(sector, 16)? != VERSION_V2 || !checksum_valid(sector)? {
        return Err("VertexFS v2 free-space map rejected: metadata invalid".to_owned());
    }
    if read_u16(sector, 18)? as usize != SECTORS {
        return Err("VertexFS v2 free-space map rejected: sector count mismatch".to_owned());
    }
    let mut map = Vec::with_capacity(SECTORS);
    for index in 0..SECTORS {
        match sector[32 + index] {
            0 => map.push(false),
            1 => map.push(true),
            _ => {
                return Err(
                    "VertexFS v2 free-space map rejected: invalid allocation byte".to_owned(),
                );
            }
        }
    }
    for required in 0..=JOURNAL_SECTOR_V2 {
        if !map[required as usize] {
            return Err(
                "VertexFS v2 free-space map rejected: metadata sector marked free".to_owned(),
            );
        }
    }
    Ok(map)
}

fn verify_directory_graph(
    inodes: &BTreeMap<u32, Inode>,
    directory: &[DirectoryEntry],
) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for entry in directory {
        let parent = inodes
            .get(&entry.parent)
            .ok_or_else(|| "VertexFS directory block rejected: missing parent inode".to_owned())?;
        if parent.kind != KIND_DIR {
            return Err("VertexFS directory block rejected: parent is not directory".to_owned());
        }
        let child = inodes
            .get(&entry.child)
            .ok_or_else(|| "VertexFS directory block rejected: missing child inode".to_owned())?;
        if child.parent != entry.parent || child.name != entry.name || child.kind != entry.kind {
            return Err("VertexFS directory block rejected: child inode mismatch".to_owned());
        }
        if !names.insert((entry.parent, entry.name.clone())) {
            return Err("VertexFS directory block rejected: duplicate child name".to_owned());
        }
    }
    for inode in inodes.values() {
        if inode.id == INODE_ROOT {
            continue;
        }
        if !directory.iter().any(|entry| entry.child == inode.id) {
            return Err("VertexFS directory block rejected: orphan inode".to_owned());
        }
    }
    Ok(())
}

fn verify_journal(
    inodes: &BTreeMap<u32, Inode>,
    journal: Option<&JournalRecord<'_>>,
) -> Result<(), String> {
    let Some(journal) = journal else {
        return Ok(());
    };
    let inode = inodes
        .get(&journal.target_inode)
        .ok_or_else(|| "VertexFS journal rejected: missing target inode".to_owned())?;
    if inode.kind != KIND_FILE {
        return Err("VertexFS journal rejected: target is not a file".to_owned());
    }
    if journal.payload.len() > MAX_JOURNAL_FILE_BYTES {
        return Err("VertexFS journal rejected: payload too large".to_owned());
    }
    if journal.payload.len() as u64 > inode.sector_count as u64 * SECTOR_SIZE as u64 {
        return Err("VertexFS journal rejected: payload exceeds target extent".to_owned());
    }
    Ok(())
}

fn verify_file_extents(
    bytes: &[u8],
    inodes: &BTreeMap<u32, Inode>,
    free_map: &[bool],
    journal: Option<&JournalRecord<'_>>,
) -> Result<Vec<FileReport>, String> {
    let mut used = BTreeSet::new();
    let mut reports = Vec::new();
    for inode in inodes.values() {
        match inode.kind {
            KIND_DIR => {
                if inode.size != 0 || inode.first_sector != 0 || inode.sector_count != 0 {
                    return Err(
                        "VertexFS inode table rejected: directory has file extent".to_owned()
                    );
                }
            }
            KIND_FILE => {
                if inode.sector_count == 0 {
                    return Err("VertexFS inode table rejected: file has no extent".to_owned());
                }
                let end = inode
                    .first_sector
                    .checked_add(inode.sector_count as u64)
                    .ok_or_else(|| "VertexFS inode table rejected: extent overflow".to_owned())?;
                if inode.first_sector < DATA_SECTOR || end > DATA_SECTOR + DATA_SECTORS {
                    return Err(
                        "VertexFS inode table rejected: file extent outside data section"
                            .to_owned(),
                    );
                }
                if inode.size > inode.sector_count as u64 * SECTOR_SIZE as u64 {
                    return Err(
                        "VertexFS inode table rejected: file length exceeds extent".to_owned()
                    );
                }
                for sector_index in inode.first_sector..end {
                    if !used.insert(sector_index) {
                        return Err(
                            "VertexFS free-space verification rejected overlapping file extents"
                                .to_owned(),
                        );
                    }
                    if !free_map[sector_index as usize] {
                        return Err("VertexFS free-space verification rejected allocated extent marked free".to_owned());
                    }
                }
                let start = inode.first_sector as usize * SECTOR_SIZE;
                let len = usize::try_from(inode.size).map_err(|_| {
                    "VertexFS inode table rejected: file length overflow".to_owned()
                })?;
                let data = bytes.get(start..start + len).ok_or_else(|| {
                    "VertexFS inode table rejected: file data out of bounds".to_owned()
                })?;
                let replay_payload = journal
                    .filter(|record| record.target_inode == inode.id)
                    .map(|record| record.payload);
                if checksum32(data) != inode.checksum
                    && !replay_payload
                        .map(|payload| file_extent_starts_with(bytes, inode, payload))
                        .unwrap_or(false)
                {
                    return Err("VertexFS inode table rejected: file checksum mismatch".to_owned());
                }
                let report_payload = replay_payload.unwrap_or(data);
                reports.push(FileReport {
                    path: inode_path(inodes, inode.id)?,
                    len: report_payload.len() as u64,
                    checksum: checksum32(report_payload),
                    first_sector: inode.first_sector,
                    sector_count: inode.sector_count,
                });
            }
            _ => return Err("VertexFS inode table rejected: unsupported inode kind".to_owned()),
        }
    }
    reports.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(reports)
}

fn verify_file_extents_v2(
    bytes: &[u8],
    inodes: &BTreeMap<u32, Inode>,
    free_map: &[bool],
    journal: Option<&JournalRecord<'_>>,
) -> Result<Vec<FileReport>, String> {
    let mut used = BTreeSet::new();
    let mut reports = Vec::new();
    for inode in inodes.values() {
        match inode.kind {
            KIND_DIR => {
                if inode.size != 0 || inode.first_sector != 0 || inode.sector_count != 0 {
                    return Err(
                        "VertexFS v2 inode table rejected: directory has file extent".to_owned(),
                    );
                }
            }
            KIND_FILE => {
                if inode.sector_count == 0 {
                    return Err("VertexFS v2 inode table rejected: file has no extent".to_owned());
                }
                let end = inode
                    .first_sector
                    .checked_add(inode.sector_count as u64)
                    .ok_or_else(|| {
                        "VertexFS v2 inode table rejected: extent overflow".to_owned()
                    })?;
                if inode.first_sector < DATA_SECTOR_V2 || end > DATA_SECTOR_V2 + DATA_SECTORS_V2 {
                    return Err(
                        "VertexFS v2 inode table rejected: file extent outside data section"
                            .to_owned(),
                    );
                }
                if inode.size > inode.sector_count as u64 * SECTOR_SIZE as u64 {
                    return Err(
                        "VertexFS v2 inode table rejected: file length exceeds extent".to_owned(),
                    );
                }
                for sector_index in inode.first_sector..end {
                    if !used.insert(sector_index) {
                        return Err(
                            "VertexFS v2 free-space verification rejected overlapping file extents"
                                .to_owned(),
                        );
                    }
                    if !free_map[sector_index as usize] {
                        return Err("VertexFS v2 free-space verification rejected allocated extent marked free".to_owned());
                    }
                }
                let start = inode.first_sector as usize * SECTOR_SIZE;
                let len = usize::try_from(inode.size).map_err(|_| {
                    "VertexFS v2 inode table rejected: file length overflow".to_owned()
                })?;
                let data = bytes.get(start..start + len).ok_or_else(|| {
                    "VertexFS v2 inode table rejected: file data out of bounds".to_owned()
                })?;
                let replay_payload = journal
                    .filter(|record| record.target_inode == inode.id)
                    .map(|record| record.payload);
                if checksum32(data) != inode.checksum
                    && !replay_payload
                        .map(|payload| file_extent_starts_with(bytes, inode, payload))
                        .unwrap_or(false)
                {
                    return Err(
                        "VertexFS v2 inode table rejected: file checksum mismatch".to_owned()
                    );
                }
                let report_payload = replay_payload.unwrap_or(data);
                reports.push(FileReport {
                    path: inode_path(inodes, inode.id)?,
                    len: report_payload.len() as u64,
                    checksum: checksum32(report_payload),
                    first_sector: inode.first_sector,
                    sector_count: inode.sector_count,
                });
            }
            _ => return Err("VertexFS v2 inode table rejected: unsupported inode kind".to_owned()),
        }
    }
    reports.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(reports)
}

fn file_extent_starts_with(bytes: &[u8], inode: &Inode, payload: &[u8]) -> bool {
    let Some(start) = inode
        .first_sector
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    let Some(end) = start.checked_add(payload.len()) else {
        return false;
    };
    bytes
        .get(start..end)
        .map(|stored| stored == payload)
        .unwrap_or(false)
}

fn inode_path(inodes: &BTreeMap<u32, Inode>, id: u32) -> Result<String, String> {
    let inode = inodes
        .get(&id)
        .ok_or_else(|| "VertexFS path lookup rejected: missing inode".to_owned())?;
    if inode.id == INODE_ROOT {
        return Ok("/".to_owned());
    }
    let parent = inode_path(inodes, inode.parent)?;
    if parent == "/" {
        Ok(format!("/{name}", name = inode.name))
    } else {
        Ok(format!("{parent}/{name}", name = inode.name))
    }
}

fn inode_by_path<'a>(inodes: &'a BTreeMap<u32, Inode>, path: &str) -> Result<&'a Inode, String> {
    for inode in inodes.values() {
        if inode_path(inodes, inode.id)? == path {
            return Ok(inode);
        }
    }
    Err("VertexFS path lookup rejected: missing path".to_owned())
}

struct Superblock {
    generation: String,
    feature_flags: u32,
}

fn write_inode(sector: &mut [u8], index: usize, inode: &Inode) -> Result<(), String> {
    let offset = inode_offset(index)?;
    write_u32(sector, offset, inode.id)?;
    write_u16(sector, offset + 4, inode.kind)?;
    write_u16(sector, offset + 6, 0)?;
    write_u64(sector, offset + 8, inode.size)?;
    write_u64(sector, offset + 16, inode.first_sector)?;
    write_u32(sector, offset + 24, inode.sector_count)?;
    write_u32(sector, offset + 28, inode.checksum)?;
    write_u32(sector, offset + 32, inode.parent)?;
    write_fixed_str_28(sector, offset + 36, &inode.name)
}

fn write_inode_v2(sector: &mut [u8], index: usize, inode: &Inode) -> Result<(), String> {
    let offset = inode_offset_v2(index)?;
    write_u32(sector, offset, inode.id)?;
    write_u16(sector, offset + 4, inode.kind)?;
    write_u16(sector, offset + 6, 0)?;
    write_u64(sector, offset + 8, inode.size)?;
    write_u64(sector, offset + 16, inode.first_sector)?;
    write_u32(sector, offset + 24, inode.sector_count)?;
    write_u32(sector, offset + 28, inode.checksum)?;
    write_u32(sector, offset + 32, inode.parent)?;
    write_fixed_str_28(sector, offset + 36, &inode.name)
}

fn read_inode(sector: &[u8], index: usize) -> Result<Inode, String> {
    let offset = inode_offset(index)?;
    Ok(Inode {
        id: read_u32(sector, offset)?,
        kind: read_u16(sector, offset + 4)?,
        size: read_u64(sector, offset + 8)?,
        first_sector: read_u64(sector, offset + 16)?,
        sector_count: read_u32(sector, offset + 24)?,
        checksum: read_u32(sector, offset + 28)?,
        parent: read_u32(sector, offset + 32)?,
        name: fixed_string_at_len(sector, offset + 36, 28)?,
    })
}

fn read_inode_v2(sector: &[u8], index: usize) -> Result<Inode, String> {
    let offset = inode_offset_v2(index)?;
    Ok(Inode {
        id: read_u32(sector, offset)?,
        kind: read_u16(sector, offset + 4)?,
        size: read_u64(sector, offset + 8)?,
        first_sector: read_u64(sector, offset + 16)?,
        sector_count: read_u32(sector, offset + 24)?,
        checksum: read_u32(sector, offset + 28)?,
        parent: read_u32(sector, offset + 32)?,
        name: fixed_string_at_len(sector, offset + 36, 28)?,
    })
}

fn write_directory_entry(
    sector: &mut [u8],
    index: usize,
    entry: &DirectoryEntry,
) -> Result<(), String> {
    let offset = directory_offset(index)?;
    write_u32(sector, offset, entry.parent)?;
    write_u32(sector, offset + 4, entry.child)?;
    write_u16(sector, offset + 8, entry.kind)?;
    write_u16(sector, offset + 10, 0)?;
    write_fixed_str_len(sector, offset + 12, DIRECTORY_NAME_BYTES, &entry.name)
}

fn write_directory_entry_v2(
    sector: &mut [u8],
    index: usize,
    entry: &DirectoryEntry,
) -> Result<(), String> {
    let offset = directory_offset_v2(index)?;
    write_u32(sector, offset, entry.parent)?;
    write_u32(sector, offset + 4, entry.child)?;
    write_u16(sector, offset + 8, entry.kind)?;
    write_u16(sector, offset + 10, 0)?;
    write_fixed_str_len(sector, offset + 12, DIRECTORY_NAME_BYTES, &entry.name)
}

fn read_directory_entry(sector: &[u8], index: usize) -> Result<DirectoryEntry, String> {
    let offset = directory_offset(index)?;
    Ok(DirectoryEntry {
        parent: read_u32(sector, offset)?,
        child: read_u32(sector, offset + 4)?,
        kind: read_u16(sector, offset + 8)?,
        name: fixed_string_at_len(sector, offset + 12, DIRECTORY_NAME_BYTES)?,
    })
}

fn read_directory_entry_v2(sector: &[u8], index: usize) -> Result<DirectoryEntry, String> {
    let offset = directory_offset_v2(index)?;
    Ok(DirectoryEntry {
        parent: read_u32(sector, offset)?,
        child: read_u32(sector, offset + 4)?,
        kind: read_u16(sector, offset + 8)?,
        name: fixed_string_at_len(sector, offset + 12, DIRECTORY_NAME_BYTES)?,
    })
}

fn inode_offset(index: usize) -> Result<usize, String> {
    let offset = INODE_ENTRY_OFFSET + index * INODE_ENTRY_LEN;
    if offset + INODE_ENTRY_LEN > INODE_TABLE_BYTES {
        return Err("VertexFS inode table rejected: entry bounds invalid".to_owned());
    }
    Ok(offset)
}

fn inode_offset_v2(index: usize) -> Result<usize, String> {
    let offset = INODE_ENTRY_OFFSET + index * INODE_ENTRY_LEN;
    if offset + INODE_ENTRY_LEN > INODE_TABLE_BYTES_V2 {
        return Err("VertexFS v2 inode table rejected: entry bounds invalid".to_owned());
    }
    Ok(offset)
}

fn inode_offset_by_id(sector: &[u8], id: u32) -> Result<usize, String> {
    let count = read_u16(sector, 18)? as usize;
    for index in 0..count {
        let offset = inode_offset(index)?;
        if read_u32(sector, offset)? == id {
            return Ok(offset);
        }
    }
    Err("VertexFS inode table rejected: missing inode id".to_owned())
}

fn inode_offset_by_id_v2(sector: &[u8], id: u32) -> Result<usize, String> {
    let count = read_u16(sector, 18)? as usize;
    for index in 0..count {
        let offset = inode_offset_v2(index)?;
        if read_u32(sector, offset)? == id {
            return Ok(offset);
        }
    }
    Err("VertexFS v2 inode table rejected: missing inode id".to_owned())
}

fn directory_offset(index: usize) -> Result<usize, String> {
    let offset = DIRECTORY_ENTRY_OFFSET + index * DIRECTORY_ENTRY_LEN;
    if offset + DIRECTORY_ENTRY_LEN > DIRECTORY_BYTES {
        return Err("VertexFS directory block rejected: entry bounds invalid".to_owned());
    }
    Ok(offset)
}

fn directory_offset_v2(index: usize) -> Result<usize, String> {
    let offset = DIRECTORY_ENTRY_OFFSET + index * DIRECTORY_ENTRY_LEN;
    if offset + DIRECTORY_ENTRY_LEN > DIRECTORY_BYTES_V2 {
        return Err("VertexFS v2 directory block rejected: entry bounds invalid".to_owned());
    }
    Ok(offset)
}

fn write_section(
    sector: &mut [u8],
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> Result<(), String> {
    let offset = SECTION_TABLE_OFFSET + index * SECTION_RECORD_LEN;
    write_u64(sector, offset, first_sector)?;
    write_u64(sector, offset + 8, sector_count)
}

fn write_section_at(
    sector: &mut [u8],
    table_offset: usize,
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> Result<(), String> {
    let offset = table_offset + index * SECTION_RECORD_LEN;
    write_u64(sector, offset, first_sector)?;
    write_u64(sector, offset + 8, sector_count)
}

fn verify_section(
    sector: &[u8],
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> Result<(), String> {
    let offset = SECTION_TABLE_OFFSET + index * SECTION_RECORD_LEN;
    if read_u64(sector, offset)? != first_sector || read_u64(sector, offset + 8)? != sector_count {
        return Err("VertexFS superblock rejected: unsupported section layout".to_owned());
    }
    Ok(())
}

fn verify_section_at(
    sector: &[u8],
    table_offset: usize,
    index: usize,
    first_sector: u64,
    sector_count: u64,
) -> Result<(), String> {
    let offset = table_offset + index * SECTION_RECORD_LEN;
    if read_u64(sector, offset)? != first_sector || read_u64(sector, offset + 8)? != sector_count {
        return Err("VertexFS v2 superblock rejected: unsupported section layout".to_owned());
    }
    Ok(())
}

fn write_bytes(image: &mut [u8], start_sector: u64, bytes: &[u8]) {
    let offset = start_sector as usize * SECTOR_SIZE;
    image[offset..offset + bytes.len()].copy_from_slice(bytes);
}

fn sector(image: &[u8], index: u64) -> Result<&[u8], String> {
    let offset = index
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS sector offset overflow".to_owned())?;
    image
        .get(offset..offset + SECTOR_SIZE)
        .ok_or_else(|| "VertexFS sector out of bounds".to_owned())
}

fn sector_mut(image: &mut [u8], index: u64) -> Result<&mut [u8], String> {
    let offset = index
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS sector offset overflow".to_owned())?;
    image
        .get_mut(offset..offset + SECTOR_SIZE)
        .ok_or_else(|| "VertexFS sector out of bounds".to_owned())
}

fn section(image: &[u8], first_sector: u64, sector_count: u64) -> Result<&[u8], String> {
    let start = first_sector
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS section offset overflow".to_owned())?;
    let len = sector_count
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS section length overflow".to_owned())?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "VertexFS section end overflow".to_owned())?;
    image
        .get(start..end)
        .ok_or_else(|| "VertexFS section out of bounds".to_owned())
}

fn section_mut(
    image: &mut [u8],
    first_sector: u64,
    sector_count: u64,
) -> Result<&mut [u8], String> {
    let start = first_sector
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS section offset overflow".to_owned())?;
    let len = sector_count
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "VertexFS section length overflow".to_owned())?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "VertexFS section end overflow".to_owned())?;
    image
        .get_mut(start..end)
        .ok_or_else(|| "VertexFS section out of bounds".to_owned())
}

fn write_checksum(bytes: &mut [u8]) -> Result<(), String> {
    write_u32(bytes, CHECKSUM_OFFSET, 0)?;
    let checksum = checksum32(bytes);
    write_u32(bytes, CHECKSUM_OFFSET, checksum)
}

fn checksum_valid(bytes: &[u8]) -> Result<bool, String> {
    let stored = read_u32(bytes, CHECKSUM_OFFSET)?;
    let mut scratch = bytes.to_vec();
    write_u32(&mut scratch, CHECKSUM_OFFSET, 0)?;
    Ok(checksum32(&scratch) == stored)
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (index, byte) in bytes.iter().enumerate() {
        checksum = checksum.wrapping_add((*byte as u32).wrapping_mul(index as u32 + 1));
    }
    checksum
}

fn write_fixed_str(buffer: &mut [u8], offset: usize, value: &str) -> Result<(), String> {
    write_fixed_str_len(buffer, offset, 64, value)
}

fn write_fixed_str_28(buffer: &mut [u8], offset: usize, value: &str) -> Result<(), String> {
    write_fixed_str_len(buffer, offset, 28, value)
}

fn write_fixed_str_len(
    buffer: &mut [u8],
    offset: usize,
    max_len: usize,
    value: &str,
) -> Result<(), String> {
    if value.is_empty() || value.len() > max_len || offset + max_len > buffer.len() {
        return Err("VertexFS fixed string bounds invalid".to_owned());
    }
    buffer[offset..offset + max_len].fill(0);
    buffer[offset..offset + value.len()].copy_from_slice(value.as_bytes());
    Ok(())
}

fn fixed_string_at(buffer: &[u8], offset: usize) -> Result<String, String> {
    fixed_string_at_len(buffer, offset, 64)
}

fn fixed_string_at_len(buffer: &[u8], offset: usize, max_len: usize) -> Result<String, String> {
    if offset + max_len > buffer.len() {
        return Err("VertexFS fixed string bounds invalid".to_owned());
    }
    let mut len = 0;
    while len < max_len && buffer[offset + len] != 0 {
        len += 1;
    }
    if len == 0 {
        return Err("VertexFS fixed string is empty".to_owned());
    }
    core::str::from_utf8(&buffer[offset..offset + len])
        .map(|value| value.to_owned())
        .map_err(|_| "VertexFS fixed string is not UTF-8".to_owned())
}

fn read_u16(buffer: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = buffer
        .get(offset..offset + 2)
        .ok_or_else(|| "VertexFS u16 read out of bounds".to_owned())?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(buffer: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = buffer
        .get(offset..offset + 4)
        .ok_or_else(|| "VertexFS u32 read out of bounds".to_owned())?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(buffer: &[u8], offset: usize) -> Result<u64, String> {
    let bytes = buffer
        .get(offset..offset + 8)
        .ok_or_else(|| "VertexFS u64 read out of bounds".to_owned())?;
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) -> Result<(), String> {
    let target = buffer
        .get_mut(offset..offset + 2)
        .ok_or_else(|| "VertexFS u16 write out of bounds".to_owned())?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) -> Result<(), String> {
    let target = buffer
        .get_mut(offset..offset + 4)
        .ok_or_else(|| "VertexFS u32 write out of bounds".to_owned())?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) -> Result<(), String> {
    let target = buffer
        .get_mut(offset..offset + 8)
        .ok_or_else(|| "VertexFS u64 write out of bounds".to_owned())?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
