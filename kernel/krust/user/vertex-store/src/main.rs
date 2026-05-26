#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_BLOCK_REPLY: u64 = 3;
const CAP_BLOCK_REQUEST: u64 = 4;
const CAP_MODEL_REPLY: u64 = 5;
const CAP_INIT_REPLY: u64 = 6;
const STORE_ID: &[u8] = b"store:hello-text";
const GENERATION_B_MANIFEST_ID: &[u8] = b"store:generation-b-manifest";
const GENERATION_B_MANIFEST: &[u8] = b"krustboot:gen:switch-b-0002";
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_READ_SECTOR: u16 = 1;
const SECTOR_SIZE: usize = 512;
const VERTEX_DISK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
const VERTEX_DISK_VERSION: u16 = 1;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = 20;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = 24;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = 32;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = 16;
const VERTEX_DISK_STORE_INDEX_SECTION: usize = 1;
const VERTEX_DISK_STORE_DATA_SECTION: usize = 2;
const STORE_ENTRY_OFFSET: usize = 32;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let entry = load_store_entry();
    send_ready();
    log(b"vertex-store ready");

    loop {
        let mut request = [0u8; 64];
        let received = sys::ipc_recv(CAP_STORE_REQUEST, &mut request);
        if received > request.len() as u64 {
            log(b"vertex-store request invalid");
            sys::exit(1);
        }

        let request = &request[..received as usize];
        if starts_with(request, GENERATION_B_MANIFEST_ID) {
            serve_generation_b_manifest();
            continue;
        }
        if starts_with(request, STORE_ID) {
            serve_hello_object(entry);
        }

        log(b"vertex-store request invalid");
        sys::exit(1);
    }
}

fn serve_generation_b_manifest() {
    log(b"vertex-store exposes generation B manifest");
    if sys::ipc_send(CAP_INIT_REPLY, GENERATION_B_MANIFEST) != sys::STATUS_OK {
        log(b"vertex-store generation manifest response failed");
        sys::exit(1);
    }
}

#[derive(Clone, Copy)]
struct StoreEntry {
    data_sector: u64,
    byte_len: usize,
    checksum: u32,
}

fn load_store_entry() -> StoreEntry {
    let mut superblock = [0u8; SECTOR_SIZE];
    read_block_sector(0, &mut superblock);
    if !valid_superblock(&superblock) {
        log(b"VertexDisk superblock rejected");
        sys::exit(1);
    }

    let Some(store_index_sector) =
        vertexdisk_section_start(&superblock, VERTEX_DISK_STORE_INDEX_SECTION)
    else {
        log(b"vertex-store store index bounds invalid");
        sys::exit(1);
    };
    let Some((store_data_start, store_data_count)) =
        vertexdisk_section(&superblock, VERTEX_DISK_STORE_DATA_SECTION)
    else {
        log(b"vertex-store store data bounds invalid");
        sys::exit(1);
    };

    let mut index_sector = [0u8; SECTOR_SIZE];
    read_block_sector(store_index_sector, &mut index_sector);
    if !valid_index_header(&index_sector, STORE_INDEX_MAGIC) {
        log(b"vertex-store object index rejected");
        sys::exit(1);
    }

    let count = read_u16(&index_sector, 18) as usize;
    let mut index = 0;
    while index < count {
        let offset = STORE_ENTRY_OFFSET + index * 80;
        if offset + 80 > index_sector.len() {
            log(b"vertex-store object index bounds invalid");
            sys::exit(1);
        }
        if fixed_string_eq(&index_sector, offset, STORE_ID) {
            let data_sector = read_u64(&index_sector, offset + 64);
            let byte_len = read_u32(&index_sector, offset + 72) as usize;
            let checksum = read_u32(&index_sector, offset + 76);
            if byte_len > SECTOR_SIZE
                || data_sector < store_data_start
                || data_sector >= store_data_start + store_data_count
            {
                log(b"vertex-store object bounds invalid");
                sys::exit(1);
            }
            log(b"vertex-store reads object index from disk");
            return StoreEntry {
                data_sector,
                byte_len,
                checksum,
            };
        }
        index += 1;
    }

    log(b"vertex-store object missing from disk index");
    sys::exit(1);
}

fn serve_hello_object(entry: StoreEntry) -> ! {
    log(b"store-service requests block read");
    let mut sector = [0u8; SECTOR_SIZE];
    read_block_sector(entry.data_sector, &mut sector);
    let object_len = entry.byte_len;
    if checksum32(&sector[..object_len]) != entry.checksum {
        log(b"vertex-store disk checksum failed");
        sys::exit(1);
    }
    if !bytes_eq(&sector[..object_len], HELLO_OBJECT) {
        log(b"vertex-store hash verification failed");
        sys::exit(1);
    }
    log(b"vertex-store verifies hash");

    sector[0] ^= 1;
    if !bytes_eq(&sector[..object_len], HELLO_OBJECT) {
        log(b"modified object fails hash check");
    } else {
        log(b"vertex-store modified-object negative failed");
        sys::exit(1);
    }
    sector[0] ^= 1;

    if sys::ipc_send(CAP_MODEL_REPLY, &sector[..object_len]) != sys::STATUS_OK {
        log(b"vertex-store response failed");
        sys::exit(1);
    }
    log(b"Native immutable store service ok");
    sys::exit(0)
}

fn block_read_request(sector: u64) -> [u8; 16] {
    let mut request = [0u8; 16];
    write_u16(&mut request, 0, BLOCK_PROTOCOL_V1);
    write_u16(&mut request, 2, BLOCK_OP_READ_SECTOR);
    write_u16(&mut request, 4, 0);
    write_u64(&mut request, 8, sector);
    request
}

fn read_block_sector(sector: u64, out: &mut [u8; SECTOR_SIZE]) {
    let request = block_read_request(sector);
    if sys::ipc_send(CAP_BLOCK_REQUEST, &request) != sys::STATUS_OK {
        log(b"vertex-store block request failed");
        sys::exit(1);
    }

    let received = sys::ipc_recv(CAP_BLOCK_REPLY, out);
    if received != SECTOR_SIZE as u64 {
        log(b"vertex-store block response failed");
        sys::exit(1);
    }
}

fn valid_superblock(sector: &[u8; SECTOR_SIZE]) -> bool {
    if !starts_with(sector, VERTEX_DISK_MAGIC)
        || read_u16(sector, 16) != VERTEX_DISK_VERSION
        || read_u16(sector, 18) != SECTOR_SIZE as u16
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

fn valid_index_header(sector: &[u8; SECTOR_SIZE], magic: &[u8; 16]) -> bool {
    starts_with(sector, magic)
        && read_u16(sector, 16) == VERTEX_DISK_VERSION
        && metadata_checksum_valid(sector)
}

fn vertexdisk_section_start(sector: &[u8; SECTOR_SIZE], section: usize) -> Option<u64> {
    vertexdisk_section(sector, section).map(|(start, _count)| start)
}

fn vertexdisk_section(sector: &[u8; SECTOR_SIZE], section: usize) -> Option<(u64, u64)> {
    let offset = VERTEX_DISK_SECTION_TABLE_OFFSET + section * VERTEX_DISK_SECTION_RECORD_LEN;
    if offset + 16 > sector.len() {
        return None;
    }
    Some((read_u64(sector, offset), read_u64(sector, offset + 8)))
}

fn metadata_checksum_valid(sector: &[u8; SECTOR_SIZE]) -> bool {
    let stored = read_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET);
    let mut checksum = 0u32;
    let mut index = 0;
    while index < sector.len() {
        let byte =
            if index >= VERTEX_DISK_CHECKSUM_OFFSET && index < VERTEX_DISK_CHECKSUM_OFFSET + 4 {
                0
            } else {
                sector[index]
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
    if value.len() < 64 && buffer[offset + value.len()] != 0 {
        return false;
    }
    true
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"vertex-store");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"vertex-store ready send failed");
        sys::exit(1);
    }
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    write_u16(&mut message, 0, PROTOCOL_HEALTH_V0);
    write_u16(&mut message, 2, MESSAGE_READY);
    write_u32(&mut message, 4, service.len() as u32);
    write_u64(&mut message, 8, 1);
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

fn read_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buffer[offset], buffer[offset + 1]])
}

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
    ])
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
    buffer[offset + 2] = bytes[2];
    buffer[offset + 3] = bytes[3];
}

fn read_u64(buffer: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
        buffer[offset + 4],
        buffer[offset + 5],
        buffer[offset + 6],
        buffer[offset + 7],
    ])
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    bytes_eq(&value[..prefix.len()], prefix)
}

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
