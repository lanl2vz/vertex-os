use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::krustboot;
use crate::vertexfs;
use vertex_abi::vertexdisk as vdisk_abi;
use vertex_ir::GenerationManifest;

const SECTOR_SIZE: usize = vdisk_abi::SECTOR_SIZE;
const SECTORS: usize = 65_536;
const SUPERBLOCK_MAGIC: &[u8; 16] = vdisk_abi::MAGIC;
const STORE_INDEX_MAGIC: &[u8; 16] = vdisk_abi::STORE_INDEX_MAGIC;
const STATE_INDEX_MAGIC: &[u8; 16] = vdisk_abi::STATE_INDEX_MAGIC;
const VERSION: u16 = vdisk_abi::VERSION;
const CHECKSUM_OFFSET: usize = vdisk_abi::CHECKSUM_OFFSET;
const SECTION_TABLE_OFFSET: usize = vdisk_abi::SECTION_TABLE_OFFSET;
const SECTION_RECORD_LEN: usize = vdisk_abi::SECTION_RECORD_LEN;
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
const GENERATION_TRANSACTION_CLEAN: u16 = vdisk_abi::GENERATION_TRANSACTION_CLEAN;
const GENERATION_TRANSACTION_PREPARE: u16 = vdisk_abi::GENERATION_TRANSACTION_PREPARE;
const GENERATION_TRANSACTION_COMMIT: u16 = vdisk_abi::GENERATION_TRANSACTION_COMMIT;
const GENERATION_TRANSACTION_ROLLBACK: u16 = vdisk_abi::GENERATION_TRANSACTION_ROLLBACK;
const GENERATION_FAILURE_NONE: u16 = vdisk_abi::GENERATION_FAILURE_NONE;
const GENERATION_FAILURE_ACTIVATION_FAILED: u16 = vdisk_abi::GENERATION_FAILURE_ACTIVATION_FAILED;
const STORE_ENTRY_OFFSET: usize = 32;
const STORE_ENTRY_LEN: usize = 144;
const STATE_ENTRY_OFFSET: usize = 32;
const STATE_ENTRY_LEN: usize = 84;
const GENERATION_METADATA_SECTOR: u64 = 1;
const STORE_INDEX_SECTOR: u64 = 2;
const STORE_INDEX_SECTORS: u64 = 16;
const STORE_DATA_SECTOR: u64 = 32;
const STORE_DATA_SECTORS: u64 = 49_152;
const STATE_INDEX_SECTOR: u64 = STORE_DATA_SECTOR + STORE_DATA_SECTORS;
const STATE_DATA_SECTOR: u64 = STATE_INDEX_SECTOR + 1;
const STATE_DATA_SECTORS: u64 = 8;
const KRUST_STATE_VOLUME_LIMIT: usize = 4;
const JOURNAL_SECTOR: u64 = STATE_DATA_SECTOR + STATE_DATA_SECTORS;
const JOURNAL_SECTORS: u64 = 16;
const VERTEXFS_IMAGE_SECTOR: u64 = JOURNAL_SECTOR + JOURNAL_SECTORS;
const VERTEXFS_IMAGE_SECTORS: u64 = 64;
const GRAPH_STORE_SECTOR: u64 = VERTEXFS_IMAGE_SECTOR + VERTEXFS_IMAGE_SECTORS;
const GRAPH_STORE_SECTORS: u64 = 128;
const JOURNAL_RECORD_MAGIC: &[u8; 16] = vdisk_abi::JOURNAL_RECORD_MAGIC;
const JOURNAL_RECORD_STATE_WRITE: u16 = vdisk_abi::JOURNAL_RECORD_STATE_WRITE;
const JOURNAL_STATE_ID_OFFSET: usize = vdisk_abi::JOURNAL_STATE_ID_OFFSET;
const JOURNAL_VALUE_OFFSET: usize = vdisk_abi::JOURNAL_VALUE_OFFSET;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const HELLO_OBJECT_ID: &str = "store:hello-text";
const BLOCK_DRIVER_FAULT_OBJECT: &[u8] = b"krust-block-driver-fault\n";
const VERTEXFS_FSYNC_FAULT_OBJECT: &[u8] = b"krust-vertexfs-fsync-fault\n";
const LOGD_OBJECT_ID: &str = "store:logd-demo";
const LOGD_CONFIG_OBJECT_ID: &str = "config:logd";
const LOGD_CONFIG_MODULE: &str = "config-logd-v0";
const LOGD_CONFIG_BYTES: &[u8] = b"{\"level\":\"info\",\"sink\":\"serial\"}\n";

pub fn create_image(manifests: &[GenerationManifest]) -> Result<Vec<u8>, String> {
    if manifests.is_empty() {
        return Err("usage: vertexctl create-vertex-disk <output> <manifest>...".to_owned());
    }

    let mut image = vec![0u8; SECTOR_SIZE * SECTORS];
    let states = state_entries(manifests)?;
    let mut objects = store_payloads(manifests)?;
    let graph_stores = manifests
        .iter()
        .map(krustboot::graph_store_image)
        .collect::<Result<Vec<_>, _>>()?;
    assign_store_sectors(&mut objects)?;
    write_superblock(&mut image);
    write_generation_metadata(&mut image, manifests)?;
    write_store_index(&mut image, &objects)?;
    write_state_index(&mut image, &states)?;
    write_store_payloads(&mut image, &objects);
    write_vertexfs_image(&mut image, &manifests[0])?;
    write_graph_stores(&mut image, &graph_stores)?;
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
        "config-object" => corrupt_store_payload(&mut out, LOGD_CONFIG_OBJECT_ID)?,
        "missing-store-object" => {
            let index = store_index_mut(&mut out)?;
            write_u16(index, 18, 0);
            write_checksum(index);
        }
        "interrupted-state-journal" => {
            write_state_journal(&mut out, b"42")?;
        }
        "corrupt-state-journal" => {
            write_state_journal(&mut out, b"42")?;
            let journal = sector_mut(&mut out, JOURNAL_SECTOR);
            journal[JOURNAL_VALUE_OFFSET] ^= 1;
        }
        "graph-store" => {
            let offset = GRAPH_STORE_SECTOR
                .checked_add(1)
                .and_then(|sector| sector.checked_mul(SECTOR_SIZE as u64))
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(|| "VertexDisk graph-store offset overflow".to_owned())?;
            let byte = out
                .get_mut(offset)
                .ok_or_else(|| "VertexDisk graph-store object missing".to_owned())?;
            *byte ^= 1;
        }
        "generation-prepare" => {
            write_generation_transaction_checkpoint(
                &mut out,
                GENERATION_TRANSACTION_PREPARE,
                GENERATION_FAILURE_NONE,
            )?;
        }
        "generation-commit" => {
            write_generation_transaction_checkpoint(
                &mut out,
                GENERATION_TRANSACTION_COMMIT,
                GENERATION_FAILURE_NONE,
            )?;
        }
        "generation-rollback" => {
            write_generation_transaction_checkpoint(
                &mut out,
                GENERATION_TRANSACTION_ROLLBACK,
                GENERATION_FAILURE_ACTIVATION_FAILED,
            )?;
        }
        other => {
            return Err(format!(
                "unknown VertexDisk corruption mode {other}; expected bad-superblock, store-object, store-executable, config-object, missing-store-object, interrupted-state-journal, corrupt-state-journal, graph-store, generation-prepare, generation-commit, or generation-rollback"
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
    write_section(
        sector,
        vdisk_abi::SECTION_GENERATION_METADATA,
        GENERATION_METADATA_SECTOR,
        1,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_STORE_INDEX,
        STORE_INDEX_SECTOR,
        STORE_INDEX_SECTORS,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_STORE_DATA,
        STORE_DATA_SECTOR,
        STORE_DATA_SECTORS,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_STATE_INDEX,
        STATE_INDEX_SECTOR,
        1,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_STATE_DATA,
        STATE_DATA_SECTOR,
        STATE_DATA_SECTORS,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_JOURNAL,
        JOURNAL_SECTOR,
        JOURNAL_SECTORS,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_VERTEXFS,
        VERTEXFS_IMAGE_SECTOR,
        VERTEXFS_IMAGE_SECTORS,
    );
    write_section(
        sector,
        vdisk_abi::SECTION_GRAPH_STORE,
        GRAPH_STORE_SECTOR,
        GRAPH_STORE_SECTORS,
    );
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
        for service in &manifest.services {
            for config in &service.configs {
                required.insert(config.clone());
            }
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
    for manifest in manifests {
        objects.push(StorePayload {
            id: generation_manifest_store_id(&manifest.generation.id)?,
            bytes: krustboot::compile(manifest)?,
            sector: 0,
        });
    }
    Ok(objects)
}

fn generation_manifest_store_id(generation_id: &str) -> Result<String, String> {
    let id = format!("store:krustboot:{generation_id}");
    if id.as_bytes().len() > 64 {
        return Err(format!(
            "VertexDisk generation manifest store id {id} exceeds 64 bytes"
        ));
    }
    Ok(id)
}

fn store_payload(manifests: &[GenerationManifest], id: &str) -> Result<StorePayload, String> {
    let store = manifests
        .iter()
        .find_map(|manifest| manifest.store_object(id))
        .ok_or_else(|| format!("VertexDisk store object {id} missing from manifests"))?;
    let bytes = match store.kind.as_str() {
        "config" => native_store_bytes(&store.name)?,
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
        "store-vertexfs-fsync-fault-token" => Ok(VERTEXFS_FSYNC_FAULT_OBJECT.to_vec()),
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

fn write_vertexfs_image(image: &mut [u8], manifest: &GenerationManifest) -> Result<(), String> {
    let vertexfs_image = vertexfs::create_image(manifest)?;
    let expected_len = VERTEXFS_IMAGE_SECTORS as usize * SECTOR_SIZE;
    if vertexfs_image.len() != expected_len {
        return Err(format!(
            "VertexDisk VertexFS section expected {expected_len} bytes, builder returned {}",
            vertexfs_image.len()
        ));
    }
    write_bytes(image, VERTEXFS_IMAGE_SECTOR, &vertexfs_image);
    Ok(())
}

fn write_generation_metadata(
    image: &mut [u8],
    manifests: &[GenerationManifest],
) -> Result<(), String> {
    let capacity = (SECTOR_SIZE - GENERATION_METADATA_ENTRY_OFFSET) / GENERATION_METADATA_ENTRY_LEN;
    if manifests.len() > capacity || manifests.len() > u16::MAX as usize {
        return Err(format!(
            "VertexDisk generation metadata holds at most {capacity} generations"
        ));
    }

    let sector = sector_mut(image, GENERATION_METADATA_SECTOR);
    sector[..GENERATION_METADATA_MAGIC.len()].copy_from_slice(GENERATION_METADATA_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(
        sector,
        GENERATION_METADATA_COUNT_OFFSET,
        manifests.len() as u16,
    );
    write_u16(
        sector,
        GENERATION_METADATA_TRANSACTION_STATE_OFFSET,
        GENERATION_TRANSACTION_CLEAN,
    );
    write_u16(
        sector,
        GENERATION_METADATA_FAILURE_REASON_OFFSET,
        GENERATION_FAILURE_NONE,
    );
    write_fixed_str(
        sector,
        GENERATION_METADATA_SELECTED_OFFSET,
        &manifests[0].generation.id,
    );
    write_fixed_str(
        sector,
        GENERATION_METADATA_KNOWN_GOOD_OFFSET,
        &manifests[0].generation.id,
    );
    for (index, manifest) in manifests.iter().enumerate() {
        let offset = GENERATION_METADATA_ENTRY_OFFSET + index * GENERATION_METADATA_ENTRY_LEN;
        write_fixed_str(sector, offset, &manifest.generation.id);
    }
    write_checksum(sector);
    Ok(())
}

fn write_generation_transaction_checkpoint(
    image: &mut [u8],
    transaction_state: u16,
    failure_reason: u16,
) -> Result<(), String> {
    let sector = sector_mut(image, GENERATION_METADATA_SECTOR);
    if !sector.starts_with(GENERATION_METADATA_MAGIC) {
        return Err("VertexDisk generation metadata missing".to_owned());
    }
    let count = read_u16(sector, GENERATION_METADATA_COUNT_OFFSET) as usize;
    if count < 2 {
        return Err(
            "VertexDisk generation transaction checkpoint requires two generations".to_owned(),
        );
    }
    let selected = fixed_string_at(sector, GENERATION_METADATA_ENTRY_OFFSET)?;
    let candidate = fixed_string_at(
        sector,
        GENERATION_METADATA_ENTRY_OFFSET + GENERATION_METADATA_ENTRY_LEN,
    )?;
    write_u16(
        sector,
        GENERATION_METADATA_TRANSACTION_STATE_OFFSET,
        transaction_state,
    );
    write_u16(
        sector,
        GENERATION_METADATA_FAILURE_REASON_OFFSET,
        failure_reason,
    );
    match transaction_state {
        GENERATION_TRANSACTION_PREPARE => {
            write_fixed_str(sector, GENERATION_METADATA_SELECTED_OFFSET, &selected);
            write_fixed_str(sector, GENERATION_METADATA_PREVIOUS_OFFSET, &selected);
            write_fixed_str(sector, GENERATION_METADATA_KNOWN_GOOD_OFFSET, &selected);
            write_fixed_str(
                sector,
                GENERATION_METADATA_TRANSACTION_TARGET_OFFSET,
                &candidate,
            );
        }
        GENERATION_TRANSACTION_COMMIT => {
            write_fixed_str(sector, GENERATION_METADATA_SELECTED_OFFSET, &candidate);
            write_fixed_str(sector, GENERATION_METADATA_PREVIOUS_OFFSET, &selected);
            write_fixed_str(sector, GENERATION_METADATA_KNOWN_GOOD_OFFSET, &selected);
            write_fixed_str(
                sector,
                GENERATION_METADATA_TRANSACTION_TARGET_OFFSET,
                &candidate,
            );
        }
        GENERATION_TRANSACTION_ROLLBACK => {
            write_fixed_str(sector, GENERATION_METADATA_SELECTED_OFFSET, &selected);
            write_fixed_str(sector, GENERATION_METADATA_PREVIOUS_OFFSET, &candidate);
            write_fixed_str(sector, GENERATION_METADATA_KNOWN_GOOD_OFFSET, &selected);
            write_fixed_str(
                sector,
                GENERATION_METADATA_TRANSACTION_TARGET_OFFSET,
                &selected,
            );
        }
        other => {
            return Err(format!(
                "unsupported generation transaction checkpoint state {other}"
            ));
        }
    }
    write_checksum(sector);
    Ok(())
}

fn write_graph_stores(
    image: &mut [u8],
    graph_stores: &[krustboot::GraphStoreImage],
) -> Result<(), String> {
    let mut cursor = GRAPH_STORE_SECTOR;
    let limit = GRAPH_STORE_SECTOR + GRAPH_STORE_SECTORS;
    for graph_store in graph_stores {
        if graph_store.node_count > u16::MAX as usize || graph_store.edge_count > u16::MAX as usize
        {
            return Err("VertexDisk graph store count exceeds u16".to_owned());
        }
        if graph_store.records.len() > u32::MAX as usize {
            return Err("VertexDisk graph store byte length exceeds u32".to_owned());
        }
        let data_sectors = sectors_for_len(graph_store.records.len()) as u64;
        let object_sectors = 1u64
            .checked_add(data_sectors)
            .ok_or_else(|| "VertexDisk graph store sector count overflow".to_owned())?;
        if cursor
            .checked_add(object_sectors)
            .is_none_or(|end| end > limit)
        {
            return Err(format!(
                "VertexDisk graph stores require more than {GRAPH_STORE_SECTORS} sectors"
            ));
        }

        vdisk_abi::write_graph_store_header(
            sector_mut(image, cursor),
            vdisk_abi::GraphStoreHeaderFields {
                generation_id: &graph_store.generation_id,
                node_count: graph_store.node_count,
                edge_count: graph_store.edge_count,
                data_sector: cursor + 1,
                byte_len: graph_store.records.len(),
                record_checksum: graph_store.checksum,
                hash: &graph_store.hash,
            },
        )
        .map_err(|error| format!("VertexDisk graph store header invalid: {error:?}"))?;
        write_bytes(image, cursor + 1, &graph_store.records);
        cursor += object_sectors;
    }
    Ok(())
}

struct StateEntry {
    id: String,
    data_sector: u64,
    value_len: u32,
    checksum: u32,
}

fn state_entries(manifests: &[GenerationManifest]) -> Result<Vec<StateEntry>, String> {
    let mut ids = BTreeSet::new();
    for manifest in manifests {
        for state in &manifest.state_volumes {
            ids.insert(state.id.clone());
        }
    }

    let index_capacity = (SECTOR_SIZE - STATE_ENTRY_OFFSET) / STATE_ENTRY_LEN;
    if ids.len() > index_capacity {
        return Err(format!(
            "VertexDisk state index holds at most {index_capacity} volumes"
        ));
    }
    if ids.len() > KRUST_STATE_VOLUME_LIMIT {
        return Err(format!(
            "Krust native runtime supports at most {KRUST_STATE_VOLUME_LIMIT} state volumes"
        ));
    }
    if ids.len() as u64 > STATE_DATA_SECTORS {
        return Err(format!(
            "VertexDisk state volumes require {} sectors, capacity is {}",
            ids.len(),
            STATE_DATA_SECTORS
        ));
    }

    let mut entries = Vec::new();
    for (index, id) in ids.into_iter().enumerate() {
        if !id.starts_with("state:") {
            return Err(format!(
                "VertexDisk state volume {id} must use state: namespace"
            ));
        }
        if id.as_bytes().len() > 64 {
            return Err(format!("VertexDisk state volume id {id} exceeds 64 bytes"));
        }
        entries.push(StateEntry {
            id,
            data_sector: STATE_DATA_SECTOR + index as u64,
            value_len: 0,
            checksum: 0,
        });
    }
    Ok(entries)
}

fn write_state_index(image: &mut [u8], entries: &[StateEntry]) -> Result<(), String> {
    let sector = sector_mut(image, STATE_INDEX_SECTOR);
    sector[..STATE_INDEX_MAGIC.len()].copy_from_slice(STATE_INDEX_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, entries.len() as u16);
    for (index, entry) in entries.iter().enumerate() {
        let offset = STATE_ENTRY_OFFSET + index * STATE_ENTRY_LEN;
        if offset + STATE_ENTRY_LEN > sector.len() {
            return Err(format!(
                "VertexDisk state index holds at most {} volumes",
                (sector.len() - STATE_ENTRY_OFFSET) / STATE_ENTRY_LEN
            ));
        }
        write_fixed_str(sector, offset, &entry.id);
        write_u64(sector, offset + 64, entry.data_sector);
        write_u32(sector, offset + 72, 1);
        write_u32(sector, offset + 76, entry.value_len);
        write_u32(sector, offset + 80, entry.checksum);
    }
    write_checksum(sector);
    Ok(())
}

fn write_state_journal(image: &mut [u8], value: &[u8]) -> Result<(), String> {
    if value.len() > SECTOR_SIZE - JOURNAL_VALUE_OFFSET {
        return Err("VertexDisk state journal value too large".to_owned());
    }
    let (state_id, data_sector) = first_state_index_entry(image)?;
    let sector = sector_mut(image, JOURNAL_SECTOR);
    sector.fill(0);
    sector[..JOURNAL_RECORD_MAGIC.len()].copy_from_slice(JOURNAL_RECORD_MAGIC);
    write_u16(sector, 16, VERSION);
    write_u16(sector, 18, JOURNAL_RECORD_STATE_WRITE);
    write_u64(sector, 24, STATE_INDEX_SECTOR);
    write_u64(sector, 32, data_sector);
    write_u32(sector, 40, value.len() as u32);
    write_u32(sector, 44, checksum32(value));
    write_fixed_str(sector, JOURNAL_STATE_ID_OFFSET, &state_id);
    sector[JOURNAL_VALUE_OFFSET..JOURNAL_VALUE_OFFSET + value.len()].copy_from_slice(value);
    write_checksum(sector);
    Ok(())
}

fn first_state_index_entry(image: &[u8]) -> Result<(String, u64), String> {
    let offset = STATE_INDEX_SECTOR as usize * SECTOR_SIZE;
    let index = image
        .get(offset..offset + SECTOR_SIZE)
        .ok_or_else(|| "VertexDisk state index section missing".to_owned())?;
    if !index.starts_with(STATE_INDEX_MAGIC) {
        return Err("VertexDisk state index missing".to_owned());
    }
    if read_u16(index, 18) == 0 {
        return Err("VertexDisk state journal requires a state volume".to_owned());
    }
    let id = fixed_string_at(index, STATE_ENTRY_OFFSET)?;
    let data_sector = read_u64(index, STATE_ENTRY_OFFSET + 64);
    Ok((id, data_sector))
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
    buffer[offset..offset + 64].fill(0);
    buffer[offset..offset + len].copy_from_slice(&bytes[..len]);
}

fn native_store_bytes(module_string: &str) -> Result<Vec<u8>, String> {
    if module_string == LOGD_CONFIG_MODULE {
        return Ok(LOGD_CONFIG_BYTES.to_vec());
    }

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
    if module_string == "package-fragment-logd" {
        return vec![
            PathBuf::from("assets/package-fragment-logd.txt"),
            PathBuf::from("kernel/krust/assets/package-fragment-logd.txt"),
        ];
    }
    let crate_dir = if module_string == "vertex-init" {
        "init"
    } else {
        module_string
    };
    vec![
        PathBuf::from(format!(
            "user/target/x86_64-unknown-none/debug/{module_string}"
        )),
        PathBuf::from(format!(
            "kernel/krust/user/target/x86_64-unknown-none/debug/{module_string}"
        )),
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

fn fixed_string_at(buffer: &[u8], offset: usize) -> Result<String, String> {
    if offset + 64 > buffer.len() {
        return Err("VertexDisk fixed string bounds invalid".to_owned());
    }
    let mut len = 0;
    while len < 64 && buffer[offset + len] != 0 {
        len += 1;
    }
    if len == 0 {
        return Err("VertexDisk fixed string is empty".to_owned());
    }
    core::str::from_utf8(&buffer[offset..offset + len])
        .map(|value| value.to_owned())
        .map_err(|_| "VertexDisk fixed string is not UTF-8".to_owned())
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
