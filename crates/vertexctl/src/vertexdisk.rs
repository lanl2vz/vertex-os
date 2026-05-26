const SECTOR_SIZE: usize = 512;
const SECTORS: usize = 64;
const SUPERBLOCK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
const STATE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTATEV0\0\0\0\0";
const VERSION: u16 = 1;
const CHECKSUM_OFFSET: usize = 20;
const SECTION_TABLE_OFFSET: usize = 32;
const SECTION_RECORD_LEN: usize = 16;
const STORE_ENTRY_OFFSET: usize = 32;
const STATE_ENTRY_OFFSET: usize = 32;
const GENERATION_METADATA_SECTOR: u64 = 1;
const STORE_INDEX_SECTOR: u64 = 2;
const STATE_INDEX_SECTOR: u64 = 3;
const STORE_DATA_SECTOR: u64 = 8;
const STATE_DATA_SECTOR: u64 = 16;
const JOURNAL_SECTOR: u64 = 32;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const HELLO_OBJECT_ID: &str = "store:hello-text";
const STATE_VOLUME_ID: &str = "state:counter";

pub fn create_image() -> Vec<u8> {
    let mut image = vec![0u8; SECTOR_SIZE * SECTORS];
    write_superblock(&mut image);
    write_store_index(&mut image);
    write_state_index(&mut image, 0, 0);
    write_sector_bytes(&mut image, STORE_DATA_SECTOR, HELLO_OBJECT);
    image
}

pub fn corrupt(bytes: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let mut out = bytes.to_vec();
    match mode {
        "bad-superblock" => {
            let first = out
                .first_mut()
                .ok_or_else(|| "cannot corrupt empty VertexDisk image".to_owned())?;
            *first = b'X';
        }
        other => {
            return Err(format!(
                "unknown VertexDisk corruption mode {other}; expected bad-superblock"
            ));
        }
    }
    Ok(out)
}

pub fn sector_size() -> usize {
    SECTOR_SIZE
}

pub fn sectors() -> usize {
    SECTORS
}

fn write_superblock(image: &mut [u8]) {
    let sector = sector_mut(image, 0);
    sector[..SUPERBLOCK_MAGIC.len()].copy_from_slice(SUPERBLOCK_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, SECTOR_SIZE as u16);
    write_u32(sector, 24, SECTORS as u32);
    write_section(sector, 0, GENERATION_METADATA_SECTOR, 1);
    write_section(sector, 1, STORE_INDEX_SECTOR, 1);
    write_section(sector, 2, STORE_DATA_SECTOR, 8);
    write_section(sector, 3, STATE_INDEX_SECTOR, 1);
    write_section(sector, 4, STATE_DATA_SECTOR, 8);
    write_section(sector, 5, JOURNAL_SECTOR, 16);
    write_checksum(sector);
}

fn write_store_index(image: &mut [u8]) {
    let sector = sector_mut(image, STORE_INDEX_SECTOR);
    sector[..STORE_INDEX_MAGIC.len()].copy_from_slice(STORE_INDEX_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, 1);
    write_fixed_str(sector, STORE_ENTRY_OFFSET, HELLO_OBJECT_ID);
    write_u64(sector, STORE_ENTRY_OFFSET + 64, STORE_DATA_SECTOR);
    write_u32(sector, STORE_ENTRY_OFFSET + 72, HELLO_OBJECT.len() as u32);
    write_u32(sector, STORE_ENTRY_OFFSET + 76, checksum32(HELLO_OBJECT));
    write_checksum(sector);
}

fn write_state_index(image: &mut [u8], value_len: u32, value_checksum: u32) {
    let sector = sector_mut(image, STATE_INDEX_SECTOR);
    sector[..STATE_INDEX_MAGIC.len()].copy_from_slice(STATE_INDEX_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, 1);
    write_fixed_str(sector, STATE_ENTRY_OFFSET, STATE_VOLUME_ID);
    write_u64(sector, STATE_ENTRY_OFFSET + 64, STATE_DATA_SECTOR);
    write_u32(sector, STATE_ENTRY_OFFSET + 72, 1);
    write_u32(sector, STATE_ENTRY_OFFSET + 76, value_len);
    write_u32(sector, STATE_ENTRY_OFFSET + 80, value_checksum);
    write_checksum(sector);
}

fn write_sector_bytes(image: &mut [u8], sector: u64, bytes: &[u8]) {
    let sector = sector_mut(image, sector);
    sector[..bytes.len()].copy_from_slice(bytes);
}

fn sector_mut(image: &mut [u8], sector: u64) -> &mut [u8] {
    let offset = sector as usize * SECTOR_SIZE;
    &mut image[offset..offset + SECTOR_SIZE]
}

fn write_section(sector: &mut [u8], index: usize, first_sector: u64, sector_count: u64) {
    let offset = SECTION_TABLE_OFFSET + index * SECTION_RECORD_LEN;
    write_u64(sector, offset, first_sector);
    write_u64(sector, offset + 8, sector_count);
}

fn write_fixed_str(buffer: &mut [u8], offset: usize, value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(64);
    buffer[offset..offset + len].copy_from_slice(&bytes[..len]);
}

fn write_checksum(sector: &mut [u8]) {
    write_u32(sector, CHECKSUM_OFFSET, 0);
    let checksum = checksum32(sector);
    write_u32(sector, CHECKSUM_OFFSET, checksum);
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (index, byte) in bytes.iter().enumerate() {
        checksum = checksum.wrapping_add((*byte as u32).wrapping_mul(index as u32 + 1));
    }
    checksum
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
