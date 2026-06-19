#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;
use vertex_abi::{graph as graph_abi, vertexdisk as vdisk_abi};

const CAP_REQUEST: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_UPDATE_CONTROL: u64 = 3;
const CAP_BLOCK_REQUEST: u64 = 4;
const CAP_BLOCK_REPLY: u64 = 5;
const COMMAND_BUFFER_LEN: usize = 96;
const INSTALL_PREFIX: &[u8] = b"install ";
const REGISTER_IMPORT_PREFIX: &[u8] = b"register-import ";
const ROLLBACK_PREFIX: &[u8] = b"rollback ";
const SHUTDOWN_COMMAND: &[u8] = b"shutdown";
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;
const BLOCK_PROTOCOL_V1: u16 = 1;
const BLOCK_OP_READ_SECTOR: u16 = 1;
const BLOCK_OP_WRITE_SECTOR: u16 = 2;
const BLOCK_REQUEST_LEN: usize = 16;
const BLOCK_WRITE_ACK_LEN: usize = 16;
const SECTOR_SIZE: usize = vdisk_abi::SECTOR_SIZE;
const VERTEX_DISK_MAGIC: &[u8; 16] = vdisk_abi::MAGIC;
const VERTEX_DISK_VERSION: u16 = vdisk_abi::VERSION;
const VERTEX_DISK_CHECKSUM_OFFSET: usize = vdisk_abi::CHECKSUM_OFFSET;
const VERTEX_DISK_TOTAL_SECTORS_OFFSET: usize = vdisk_abi::TOTAL_SECTORS_OFFSET;
const VERTEX_DISK_SECTION_TABLE_OFFSET: usize = vdisk_abi::SECTION_TABLE_OFFSET;
const VERTEX_DISK_SECTION_RECORD_LEN: usize = vdisk_abi::SECTION_RECORD_LEN;
const VERTEX_DISK_GENERATION_METADATA_SECTION: usize = vdisk_abi::SECTION_GENERATION_METADATA;
const GENERATION_METADATA_MAGIC: &[u8; 16] = vdisk_abi::GENERATION_METADATA_MAGIC;
const GENERATION_METADATA_COUNT_OFFSET: usize = vdisk_abi::GENERATION_METADATA_COUNT_OFFSET;
const GENERATION_METADATA_TRANSACTION_STATE_OFFSET: usize =
    vdisk_abi::GENERATION_METADATA_TRANSACTION_STATE_OFFSET;
const GENERATION_METADATA_FAILURE_REASON_OFFSET: usize =
    vdisk_abi::GENERATION_METADATA_FAILURE_REASON_OFFSET;
const GENERATION_METADATA_SELECTED_OFFSET: usize = vdisk_abi::GENERATION_METADATA_SELECTED_OFFSET;
const GENERATION_METADATA_PREVIOUS_OFFSET: usize = vdisk_abi::GENERATION_METADATA_PREVIOUS_OFFSET;
const GENERATION_METADATA_KNOWN_GOOD_OFFSET: usize =
    vdisk_abi::GENERATION_METADATA_KNOWN_GOOD_OFFSET;
const GENERATION_METADATA_TRANSACTION_TARGET_OFFSET: usize =
    vdisk_abi::GENERATION_METADATA_TRANSACTION_TARGET_OFFSET;
const GENERATION_METADATA_ENTRY_OFFSET: usize = vdisk_abi::GENERATION_METADATA_ENTRY_OFFSET;
const GENERATION_METADATA_ENTRY_LEN: usize = vdisk_abi::GENERATION_METADATA_ENTRY_LEN;
const GENERATION_TRANSACTION_PREPARE: u16 = vdisk_abi::GENERATION_TRANSACTION_PREPARE;
const GENERATION_TRANSACTION_COMMIT: u16 = vdisk_abi::GENERATION_TRANSACTION_COMMIT;
const GENERATION_TRANSACTION_ROLLBACK: u16 = vdisk_abi::GENERATION_TRANSACTION_ROLLBACK;
const GENERATION_TRANSACTION_ABORT: u16 = vdisk_abi::GENERATION_TRANSACTION_ABORT;
const GENERATION_FAILURE_NONE: u16 = vdisk_abi::GENERATION_FAILURE_NONE;
const GENERATION_FAILURE_ACTIVATION_FAILED: u16 = vdisk_abi::GENERATION_FAILURE_ACTIVATION_FAILED;
const GENERATION_FAILURE_VERIFICATION_FAILED: u16 =
    vdisk_abi::GENERATION_FAILURE_VERIFICATION_FAILED;
const GENERATION_FAILURE_RUNTIME_BUILD_FAILED: u16 =
    vdisk_abi::GENERATION_FAILURE_RUNTIME_BUILD_FAILED;
const GENERATION_FAILURE_ROLLBACK_BUILD_FAILED: u16 =
    vdisk_abi::GENERATION_FAILURE_ROLLBACK_BUILD_FAILED;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    log(b"generation-manager ready");
    log(b"native generation-manager owns selected-generation state");
    send_ready();

    loop {
        let mut command = [0u8; COMMAND_BUFFER_LEN];
        let received = sys::ipc_recv(CAP_REQUEST, &mut command);
        if received == sys::STATUS_BAD_CAPABILITY
            || received == sys::STATUS_BAD_BUFFER
            || received == sys::STATUS_TOO_LARGE
            || received > command.len() as u64
        {
            log(b"generation-manager command receive failed");
            sys::exit(1);
        }

        let command = &command[..received as usize];
        if let Some(generation) = strip_prefix(command, REGISTER_IMPORT_PREFIX) {
            register_import_generation(generation);
            continue;
        }
        if let Some(generation) = strip_prefix(command, INSTALL_PREFIX) {
            install_generation(generation);
            continue;
        }
        if let Some(generation) = strip_prefix(command, ROLLBACK_PREFIX) {
            rollback_generation(generation);
            continue;
        }
        if bytes_eq(command, SHUTDOWN_COMMAND) {
            log(b"generation-manager shutdown requested");
            sys::exit(0);
        }

        log(b"generation-manager rejected unknown command");
    }
}

fn register_import_generation(generation: &[u8]) {
    log_generation(
        b"generation-manager registers imported graph generation: generation=",
        generation,
    );
    let Some(mut metadata) = load_generation_metadata() else {
        log_generation(
            b"generation-manager register import abort: reason=metadata-unavailable generation=",
            generation,
        );
        return;
    };
    if metadata.contains_generation(generation) {
        log_generation(
            b"generation-manager imported graph generation already registered: generation=",
            generation,
        );
        return;
    }
    if sys::verify_generation(CAP_UPDATE_CONTROL, generation) != sys::STATUS_OK {
        log_generation(
            b"generation-manager register import abort: reason=verification-failed generation=",
            generation,
        );
        return;
    }
    if !metadata.append_generation(generation) {
        log_generation(
            b"generation-manager register import abort: reason=metadata-write-failed generation=",
            generation,
        );
        return;
    }
    log_generation(
        b"generation-manager imported graph generation registered: generation=",
        generation,
    );
}

fn install_generation(generation: &[u8]) {
    log_generation(
        b"generation-manager install candidate from native graph-store: generation=",
        generation,
    );
    log_generation(
        b"generation-manager transaction prepare: generation=",
        generation,
    );
    let Some(mut metadata) = load_generation_metadata() else {
        log_generation(
            b"generation-manager transaction abort: reason=metadata-unavailable generation=",
            generation,
        );
        return;
    };
    if !metadata.contains_generation(generation) {
        log_generation(
            b"generation-manager transaction abort: reason=unknown-generation generation=",
            generation,
        );
        return;
    }
    let Some(selected) = metadata.selected_generation() else {
        log_generation(
            b"generation-manager transaction abort: reason=selected-generation-invalid generation=",
            generation,
        );
        return;
    };
    let known_good = metadata.known_good_generation().unwrap_or(selected);
    if !metadata.write_checkpoint(
        GENERATION_TRANSACTION_PREPARE,
        GENERATION_FAILURE_NONE,
        selected.as_slice(),
        selected.as_slice(),
        known_good.as_slice(),
        generation,
    ) {
        log_generation(
            b"generation-manager transaction abort: reason=prepare-write-failed generation=",
            generation,
        );
        return;
    }

    if sys::verify_generation(CAP_UPDATE_CONTROL, generation) != sys::STATUS_OK {
        let _ = metadata.write_checkpoint(
            GENERATION_TRANSACTION_ABORT,
            GENERATION_FAILURE_VERIFICATION_FAILED,
            selected.as_slice(),
            selected.as_slice(),
            known_good.as_slice(),
            generation,
        );
        log_generation(
            b"generation-manager transaction abort: reason=verification-failed generation=",
            generation,
        );
        return;
    }
    if sys::stage_generation(CAP_UPDATE_CONTROL, generation) != sys::STATUS_OK {
        let _ = metadata.write_checkpoint(
            GENERATION_TRANSACTION_ABORT,
            GENERATION_FAILURE_RUNTIME_BUILD_FAILED,
            selected.as_slice(),
            selected.as_slice(),
            known_good.as_slice(),
            generation,
        );
        log_generation(
            b"generation-manager transaction abort: reason=stage-failed generation=",
            generation,
        );
        return;
    }
    if !metadata.write_checkpoint(
        GENERATION_TRANSACTION_COMMIT,
        GENERATION_FAILURE_NONE,
        generation,
        selected.as_slice(),
        known_good.as_slice(),
        generation,
    ) {
        log_generation(
            b"generation-manager transaction abort: reason=commit-write-failed generation=",
            generation,
        );
        return;
    }

    let status = sys::activate_generation(CAP_UPDATE_CONTROL, generation);
    if status == sys::STATUS_OK {
        log_generation(
            b"generation-manager transaction commit returned: generation=",
            generation,
        );
        return;
    }
    let _ = metadata.write_checkpoint(
        GENERATION_TRANSACTION_ABORT,
        GENERATION_FAILURE_RUNTIME_BUILD_FAILED,
        selected.as_slice(),
        selected.as_slice(),
        known_good.as_slice(),
        generation,
    );
    log_generation(
        b"generation-manager transaction abort: reason=activation-rejected generation=",
        generation,
    );
}

fn rollback_generation(generation: &[u8]) {
    log_generation(
        b"generation-manager transaction rollback prepare: target=",
        generation,
    );
    let Some(mut metadata) = load_generation_metadata() else {
        log_generation(
            b"generation-manager transaction rollback abort: reason=metadata-unavailable target=",
            generation,
        );
        return;
    };
    if !metadata.contains_generation(generation) {
        log_generation(
            b"generation-manager transaction rollback abort: reason=unknown-generation target=",
            generation,
        );
        return;
    }
    let Some(selected) = metadata.selected_generation() else {
        log_generation(
            b"generation-manager transaction rollback abort: reason=selected-generation-invalid target=",
            generation,
        );
        return;
    };
    if sys::verify_generation(CAP_UPDATE_CONTROL, generation) != sys::STATUS_OK {
        log_generation(
            b"generation-manager transaction rollback abort: reason=verification-failed target=",
            generation,
        );
        return;
    }
    if sys::stage_rollback_generation(CAP_UPDATE_CONTROL, generation) != sys::STATUS_OK {
        let _ = metadata.write_checkpoint(
            GENERATION_TRANSACTION_ABORT,
            GENERATION_FAILURE_ROLLBACK_BUILD_FAILED,
            selected.as_slice(),
            generation,
            generation,
            generation,
        );
        log_generation(
            b"generation-manager transaction rollback abort: reason=stage-failed target=",
            generation,
        );
        return;
    }
    if !metadata.write_checkpoint(
        GENERATION_TRANSACTION_ROLLBACK,
        GENERATION_FAILURE_ACTIVATION_FAILED,
        generation,
        selected.as_slice(),
        generation,
        generation,
    ) {
        log_generation(
            b"generation-manager transaction rollback abort: reason=rollback-write-failed target=",
            generation,
        );
        return;
    }

    let status = sys::rollback_generation(CAP_UPDATE_CONTROL, generation);
    if status == sys::STATUS_OK {
        log_generation(
            b"generation-manager transaction rollback returned: target=",
            generation,
        );
        return;
    }
    let _ = metadata.write_checkpoint(
        GENERATION_TRANSACTION_ABORT,
        GENERATION_FAILURE_ROLLBACK_BUILD_FAILED,
        selected.as_slice(),
        generation,
        generation,
        generation,
    );
    log_generation(
        b"generation-manager transaction rollback abort: reason=rollback-rejected target=",
        generation,
    );
}

#[derive(Clone, Copy)]
struct FixedString {
    bytes: [u8; graph_abi::STRING_LEN],
    len: usize,
}

impl FixedString {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

struct GenerationMetadata {
    sector: [u8; SECTOR_SIZE],
    sector_number: u64,
}

impl GenerationMetadata {
    fn selected_generation(&self) -> Option<FixedString> {
        read_fixed_string(&self.sector, GENERATION_METADATA_SELECTED_OFFSET, false)
            .or_else(|| self.entry_generation(0))
    }

    fn known_good_generation(&self) -> Option<FixedString> {
        let known_good =
            read_fixed_string(&self.sector, GENERATION_METADATA_KNOWN_GOOD_OFFSET, true)?;
        if known_good.len == 0 {
            self.selected_generation()
        } else {
            Some(known_good)
        }
    }

    fn entry_generation(&self, index: usize) -> Option<FixedString> {
        let count = read_u16(&self.sector, GENERATION_METADATA_COUNT_OFFSET) as usize;
        if index >= count {
            return None;
        }
        let offset = GENERATION_METADATA_ENTRY_OFFSET + index * GENERATION_METADATA_ENTRY_LEN;
        read_fixed_string(&self.sector, offset, false)
    }

    fn contains_generation(&self, generation: &[u8]) -> bool {
        let count = read_u16(&self.sector, GENERATION_METADATA_COUNT_OFFSET) as usize;
        let mut index = 0;
        while index < count {
            if let Some(entry) = self.entry_generation(index)
                && entry.as_slice() == generation
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn append_generation(&mut self, generation: &[u8]) -> bool {
        let count = read_u16(&self.sector, GENERATION_METADATA_COUNT_OFFSET) as usize;
        let capacity =
            (SECTOR_SIZE - GENERATION_METADATA_ENTRY_OFFSET) / GENERATION_METADATA_ENTRY_LEN;
        if count >= capacity || count >= u16::MAX as usize {
            return false;
        }
        let offset = GENERATION_METADATA_ENTRY_OFFSET + count * GENERATION_METADATA_ENTRY_LEN;
        if write_fixed_string(&mut self.sector, offset, generation).is_none() {
            return false;
        }
        write_u16(
            &mut self.sector,
            GENERATION_METADATA_COUNT_OFFSET,
            (count + 1) as u16,
        );
        write_metadata_checksum(&mut self.sector);
        if !write_block_sector(self.sector_number, &self.sector) {
            return false;
        }
        log_registration(generation, count + 1);
        true
    }

    fn write_checkpoint(
        &mut self,
        transaction_state: u16,
        failure_reason: u16,
        selected: &[u8],
        previous: &[u8],
        known_good: &[u8],
        target: &[u8],
    ) -> bool {
        if selected.is_empty()
            || write_fixed_string(
                &mut self.sector,
                GENERATION_METADATA_SELECTED_OFFSET,
                selected,
            )
            .is_none()
            || write_fixed_string(
                &mut self.sector,
                GENERATION_METADATA_PREVIOUS_OFFSET,
                previous,
            )
            .is_none()
            || write_fixed_string(
                &mut self.sector,
                GENERATION_METADATA_KNOWN_GOOD_OFFSET,
                known_good,
            )
            .is_none()
            || write_fixed_string(
                &mut self.sector,
                GENERATION_METADATA_TRANSACTION_TARGET_OFFSET,
                target,
            )
            .is_none()
        {
            return false;
        }
        write_u16(
            &mut self.sector,
            GENERATION_METADATA_TRANSACTION_STATE_OFFSET,
            transaction_state,
        );
        write_u16(
            &mut self.sector,
            GENERATION_METADATA_FAILURE_REASON_OFFSET,
            failure_reason,
        );
        write_metadata_checksum(&mut self.sector);
        if !write_block_sector(self.sector_number, &self.sector) {
            return false;
        }
        log_checkpoint(transaction_state, selected, previous, target);
        true
    }
}

fn load_generation_metadata() -> Option<GenerationMetadata> {
    let mut superblock = [0u8; SECTOR_SIZE];
    if !read_block_sector(0, &mut superblock) || !valid_superblock(&superblock) {
        log(b"generation-manager VertexDisk superblock rejected");
        return None;
    }
    let (metadata_sector, metadata_count) =
        vertexdisk_section(&superblock, VERTEX_DISK_GENERATION_METADATA_SECTION)?;
    if metadata_count == 0 {
        log(b"generation-manager VertexDisk generation metadata section missing");
        return None;
    }

    let mut sector = [0u8; SECTOR_SIZE];
    if !read_block_sector(metadata_sector, &mut sector) {
        log(b"generation-manager generation metadata read failed");
        return None;
    }
    if !starts_with(&sector, GENERATION_METADATA_MAGIC)
        || read_u16(&sector, 16) != VERTEX_DISK_VERSION
        || !metadata_checksum_valid(&sector)
    {
        log(b"generation-manager generation metadata rejected");
        return None;
    }
    log(b"generation-manager reads VertexDisk generation metadata");
    Some(GenerationMetadata {
        sector,
        sector_number: metadata_sector,
    })
}

fn read_block_sector(sector: u64, out: &mut [u8; SECTOR_SIZE]) -> bool {
    let request = block_request(BLOCK_OP_READ_SECTOR, sector);
    if sys::ipc_send(CAP_BLOCK_REQUEST, &request) != sys::STATUS_OK {
        log(b"generation-manager block read request failed");
        return false;
    }
    let received = sys::ipc_recv(CAP_BLOCK_REPLY, out);
    if received != SECTOR_SIZE as u64 {
        log(b"generation-manager block read response failed");
        return false;
    }
    true
}

fn write_block_sector(sector: u64, bytes: &[u8; SECTOR_SIZE]) -> bool {
    let request = block_request(BLOCK_OP_WRITE_SECTOR, sector);
    if sys::ipc_send(CAP_BLOCK_REQUEST, &request) != sys::STATUS_OK
        || sys::ipc_send(CAP_BLOCK_REQUEST, bytes) != sys::STATUS_OK
    {
        log(b"generation-manager block write request failed");
        return false;
    }

    let mut ack = [0u8; BLOCK_WRITE_ACK_LEN];
    let received = sys::ipc_recv(CAP_BLOCK_REPLY, &mut ack);
    if received != BLOCK_WRITE_ACK_LEN as u64
        || read_u16(&ack, 0) != BLOCK_PROTOCOL_V1
        || read_u16(&ack, 2) != BLOCK_OP_WRITE_SECTOR
        || read_u16(&ack, 4) != 0
        || read_u64(&ack, 8) != sector
    {
        log(b"generation-manager block write ack failed");
        return false;
    }
    true
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
    if total_sectors == 0 {
        return false;
    }
    let Some((start, count)) = vertexdisk_section(sector, VERTEX_DISK_GENERATION_METADATA_SECTION)
    else {
        return false;
    };
    count != 0
        && start
            .checked_add(count)
            .is_some_and(|end| end <= total_sectors)
}

fn vertexdisk_section(sector: &[u8; SECTOR_SIZE], section: usize) -> Option<(u64, u64)> {
    let offset = VERTEX_DISK_SECTION_TABLE_OFFSET + section * VERTEX_DISK_SECTION_RECORD_LEN;
    if offset + 16 > sector.len() {
        return None;
    }
    Some((read_u64(sector, offset), read_u64(sector, offset + 8)))
}

fn read_fixed_string(
    sector: &[u8; SECTOR_SIZE],
    offset: usize,
    allow_empty: bool,
) -> Option<FixedString> {
    if offset + graph_abi::STRING_LEN > sector.len() {
        return None;
    }
    let mut len = 0;
    while len < graph_abi::STRING_LEN && sector[offset + len] != 0 {
        len += 1;
    }
    if len == 0 && !allow_empty {
        return None;
    }
    let mut padding = len;
    while padding < graph_abi::STRING_LEN {
        if sector[offset + padding] != 0 {
            return None;
        }
        padding += 1;
    }
    let mut bytes = [0u8; graph_abi::STRING_LEN];
    bytes[..len].copy_from_slice(&sector[offset..offset + len]);
    Some(FixedString { bytes, len })
}

fn write_fixed_string(sector: &mut [u8; SECTOR_SIZE], offset: usize, value: &[u8]) -> Option<()> {
    if value.len() > graph_abi::STRING_LEN || offset + graph_abi::STRING_LEN > sector.len() {
        return None;
    }
    let mut index = 0;
    while index < graph_abi::STRING_LEN {
        sector[offset + index] = 0;
        index += 1;
    }
    sector[offset..offset + value.len()].copy_from_slice(value);
    Some(())
}

fn metadata_checksum_valid(sector: &[u8; SECTOR_SIZE]) -> bool {
    read_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET) == metadata_checksum(sector)
}

fn write_metadata_checksum(sector: &mut [u8; SECTOR_SIZE]) {
    write_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET, 0);
    let checksum = metadata_checksum(sector);
    write_u32(sector, VERTEX_DISK_CHECKSUM_OFFSET, checksum);
}

fn metadata_checksum(sector: &[u8; SECTOR_SIZE]) -> u32 {
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
    checksum
}

fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix
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

fn log_checkpoint(transaction_state: u16, selected: &[u8], previous: &[u8], target: &[u8]) {
    let mut line = [0u8; 192];
    let mut len = append(
        &mut line,
        0,
        b"generation-manager writes VertexDisk generation metadata: transaction=",
    );
    len = append(&mut line, len, transaction_label(transaction_state));
    len = append(&mut line, len, b" selected=");
    len = append(&mut line, len, selected);
    len = append(&mut line, len, b" previous=");
    len = append_optional(&mut line, len, previous);
    len = append(&mut line, len, b" target=");
    len = append_optional(&mut line, len, target);
    log(&line[..len]);
}

fn log_registration(generation: &[u8], count: usize) {
    let mut line = [0u8; 160];
    let mut len = append(
        &mut line,
        0,
        b"generation-manager writes VertexDisk generation metadata: register generation=",
    );
    len = append(&mut line, len, generation);
    len = append(&mut line, len, b" count=");
    len = append_decimal(&mut line, len, count as u64);
    log(&line[..len]);
}

fn transaction_label(transaction_state: u16) -> &'static [u8] {
    match transaction_state {
        GENERATION_TRANSACTION_PREPARE => b"prepare",
        GENERATION_TRANSACTION_COMMIT => b"commit",
        GENERATION_TRANSACTION_ROLLBACK => b"rollback",
        GENERATION_TRANSACTION_ABORT => b"abort",
        _ => b"unknown",
    }
}

fn append_optional(buffer: &mut [u8], offset: usize, value: &[u8]) -> usize {
    if value.is_empty() {
        append(buffer, offset, b"<none>")
    } else {
        append(buffer, offset, value)
    }
}

fn strip_prefix<'a>(bytes: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    if bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix {
        Some(&bytes[prefix.len()..])
    } else {
        None
    }
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

fn log_generation(prefix: &[u8], generation: &[u8]) {
    let mut line = [0u8; 128];
    let mut len = append(&mut line, 0, prefix);
    len = append(&mut line, len, generation);
    log(&line[..len]);
}

fn append(buffer: &mut [u8], offset: usize, value: &[u8]) -> usize {
    let len = value.len().min(buffer.len().saturating_sub(offset));
    buffer[offset..offset + len].copy_from_slice(&value[..len]);
    offset + len
}

fn append_decimal(buffer: &mut [u8], offset: usize, mut value: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut digit_len = 0;
    if value == 0 {
        digits[0] = b'0';
        digit_len = 1;
    } else {
        while value > 0 && digit_len < digits.len() {
            digits[digit_len] = b'0' + (value % 10) as u8;
            value /= 10;
            digit_len += 1;
        }
    }

    let mut out = offset;
    while digit_len > 0 && out < buffer.len() {
        digit_len -= 1;
        buffer[out] = digits[digit_len];
        out += 1;
    }
    out
}

fn send_ready() {
    let ready = ready_message(b"gen-manager");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"generation-manager ready send failed");
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
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
