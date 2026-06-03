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
const CAP_LOGD_EXECUTABLE: u64 = 7;
const CAP_ECHO_EXECUTABLE: u64 = 8;
const STORE_ID: &[u8] = b"store:hello-text";
const GENERATION_B_MANIFEST_ID: &[u8] = b"store:generation-b-manifest";
const GENERATION_B_MANIFEST: &[u8] = b"krustboot:gen:switch-b-0002";
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_READ_SECTOR: u16 = 1;
const SECTOR_SIZE: usize = 512;
const VERTEX_DISK_MAGIC: &[u8; 16] = b"VERTEXDISKV1\0\0\0\0";
const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
const VERTEX_DISK_VERSION: u16 = 2;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = 20;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = 24;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = 32;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = 16;
const VERTEX_DISK_STORE_INDEX_SECTION: usize = 1;
const VERTEX_DISK_STORE_DATA_SECTION: usize = 2;
const VERTEX_DISK_VERTEXFS_SECTION: usize = 6;
const MAX_STORE_INDEX_SECTORS: usize = 16;
const STORE_INDEX_BYTES: usize = MAX_STORE_INDEX_SECTORS * SECTOR_SIZE;
const STORE_ENTRY_OFFSET: usize = 32;
const STORE_ENTRY_LEN: usize = 144;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const LOGD_OBJECT_ID: &[u8] = b"store:logd-demo";
const ECHO_OBJECT_ID: &[u8] = b"store:echo-server-demo";
const MAX_EXECUTABLE_OBJECT_BYTES: usize = 1024 * 1024;

struct Global<T>(core::cell::UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static EXECUTABLE_OBJECT_BUFFER: Global<[u8; MAX_EXECUTABLE_OBJECT_BYTES]> = Global(
    core::cell::UnsafeCell::new([0; MAX_EXECUTABLE_OBJECT_BYTES]),
);

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let entries = load_store_entries();
    send_ready();
    log(b"vertex-store ready");
    if let Some(entry) = entries.logd {
        verify_store_object(entry, CAP_LOGD_EXECUTABLE, LOGD_OBJECT_ID, b"logd");
    }
    if let Some(entry) = entries.echo {
        verify_store_object(entry, CAP_ECHO_EXECUTABLE, ECHO_OBJECT_ID, b"echo");
    }

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
            serve_hello_object(entries.hello);
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
    hash: [u8; 64],
}

#[derive(Clone, Copy)]
struct StoreEntries {
    hello: StoreEntry,
    logd: Option<StoreEntry>,
    echo: Option<StoreEntry>,
}

fn load_store_entries() -> StoreEntries {
    let mut superblock = [0u8; SECTOR_SIZE];
    read_block_sector(0, &mut superblock);
    if !valid_superblock(&superblock) {
        log(b"VertexDisk superblock rejected");
        sys::exit(1);
    }

    let Some((store_index_start, store_index_count)) =
        vertexdisk_section(&superblock, VERTEX_DISK_STORE_INDEX_SECTION)
    else {
        log(b"vertex-store store index bounds invalid");
        sys::exit(1);
    };
    if store_index_count == 0 || store_index_count as usize > MAX_STORE_INDEX_SECTORS {
        log(b"vertex-store store index bounds invalid");
        sys::exit(1);
    }
    let Some((store_data_start, store_data_count)) =
        vertexdisk_section(&superblock, VERTEX_DISK_STORE_DATA_SECTION)
    else {
        log(b"vertex-store store data bounds invalid");
        sys::exit(1);
    };

    let mut index_bytes = [0u8; STORE_INDEX_BYTES];
    read_block_sectors(
        store_index_start,
        store_index_count as usize,
        &mut index_bytes,
    );
    let index_bytes = &index_bytes[..store_index_count as usize * SECTOR_SIZE];
    if !valid_index_header(index_bytes, STORE_INDEX_MAGIC) {
        log(b"vertex-store object index rejected");
        sys::exit(1);
    }

    let count = read_u16(index_bytes, 18) as usize;
    let mut hello = None;
    let mut logd = None;
    let mut echo = None;
    let mut index = 0;
    while index < count {
        let offset = STORE_ENTRY_OFFSET + index * STORE_ENTRY_LEN;
        if offset + STORE_ENTRY_LEN > index_bytes.len() {
            log(b"vertex-store object index bounds invalid");
            sys::exit(1);
        }
        let data_sector = read_u64(index_bytes, offset + 64);
        let byte_len = read_u32(index_bytes, offset + 72) as usize;
        let checksum = read_u32(index_bytes, offset + 76);
        let hash = fixed_hash(index_bytes, offset + 80);
        let entry = StoreEntry {
            data_sector,
            byte_len,
            checksum,
            hash,
        };
        if !store_entry_bounds_valid(entry, store_data_start, store_data_count) {
            log(b"vertex-store object bounds invalid");
            sys::exit(1);
        }
        if fixed_string_eq(index_bytes, offset, LOGD_OBJECT_ID) {
            logd = Some(entry);
        }
        if fixed_string_eq(index_bytes, offset, ECHO_OBJECT_ID) {
            echo = Some(entry);
        }
        if fixed_string_eq(index_bytes, offset, STORE_ID) {
            hello = Some(entry);
        }
        index += 1;
    }

    if let Some(entry) = hello {
        if entry.byte_len > SECTOR_SIZE {
            log(b"vertex-store object bounds invalid");
            sys::exit(1);
        }
        log(b"vertex-store reads object index from disk");
        return StoreEntries {
            hello: entry,
            logd,
            echo,
        };
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
        log(b"vertex-store hash mismatch security event: object=store:hello-text");
        log(b"vertex-inspect security event: store hash mismatch object=store:hello-text");
        sys::exit(1);
    }
    if !hash_matches(&sector[..object_len], &entry.hash) {
        log(b"vertex-store hash mismatch security event: object=store:hello-text");
        log(b"vertex-inspect security event: store hash mismatch object=store:hello-text");
        sys::exit(1);
    }
    if !bytes_eq(&sector[..object_len], HELLO_OBJECT) {
        log(b"vertex-store hash verification failed");
        sys::exit(1);
    }
    log(b"vertex-store verifies hash");
    log(b"immutable store object served read-only");

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

fn verify_store_object(entry: StoreEntry, cap_slot: u64, object_id: &[u8], name: &[u8]) {
    if entry.byte_len > MAX_EXECUTABLE_OBJECT_BYTES {
        log_prefix(b"vertex-store executable object too large: ", object_id);
        sys::exit(1);
    }

    let buffer = executable_object_buffer();
    let handle = sys::vfs_open_read(cap_slot);
    if handle == sys::STATUS_BAD_CAPABILITY {
        log_prefix(b"vertex-store executable VFS open failed: ", object_id);
        sys::exit(1);
    }
    let read = sys::vfs_read(handle, &mut buffer[..entry.byte_len]);
    if read != entry.byte_len as u64
        || checksum32(&buffer[..entry.byte_len]) != entry.checksum
        || !hash_matches(&buffer[..entry.byte_len], &entry.hash)
    {
        log_prefix(
            b"vertex-store hash mismatch security event: object=",
            object_id,
        );
        log_prefix(
            b"vertex-inspect security event: store hash mismatch object=",
            object_id,
        );
        sys::exit(1);
    }
    if sys::vfs_close(handle) != sys::STATUS_OK {
        log_prefix(b"vertex-store executable VFS close failed: ", object_id);
        sys::exit(1);
    }
    log_prefix(b"vertex-store verifies executable store object: ", name);
}

fn executable_object_buffer() -> &'static mut [u8; MAX_EXECUTABLE_OBJECT_BYTES] {
    unsafe { &mut *EXECUTABLE_OBJECT_BUFFER.0.get() }
}

fn store_entry_bounds_valid(
    entry: StoreEntry,
    store_data_start: u64,
    store_data_count: u64,
) -> bool {
    if entry.byte_len == 0 {
        return false;
    }
    let sectors = sectors_for_len(entry.byte_len) as u64;
    entry.data_sector >= store_data_start
        && entry
            .data_sector
            .checked_add(sectors)
            .is_some_and(|end| end <= store_data_start + store_data_count)
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

fn read_block_sectors(start_sector: u64, count: usize, out: &mut [u8]) {
    let mut sector = [0u8; SECTOR_SIZE];
    let mut index = 0;
    while index < count {
        read_block_sector(start_sector + index as u64, &mut sector);
        let offset = index * SECTOR_SIZE;
        out[offset..offset + SECTOR_SIZE].copy_from_slice(&sector);
        index += 1;
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
    while section <= VERTEX_DISK_VERTEXFS_SECTION {
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

fn valid_index_header(sector: &[u8], magic: &[u8; 16]) -> bool {
    starts_with(sector, magic)
        && read_u16(sector, 16) == VERTEX_DISK_VERSION
        && metadata_checksum_valid(sector)
}

fn vertexdisk_section(sector: &[u8; SECTOR_SIZE], section: usize) -> Option<(u64, u64)> {
    let offset = VERTEX_DISK_SECTION_TABLE_OFFSET + section * VERTEX_DISK_SECTION_RECORD_LEN;
    if offset + 16 > sector.len() {
        return None;
    }
    Some((read_u64(sector, offset), read_u64(sector, offset + 8)))
}

fn metadata_checksum_valid(sector: &[u8]) -> bool {
    if sector.len() < VERTEX_DISK_CHECKSUM_OFFSET + 4 {
        return false;
    }
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
    checksum32_update(0, 0, bytes)
}

fn checksum32_update(mut checksum: u32, base_index: usize, bytes: &[u8]) -> u32 {
    let mut index = 0;
    while index < bytes.len() {
        checksum = checksum
            .wrapping_add((bytes[index] as u32).wrapping_mul((base_index + index) as u32 + 1));
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

fn fixed_hash(buffer: &[u8], offset: usize) -> [u8; 64] {
    let mut hash = [0u8; 64];
    let mut index = 0;
    while index < hash.len() && offset + index < buffer.len() {
        hash[index] = buffer[offset + index];
        index += 1;
    }
    hash
}

fn sectors_for_len(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (len + SECTOR_SIZE - 1) / SECTOR_SIZE
    }
}

fn hash_matches(bytes: &[u8], expected: &[u8; 64]) -> bool {
    hash_digest_matches(blake3::hash(bytes).as_bytes(), expected)
}

fn hash_digest_matches(bytes: &[u8; 32], expected: &[u8; 64]) -> bool {
    let mut actual = [0u8; 64];
    write_hash_hex(bytes, &mut actual);
    bytes_eq(&actual, expected)
}

fn write_hash_hex(bytes: &[u8; 32], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut index = 0;
    while index < bytes.len() {
        out[index * 2] = HEX[(bytes[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(bytes[index] & 0xf) as usize];
        index += 1;
    }
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let mut index = 0;
    while index < prefix.len() && index < buffer.len() {
        buffer[index] = prefix[index];
        index += 1;
    }
    let mut value_index = 0;
    while value_index < value.len() && index < buffer.len() {
        buffer[index] = value[value_index];
        index += 1;
        value_index += 1;
    }
    log(&buffer[..index]);
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
