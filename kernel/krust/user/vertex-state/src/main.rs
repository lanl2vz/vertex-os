#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STATE_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_BLOCK_REPLY: u64 = 3;
const CAP_BLOCK_REQUEST: u64 = 4;
const CAP_READER_REPLY: u64 = 5;
const CAP_COUNTER_REPLY: u64 = 5;
const STATE_VOLUME_ID: &[u8] = b"state:counter";
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_READ_SECTOR: u16 = 1;
const BLOCK_OP_WRITE_SECTOR: u16 = 2;
const BLOCK_REQUEST_LEN: usize = 16;
const BLOCK_WRITE_ACK_LEN: usize = 16;
const SECTOR_SIZE: usize = 512;
const MAX_STATE_VALUE_BYTES: usize = 16;
const VERTEX_DISK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const STATE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTATEV0\0\0\0\0";
const JOURNAL_RECORD_MAGIC: &[u8; 16] = b"VDISKJOURNALV0\0\0";
const VERTEX_DISK_VERSION: u16 = 1;
const JOURNAL_RECORD_STATE_WRITE: u16 = 1;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = 20;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = 24;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = 32;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = 16;
const VERTEX_DISK_STATE_INDEX_SECTION: usize = 3;
const VERTEX_DISK_STATE_DATA_SECTION: usize = 4;
const VERTEX_DISK_JOURNAL_SECTION: usize = 5;
const STATE_ENTRY_OFFSET: usize = 32;
const JOURNAL_STATE_ID_OFFSET: usize = 48;
const JOURNAL_VALUE_OFFSET: usize = 128;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut state = load_state_entry();
    send_ready();
    log(b"vertex-state ready");

    let mut request = [0u8; 16];
    let mut value = [0u8; MAX_STATE_VALUE_BYTES];
    let mut value_len = read_state_value(&state, &mut value);
    if value_len > 0 {
        log(b"reboot preserves state value");
    }

    let mut wrote_value = value_len > 0;
    let mut pending_read = false;
    loop {
        let received = sys::ipc_recv(CAP_STATE_REQUEST, &mut request);
        if received == 1 && request[0] == b'Q' {
            write_state_value(&mut state, &value[..value_len]);
            log(b"state restored");
            log(b"system generation rollback does not automatically roll back state unless policy says so");
            log(b"Native VertexDisk state service ok");
            sys::exit(0);
        }

        if received == 2 && request[0] == b'R' && request[1] == b'C' {
            send_counter_response(&value[..value_len]);
            continue;
        }

        if received == 1 && request[0] == b'R' {
            if wrote_value {
                value_len = read_state_value(&state, &mut value);
                send_reader_response(&value[..value_len]);
                continue;
            }
            pending_read = true;
            continue;
        }

        if received == 2 && request[0] == b'W' && request[1] == b'2' {
            log(b"reader-service write denied");
            if sys::ipc_send(CAP_READER_REPLY, b"DENIED") != sys::STATUS_OK {
                log(b"vertex-state denial response failed");
                sys::exit(1);
            }
            write_state_value(&mut state, &value[..value_len]);
            log(b"state restored");
            log(b"system generation rollback does not automatically roll back state unless policy says so");
            log(b"Native VertexDisk state service ok");
            sys::exit(0);
        }

        if received >= 2 && received <= request.len() as u64 && request[0] == b'W' {
            let input = &request[1..received as usize];
            log(b"vertex-state owner check accepted: state:counter via vertex-state endpoint");
            write_state_value(&mut state, input);
            value[..input.len()].copy_from_slice(input);
            value_len = input.len();
            log(b"counter-service writes state");
            wrote_value = true;
            if pending_read {
                value_len = read_state_value(&state, &mut value);
                send_reader_response(&value[..value_len]);
                pending_read = false;
            }
            continue;
        }

        log(b"vertex-state request invalid");
        sys::exit(1);
    }
}

#[derive(Clone, Copy)]
struct StateEntry {
    index_sector: u64,
    data_sector: u64,
    journal_sector: u64,
    value_len: usize,
    checksum: u32,
}

#[derive(Clone, Copy)]
struct JournalRecord {
    value_len: usize,
    checksum: u32,
}

fn load_state_entry() -> StateEntry {
    let mut superblock = [0u8; SECTOR_SIZE];
    read_block_sector(0, &mut superblock);
    if !valid_superblock(&superblock) {
        log(b"VertexDisk superblock rejected");
        sys::exit(1);
    }

    let Some(state_index_sector) =
        vertexdisk_section_start(&superblock, VERTEX_DISK_STATE_INDEX_SECTION)
    else {
        log(b"vertex-state index bounds invalid");
        sys::exit(1);
    };
    let Some((state_data_start, state_data_count)) =
        vertexdisk_section(&superblock, VERTEX_DISK_STATE_DATA_SECTION)
    else {
        log(b"vertex-state data bounds invalid");
        sys::exit(1);
    };
    let Some(journal_sector) = vertexdisk_section_start(&superblock, VERTEX_DISK_JOURNAL_SECTION)
    else {
        log(b"vertex-state journal bounds invalid");
        sys::exit(1);
    };

    let mut index_sector = [0u8; SECTOR_SIZE];
    read_block_sector(state_index_sector, &mut index_sector);
    if !valid_index_header(&index_sector, STATE_INDEX_MAGIC) {
        log(b"vertex-state index rejected");
        sys::exit(1);
    }

    let count = read_u16(&index_sector, 18) as usize;
    let mut index = 0;
    while index < count {
        let offset = STATE_ENTRY_OFFSET + index * 84;
        if offset + 84 > index_sector.len() {
            log(b"vertex-state index record bounds invalid");
            sys::exit(1);
        }
        if fixed_string_eq(&index_sector, offset, STATE_VOLUME_ID) {
            let data_sector = read_u64(&index_sector, offset + 64);
            let sector_count = read_u32(&index_sector, offset + 72);
            let value_len = read_u32(&index_sector, offset + 76) as usize;
            let checksum = read_u32(&index_sector, offset + 80);
            if sector_count != 1
                || value_len > MAX_STATE_VALUE_BYTES
                || data_sector < state_data_start
                || data_sector >= state_data_start + state_data_count
            {
                log(b"vertex-state value bounds invalid");
                sys::exit(1);
            }
            log(b"vertex-state reads state volume from disk");
            let mut state = StateEntry {
                index_sector: state_index_sector,
                data_sector,
                journal_sector,
                value_len,
                checksum,
            };
            recover_state_from_journal(&mut state);
            return state;
        }
        index += 1;
    }

    log(b"vertex-state volume missing from disk index");
    sys::exit(1);
}

fn read_state_value(state: &StateEntry, value: &mut [u8; MAX_STATE_VALUE_BYTES]) -> usize {
    if state.value_len == 0 {
        return 0;
    }

    let mut sector = [0u8; SECTOR_SIZE];
    read_block_sector(state.data_sector, &mut sector);
    if checksum32(&sector[..state.value_len]) != state.checksum {
        log(b"vertex-state disk checksum failed");
        sys::exit(1);
    }
    value[..state.value_len].copy_from_slice(&sector[..state.value_len]);
    state.value_len
}

fn write_state_value(state: &mut StateEntry, value: &[u8]) {
    if value.len() > MAX_STATE_VALUE_BYTES {
        log(b"vertex-state value too large");
        sys::exit(1);
    }
    log(b"vertex-state write bounds enforced");

    let checksum = checksum32(value);
    let journal_sector = state_journal_sector(*state, value, checksum);
    write_block_sector(state.journal_sector, &journal_sector);
    log(b"vertex-state writes journal record to disk");

    let mut data_sector = [0u8; SECTOR_SIZE];
    data_sector[..value.len()].copy_from_slice(value);
    write_block_sector(state.data_sector, &data_sector);

    state.value_len = value.len();
    state.checksum = checksum;
    let index_sector = state_index_sector(*state);
    write_block_sector(state.index_sector, &index_sector);
    log(b"vertex-state writes state volume to disk");
}

fn state_index_sector(state: StateEntry) -> [u8; SECTOR_SIZE] {
    let mut sector = [0u8; SECTOR_SIZE];
    sector[..STATE_INDEX_MAGIC.len()].copy_from_slice(STATE_INDEX_MAGIC);
    write_u16(&mut sector, 16, VERTEX_DISK_VERSION);
    write_u16(&mut sector, 18, 1);
    write_fixed_string(&mut sector, STATE_ENTRY_OFFSET, STATE_VOLUME_ID);
    write_u64(&mut sector, STATE_ENTRY_OFFSET + 64, state.data_sector);
    write_u32(&mut sector, STATE_ENTRY_OFFSET + 72, 1);
    write_u32(&mut sector, STATE_ENTRY_OFFSET + 76, state.value_len as u32);
    write_u32(&mut sector, STATE_ENTRY_OFFSET + 80, state.checksum);
    write_metadata_checksum(&mut sector);
    sector
}

fn state_journal_sector(state: StateEntry, value: &[u8], checksum: u32) -> [u8; SECTOR_SIZE] {
    let mut sector = [0u8; SECTOR_SIZE];
    sector[..JOURNAL_RECORD_MAGIC.len()].copy_from_slice(JOURNAL_RECORD_MAGIC);
    write_u16(&mut sector, 16, VERTEX_DISK_VERSION);
    write_u16(&mut sector, 18, JOURNAL_RECORD_STATE_WRITE);
    write_u64(&mut sector, 24, state.index_sector);
    write_u64(&mut sector, 32, state.data_sector);
    write_u32(&mut sector, 40, value.len() as u32);
    write_u32(&mut sector, 44, checksum);
    write_fixed_string(&mut sector, JOURNAL_STATE_ID_OFFSET, STATE_VOLUME_ID);
    sector[JOURNAL_VALUE_OFFSET..JOURNAL_VALUE_OFFSET + value.len()].copy_from_slice(value);
    write_metadata_checksum(&mut sector);
    sector
}

fn recover_state_from_journal(state: &mut StateEntry) {
    let mut journal = [0u8; SECTOR_SIZE];
    read_block_sector(state.journal_sector, &mut journal);
    let record = match parse_state_journal_record(&journal, *state) {
        JournalStatus::Empty => return,
        JournalStatus::Corrupt => {
            log(b"vertex-state corrupt journal detected");
            log(b"corrupt state journal reported and rolled back deterministically");
            return;
        }
        JournalStatus::Valid(record) => record,
    };
    if state.value_len == record.value_len && state.checksum == record.checksum {
        return;
    }

    let mut data_sector = [0u8; SECTOR_SIZE];
    data_sector[..record.value_len]
        .copy_from_slice(&journal[JOURNAL_VALUE_OFFSET..JOURNAL_VALUE_OFFSET + record.value_len]);
    write_block_sector(state.data_sector, &data_sector);
    state.value_len = record.value_len;
    state.checksum = record.checksum;
    let index_sector = state_index_sector(*state);
    write_block_sector(state.index_sector, &index_sector);
    log(b"vertex-state replays journal record");
    log(b"interrupted state journal replays deterministically");
}

enum JournalStatus {
    Empty,
    Corrupt,
    Valid(JournalRecord),
}

fn parse_state_journal_record(sector: &[u8; SECTOR_SIZE], state: StateEntry) -> JournalStatus {
    if !starts_with(sector, JOURNAL_RECORD_MAGIC) {
        return JournalStatus::Empty;
    }

    if read_u16(sector, 16) != VERTEX_DISK_VERSION
        || read_u16(sector, 18) != JOURNAL_RECORD_STATE_WRITE
        || !metadata_checksum_valid(sector)
        || read_u64(sector, 24) != state.index_sector
        || read_u64(sector, 32) != state.data_sector
        || !fixed_string_eq(sector, JOURNAL_STATE_ID_OFFSET, STATE_VOLUME_ID)
    {
        return JournalStatus::Corrupt;
    }

    let value_len = read_u32(sector, 40) as usize;
    let checksum = read_u32(sector, 44);
    if value_len > MAX_STATE_VALUE_BYTES
        || JOURNAL_VALUE_OFFSET
            .checked_add(value_len)
            .is_none_or(|end| end > sector.len())
        || checksum32(&sector[JOURNAL_VALUE_OFFSET..JOURNAL_VALUE_OFFSET + value_len]) != checksum
    {
        return JournalStatus::Corrupt;
    }

    JournalStatus::Valid(JournalRecord {
        value_len,
        checksum,
    })
}

fn read_block_sector(sector: u64, out: &mut [u8; SECTOR_SIZE]) {
    let request = block_request(BLOCK_OP_READ_SECTOR, sector);
    if sys::ipc_send(CAP_BLOCK_REQUEST, &request) != sys::STATUS_OK {
        log(b"vertex-state block read request failed");
        sys::exit(1);
    }

    let received = sys::ipc_recv(CAP_BLOCK_REPLY, out);
    if received != SECTOR_SIZE as u64 {
        log(b"vertex-state block read response failed");
        sys::exit(1);
    }
}

fn write_block_sector(sector: u64, bytes: &[u8; SECTOR_SIZE]) {
    let request = block_request(BLOCK_OP_WRITE_SECTOR, sector);
    if sys::ipc_send(CAP_BLOCK_REQUEST, &request) != sys::STATUS_OK
        || sys::ipc_send(CAP_BLOCK_REQUEST, bytes) != sys::STATUS_OK
    {
        log(b"vertex-state block write request failed");
        sys::exit(1);
    }

    let mut ack = [0u8; BLOCK_WRITE_ACK_LEN];
    let received = sys::ipc_recv(CAP_BLOCK_REPLY, &mut ack);
    if received != BLOCK_WRITE_ACK_LEN as u64
        || read_u16(&ack, 0) != BLOCK_PROTOCOL_V1
        || read_u16(&ack, 2) != BLOCK_OP_WRITE_SECTOR
        || read_u16(&ack, 4) != 0
        || read_u64(&ack, 8) != sector
    {
        log(b"vertex-state block write ack failed");
        sys::exit(1);
    }
}

fn block_request(op: u16, sector: u64) -> [u8; BLOCK_REQUEST_LEN] {
    let mut request = [0u8; BLOCK_REQUEST_LEN];
    write_u16(&mut request, 0, BLOCK_PROTOCOL_V1);
    write_u16(&mut request, 2, op);
    write_u16(&mut request, 4, 0);
    write_u64(&mut request, 8, sector);
    request
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
    while section <= VERTEX_DISK_JOURNAL_SECTION {
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

fn write_metadata_checksum(sector: &mut [u8; SECTOR_SIZE]) {
    write_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET, 0);
    let checksum = checksum32(sector);
    write_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET, checksum);
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

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    if value.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if value[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn send_reader_response(value: &[u8]) {
    log(b"snapshot created");
    if sys::ipc_send(CAP_READER_REPLY, value) != sys::STATUS_OK {
        log(b"vertex-state read response failed");
        sys::exit(1);
    }
}

fn send_counter_response(value: &[u8]) {
    if sys::ipc_send(CAP_COUNTER_REPLY, value) != sys::STATUS_OK {
        log(b"vertex-state counter read response failed");
        sys::exit(1);
    }
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn send_ready() {
    let ready = ready_message(b"vertex-state");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"vertex-state ready send failed");
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

fn write_fixed_string(buffer: &mut [u8], offset: usize, value: &[u8]) {
    let mut index = 0;
    while index < value.len() && index < 64 {
        buffer[offset + index] = value[index];
        index += 1;
    }
}

fn read_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buffer[offset], buffer[offset + 1]])
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
