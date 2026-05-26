use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use vertex_ir::GenerationManifest;

const SECTOR_SIZE: usize = 512;
const SECTORS: usize = 65_536;
const SUPERBLOCK_MAGIC: &[u8; 16] = b"VERTEXDISKV0\0\0\0\0";
const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
const STATE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTATEV0\0\0\0\0";
const VERSION: u16 = 1;
const CHECKSUM_OFFSET: usize = 20;
const SECTION_TABLE_OFFSET: usize = 32;
const SECTION_RECORD_LEN: usize = 16;
const STORE_ENTRY_OFFSET: usize = 32;
const STORE_ENTRY_LEN: usize = 144;
const STATE_ENTRY_OFFSET: usize = 32;
const GENERATION_METADATA_SECTOR: u64 = 1;
const STORE_INDEX_SECTOR: u64 = 2;
const STORE_INDEX_SECTORS: u64 = 16;
const STORE_DATA_SECTOR: u64 = 32;
const STORE_DATA_SECTORS: u64 = 49_152;
const STATE_INDEX_SECTOR: u64 = STORE_DATA_SECTOR + STORE_DATA_SECTORS;
const STATE_DATA_SECTOR: u64 = STATE_INDEX_SECTOR + 1;
const STATE_DATA_SECTORS: u64 = 8;
const JOURNAL_SECTOR: u64 = STATE_DATA_SECTOR + STATE_DATA_SECTORS;
const JOURNAL_SECTORS: u64 = 16;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const HELLO_OBJECT_ID: &str = "store:hello-text";
const BLOCK_DRIVER_FAULT_OBJECT: &[u8] = b"krust-block-driver-fault\n";
const LOGD_OBJECT_ID: &str = "store:logd-demo";
const STATE_VOLUME_ID: &str = "state:counter";

pub fn create_image(manifests: &[GenerationManifest]) -> Result<Vec<u8>, String> {
    if manifests.is_empty() {
        return Err("usage: vertexctl create-vertex-disk <output> <manifest>...".to_owned());
    }

    let mut image = vec![0u8; SECTOR_SIZE * SECTORS];
    let mut objects = store_payloads(manifests)?;
    assign_store_sectors(&mut objects)?;
    write_superblock(&mut image);
    write_store_index(&mut image, &objects)?;
    write_state_index(&mut image, 0, 0);
    write_store_payloads(&mut image, &objects);
    Ok(image)
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
        "store-object" => corrupt_store_payload(&mut out, HELLO_OBJECT_ID)?,
        "store-executable" => corrupt_store_payload(&mut out, LOGD_OBJECT_ID)?,
        "missing-store-object" => {
            let index = store_index_mut(&mut out)?;
            write_u16(index, 18, 0);
            write_checksum(index);
        }
        other => {
            return Err(format!(
                "unknown VertexDisk corruption mode {other}; expected bad-superblock, store-object, store-executable, or missing-store-object"
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
    write_section(sector, 1, STORE_INDEX_SECTOR, STORE_INDEX_SECTORS);
    write_section(sector, 2, STORE_DATA_SECTOR, STORE_DATA_SECTORS);
    write_section(sector, 3, STATE_INDEX_SECTOR, 1);
    write_section(sector, 4, STATE_DATA_SECTOR, STATE_DATA_SECTORS);
    write_section(sector, 5, JOURNAL_SECTOR, JOURNAL_SECTORS);
    write_checksum(sector);
}

struct StorePayload {
    id: String,
    bytes: Vec<u8>,
    sector: u64,
}

fn store_payloads(manifests: &[GenerationManifest]) -> Result<Vec<StorePayload>, String> {
    let mut required = BTreeSet::new();
    for manifest in manifests {
        for executable in &manifest.executables {
            required.insert(executable.store_object.clone());
        }
        for capability in &manifest.capabilities {
            if capability.kind == "store-object" {
                required.insert(capability.provider.clone());
            }
            if let Some(object) = capability
                .properties
                .get("object")
                .and_then(|value| value.as_str())
                && manifest.store_object(object).is_some()
            {
                required.insert(object.to_owned());
            }
        }
    }

    let mut objects = Vec::new();
    for id in required {
        objects.push(store_payload(manifests, &id)?);
    }
    Ok(objects)
}

fn store_payload(manifests: &[GenerationManifest], id: &str) -> Result<StorePayload, String> {
    let store = manifests
        .iter()
        .find_map(|manifest| manifest.store_object(id))
        .ok_or_else(|| format!("VertexDisk store object {id} missing from manifests"))?;
    let bytes = match store.kind.as_str() {
        "data" => data_store_bytes(&store.name)?,
        "executable" => executable_store_bytes(manifests, id)?,
        other => {
            return Err(format!(
                "VertexDisk cannot materialize store object {} of kind {other}",
                store.id
            ));
        }
    };

    Ok(StorePayload {
        id: store.id.clone(),
        bytes,
        sector: 0,
    })
}

fn data_store_bytes(name: &str) -> Result<Vec<u8>, String> {
    match name {
        "store-hello-text" => Ok(HELLO_OBJECT.to_vec()),
        "store-block-driver-fault-token" => Ok(BLOCK_DRIVER_FAULT_OBJECT.to_vec()),
        other => native_store_bytes(other),
    }
}

fn executable_store_bytes(
    manifests: &[GenerationManifest],
    store_id: &str,
) -> Result<Vec<u8>, String> {
    let executable = manifests
        .iter()
        .flat_map(|manifest| manifest.executables.iter())
        .find(|executable| executable.store_object == store_id)
        .ok_or_else(|| format!("executable store object {store_id} has no executable binding"))?;
    native_store_bytes(module_basename(&executable.entrypoint))
}

fn module_basename(entrypoint: &str) -> &str {
    entrypoint.rsplit('/').next().unwrap_or(entrypoint)
}

fn assign_store_sectors(objects: &mut [StorePayload]) -> Result<(), String> {
    let mut cursor = STORE_DATA_SECTOR;
    for object in objects {
        object.sector = cursor;
        cursor = cursor
            .checked_add(sectors_for_len(object.bytes.len()) as u64)
            .ok_or_else(|| "VertexDisk store payload sector overflow".to_owned())?;
    }
    if cursor > STORE_DATA_SECTOR + STORE_DATA_SECTORS {
        return Err(format!(
            "VertexDisk store payloads require {} sectors, capacity is {}",
            cursor - STORE_DATA_SECTOR,
            STORE_DATA_SECTORS
        ));
    }
    Ok(())
}

fn write_store_index(image: &mut [u8], objects: &[StorePayload]) -> Result<(), String> {
    let sector = store_index_mut(image)?;
    sector[..STORE_INDEX_MAGIC.len()].copy_from_slice(STORE_INDEX_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, objects.len() as u16);
    for (index, object) in objects.iter().enumerate() {
        let offset = STORE_ENTRY_OFFSET + index * STORE_ENTRY_LEN;
        if offset + STORE_ENTRY_LEN > sector.len() {
            return Err(format!(
                "VertexDisk store index holds at most {} objects",
                (sector.len() - STORE_ENTRY_OFFSET) / STORE_ENTRY_LEN
            ));
        }
        write_fixed_str(sector, offset, &object.id);
        write_u64(sector, offset + 64, object.sector);
        write_u32(sector, offset + 72, object.bytes.len() as u32);
        write_u32(sector, offset + 76, checksum32(&object.bytes));
        write_fixed_str(sector, offset + 80, &store_hash_hex(&object.bytes));
    }
    write_checksum(sector);
    Ok(())
}

fn write_store_payloads(image: &mut [u8], objects: &[StorePayload]) {
    for object in objects {
        write_bytes(image, object.sector, &object.bytes);
    }
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

fn write_bytes(image: &mut [u8], start_sector: u64, bytes: &[u8]) {
    let mut offset = 0;
    let mut sector = start_sector;
    while offset < bytes.len() {
        let len = (bytes.len() - offset).min(SECTOR_SIZE);
        write_sector_bytes(image, sector, &bytes[offset..offset + len]);
        offset += len;
        sector += 1;
    }
}

fn corrupt_store_payload(image: &mut [u8], id: &str) -> Result<(), String> {
    let offset = store_payload_offset(image, id)?;
    let byte = image
        .get_mut(offset)
        .ok_or_else(|| format!("cannot corrupt missing VertexDisk store object {id}"))?;
    *byte ^= 1;
    Ok(())
}

fn store_payload_offset(image: &[u8], id: &str) -> Result<usize, String> {
    let index = store_index(image)?;
    let count = read_u16(index, 18) as usize;
    let mut item = 0;
    while item < count {
        let offset = STORE_ENTRY_OFFSET + item * STORE_ENTRY_LEN;
        if offset + STORE_ENTRY_LEN > index.len() {
            return Err("VertexDisk store index bounds invalid".to_owned());
        }
        if fixed_string_eq(index, offset, id) {
            let sector = read_u64(index, offset + 64);
            return usize::try_from(sector)
                .ok()
                .and_then(|sector| sector.checked_mul(SECTOR_SIZE))
                .ok_or_else(|| format!("VertexDisk store object {id} offset overflow"));
        }
        item += 1;
    }
    Err(format!("VertexDisk store object {id} missing"))
}

fn sector_mut(image: &mut [u8], sector: u64) -> &mut [u8] {
    let offset = sector as usize * SECTOR_SIZE;
    &mut image[offset..offset + SECTOR_SIZE]
}

fn store_index(image: &[u8]) -> Result<&[u8], String> {
    let offset = STORE_INDEX_SECTOR as usize * SECTOR_SIZE;
    let len = STORE_INDEX_SECTORS as usize * SECTOR_SIZE;
    image
        .get(offset..offset + len)
        .ok_or_else(|| "VertexDisk store index section missing".to_owned())
}

fn store_index_mut(image: &mut [u8]) -> Result<&mut [u8], String> {
    let offset = STORE_INDEX_SECTOR as usize * SECTOR_SIZE;
    let len = STORE_INDEX_SECTORS as usize * SECTOR_SIZE;
    image
        .get_mut(offset..offset + len)
        .ok_or_else(|| "VertexDisk store index section missing".to_owned())
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

fn native_store_bytes(module_string: &str) -> Result<Vec<u8>, String> {
    let mut candidates = Vec::new();
    for path in native_store_candidate_paths(module_string) {
        candidates.push(path.display().to_string());
        if let Ok(bytes) = fs::read(&path) {
            return Ok(bytes);
        }
    }
    Err(format!(
        "native store artifact {module_string} missing; checked {}",
        candidates.join(", ")
    ))
}

fn native_store_candidate_paths(module_string: &str) -> Vec<PathBuf> {
    let crate_dir = if module_string == "vertex-init" {
        "init"
    } else {
        module_string
    };
    vec![
        PathBuf::from(format!(
            "user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
        )),
        PathBuf::from(format!(
            "kernel/krust/user/{crate_dir}/target/x86_64-unknown-none/debug/{module_string}"
        )),
    ]
}

fn sectors_for_len(len: usize) -> usize {
    len.div_ceil(SECTOR_SIZE).max(1)
}

fn write_checksum(bytes: &mut [u8]) {
    write_u32(bytes, CHECKSUM_OFFSET, 0);
    let checksum = checksum32(bytes);
    write_u32(bytes, CHECKSUM_OFFSET, checksum);
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (index, byte) in bytes.iter().enumerate() {
        checksum = checksum.wrapping_add((*byte as u32).wrapping_mul(index as u32 + 1));
    }
    checksum
}

fn store_hash_hex(bytes: &[u8]) -> String {
    let digest = blake3::hash(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn fixed_string_eq(buffer: &[u8], offset: usize, value: &str) -> bool {
    let bytes = value.as_bytes();
    if offset + 64 > buffer.len() || bytes.len() > 64 {
        return false;
    }
    if &buffer[offset..offset + bytes.len()] != bytes {
        return false;
    }
    bytes.len() == 64 || buffer[offset + bytes.len()] == 0
}

fn read_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buffer[offset], buffer[offset + 1]])
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

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
