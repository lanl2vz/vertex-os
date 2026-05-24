use core::{cell::UnsafeCell, str};

pub const MODULE_STRING: &[u8] = b"krustboot-manifest";
pub const FALLBACK_MODULE_STRING: &[u8] = b"krustboot-fallback-manifest";
pub const BAD_GENERATION_MODULE_STRING: &[u8] = b"krustboot-bad-generation-manifest";

const COMPACT_MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const COMPACT_VERSION: u16 = 4;
const V1_MAGIC: &[u8; 16] = b"KRUSTBOOTV1\0\0\0\0\0";
const V1_VERSION: u16 = 1;
const V1_HEADER_SIZE: usize = 164;
const V1_CHECKSUM_OFFSET: usize = 32;
const V1_RECORD_SIZE: usize = 12;
const V1_RECORD_COUNT: usize = 9;
const V1_PAYLOAD_OFFSET: usize = V1_HEADER_SIZE + V1_RECORD_COUNT * V1_RECORD_SIZE;
const STRING_LEN: usize = 64;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 64;
const MAX_STORE_OBJECTS: usize = 4;
const MAX_STATE_VOLUMES: usize = 4;
const MAX_NETWORK_PORTS: usize = 4;
const MAX_IO_PORT_RANGES: usize = 4;
const MAX_MMIO_REGIONS: usize = 4;
const MAX_INTERRUPT_LINES: usize = 4;
const MAX_DMA_REGIONS: usize = 4;
pub const MAX_PROCESS_REFS: usize = 4;

pub const RIGHT_SEND: u16 = 1 << 0;
pub const RIGHT_RECEIVE: u16 = 1 << 1;
pub const RIGHT_READ: u16 = 1 << 2;
pub const RIGHT_WRITE: u16 = 1 << 3;
pub const RIGHT_SNAPSHOT: u16 = 1 << 4;
pub const RIGHT_RESTORE: u16 = 1 << 5;
pub const RIGHT_CONTROL: u16 = 1 << 6;
pub const RIGHT_BIND: u16 = 1 << 7;
pub const RIGHT_LISTEN: u16 = 1 << 8;
pub const RIGHT_MAP: u16 = 1 << 9;

pub const OBJECT_ENDPOINT: u16 = 1;
pub const OBJECT_STORE: u16 = 2;
pub const OBJECT_STATE: u16 = 3;
pub const OBJECT_TIMER: u16 = 4;
pub const OBJECT_NETWORK_PORT: u16 = 5;
pub const OBJECT_IO_PORT_RANGE: u16 = 6;
pub const OBJECT_MMIO_REGION: u16 = 7;
pub const OBJECT_INTERRUPT_LINE: u16 = 8;
pub const OBJECT_DMA_REGION: u16 = 9;

#[derive(Clone, Copy)]
pub struct BootModule<'a> {
    pub name: &'a str,
    pub module_string: &'a str,
}

#[derive(Clone, Copy)]
pub struct Process<'a> {
    pub name: &'a str,
    pub module_string: &'a str,
    pub initial: bool,
    pub restart_policy: u16,
    pub service_id: &'a str,
    pub health_kind: &'a str,
    pub start_after: [u16; MAX_PROCESS_REFS],
    pub start_after_count: usize,
    pub requires_endpoint: [u16; MAX_PROCESS_REFS],
    pub requires_endpoint_rights: [u16; MAX_PROCESS_REFS],
    pub requires_endpoint_count: usize,
    pub provides_endpoint: [u16; MAX_PROCESS_REFS],
    pub provides_endpoint_count: usize,
}

#[derive(Clone, Copy)]
pub struct Endpoint<'a> {
    pub name: &'a str,
}

#[derive(Clone, Copy)]
pub struct Grant {
    pub process_index: usize,
    pub object_kind: u16,
    pub object_index: usize,
    pub cap_slot: u64,
    pub rights: u16,
}

#[derive(Clone, Copy)]
pub struct StoreObject<'a> {
    pub id: &'a str,
    pub module_string: &'a str,
    pub hash: &'a str,
    pub size: u64,
}

#[derive(Clone, Copy)]
pub struct StateVolume<'a> {
    pub id: &'a str,
}

#[derive(Clone, Copy)]
pub struct NetworkPort<'a> {
    pub id: &'a str,
}

#[derive(Clone, Copy)]
pub struct IoPortRange<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct MmioRegion<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

#[derive(Clone, Copy)]
pub struct InterruptLine<'a> {
    pub id: &'a str,
    pub line: u64,
}

#[derive(Clone, Copy)]
pub struct DmaRegion<'a> {
    pub id: &'a str,
    pub base: u64,
    pub length: u64,
}

pub struct Manifest<'a> {
    generation_id: &'a str,
    parent_generation_id: &'a str,
    source_base: u64,
    source_len: u64,
    layout_version: u16,
    record_count: usize,
    boot_modules: [Option<BootModule<'a>>; MAX_BOOT_MODULES],
    boot_module_count: usize,
    processes: [Option<Process<'a>>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<Endpoint<'a>>; MAX_ENDPOINTS],
    endpoint_count: usize,
    grants: [Option<Grant>; MAX_GRANTS],
    grant_count: usize,
    store_objects: [Option<StoreObject<'a>>; MAX_STORE_OBJECTS],
    store_object_count: usize,
    state_volumes: [Option<StateVolume<'a>>; MAX_STATE_VOLUMES],
    state_volume_count: usize,
    network_ports: [Option<NetworkPort<'a>>; MAX_NETWORK_PORTS],
    network_port_count: usize,
    io_ports: [Option<IoPortRange<'a>>; MAX_IO_PORT_RANGES],
    io_port_count: usize,
    mmio_regions: [Option<MmioRegion<'a>>; MAX_MMIO_REGIONS],
    mmio_region_count: usize,
    interrupt_lines: [Option<InterruptLine<'a>>; MAX_INTERRUPT_LINES],
    interrupt_line_count: usize,
    dma_regions: [Option<DmaRegion<'a>>; MAX_DMA_REGIONS],
    dma_region_count: usize,
}

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static SELECTED_MANIFEST: Global<Manifest<'static>> = Global(UnsafeCell::new(Manifest::empty()));
static FALLBACK_MANIFEST: Global<Manifest<'static>> = Global(UnsafeCell::new(Manifest::empty()));
static BAD_GENERATION_MANIFEST: Global<Manifest<'static>> =
    Global(UnsafeCell::new(Manifest::empty()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    Truncated,
    BadMagic,
    UnsupportedVersion,
    TooManyBootModules,
    TooManyProcesses,
    TooManyEndpoints,
    TooManyGrants,
    TooManyStoreObjects,
    TooManyStateVolumes,
    TooManyNetworkPorts,
    TooManyIoPortRanges,
    TooManyMmioRegions,
    TooManyInterruptLines,
    TooManyDmaRegions,
    InvalidString,
    InvalidReference,
    InvalidRights,
    InvalidObjectKind,
    TrailingBytes,
    BadChecksum,
    BadRecordTable,
    OutOfBoundsRecord,
}

impl<'a> Manifest<'a> {
    const fn empty() -> Self {
        Self {
            generation_id: "",
            parent_generation_id: "",
            source_base: 0,
            source_len: 0,
            layout_version: 0,
            record_count: 0,
            boot_modules: [None; MAX_BOOT_MODULES],
            boot_module_count: 0,
            processes: [None; MAX_PROCESSES],
            process_count: 0,
            endpoints: [None; MAX_ENDPOINTS],
            endpoint_count: 0,
            grants: [None; MAX_GRANTS],
            grant_count: 0,
            store_objects: [None; MAX_STORE_OBJECTS],
            store_object_count: 0,
            state_volumes: [None; MAX_STATE_VOLUMES],
            state_volume_count: 0,
            network_ports: [None; MAX_NETWORK_PORTS],
            network_port_count: 0,
            io_ports: [None; MAX_IO_PORT_RANGES],
            io_port_count: 0,
            mmio_regions: [None; MAX_MMIO_REGIONS],
            mmio_region_count: 0,
            interrupt_lines: [None; MAX_INTERRUPT_LINES],
            interrupt_line_count: 0,
            dma_regions: [None; MAX_DMA_REGIONS],
            dma_region_count: 0,
        }
    }

    pub fn generation_id(&self) -> &'a str {
        self.generation_id
    }

    pub fn parent_generation_id(&self) -> &'a str {
        self.parent_generation_id
    }

    pub fn source_base(&self) -> u64 {
        self.source_base
    }

    pub fn source_len(&self) -> u64 {
        self.source_len
    }

    pub fn layout_version(&self) -> u16 {
        self.layout_version
    }

    pub fn record_count(&self) -> usize {
        self.record_count
    }

    pub fn boot_module_count(&self) -> usize {
        self.boot_module_count
    }

    pub fn process_count(&self) -> usize {
        self.process_count
    }

    pub fn endpoint_count(&self) -> usize {
        self.endpoint_count
    }

    pub fn grant_count(&self) -> usize {
        self.grant_count
    }

    pub fn store_object_count(&self) -> usize {
        self.store_object_count
    }

    pub fn state_volume_count(&self) -> usize {
        self.state_volume_count
    }

    pub fn network_port_count(&self) -> usize {
        self.network_port_count
    }

    pub fn io_port_count(&self) -> usize {
        self.io_port_count
    }

    pub fn mmio_region_count(&self) -> usize {
        self.mmio_region_count
    }

    pub fn interrupt_line_count(&self) -> usize {
        self.interrupt_line_count
    }

    pub fn dma_region_count(&self) -> usize {
        self.dma_region_count
    }

    pub fn boot_module(&self, index: usize) -> Option<BootModule<'a>> {
        if index < self.boot_module_count {
            self.boot_modules[index]
        } else {
            None
        }
    }

    pub fn process(&self, index: usize) -> Option<Process<'a>> {
        if index < self.process_count {
            self.processes[index]
        } else {
            None
        }
    }

    pub fn endpoint(&self, index: usize) -> Option<Endpoint<'a>> {
        if index < self.endpoint_count {
            self.endpoints[index]
        } else {
            None
        }
    }

    pub fn grant(&self, index: usize) -> Option<Grant> {
        if index < self.grant_count {
            self.grants[index]
        } else {
            None
        }
    }

    pub fn store_object(&self, index: usize) -> Option<StoreObject<'a>> {
        if index < self.store_object_count {
            self.store_objects[index]
        } else {
            None
        }
    }

    pub fn state_volume(&self, index: usize) -> Option<StateVolume<'a>> {
        if index < self.state_volume_count {
            self.state_volumes[index]
        } else {
            None
        }
    }

    pub fn network_port(&self, index: usize) -> Option<NetworkPort<'a>> {
        if index < self.network_port_count {
            self.network_ports[index]
        } else {
            None
        }
    }

    pub fn io_port(&self, index: usize) -> Option<IoPortRange<'a>> {
        if index < self.io_port_count {
            self.io_ports[index]
        } else {
            None
        }
    }

    pub fn mmio_region(&self, index: usize) -> Option<MmioRegion<'a>> {
        if index < self.mmio_region_count {
            self.mmio_regions[index]
        } else {
            None
        }
    }

    pub fn interrupt_line(&self, index: usize) -> Option<InterruptLine<'a>> {
        if index < self.interrupt_line_count {
            self.interrupt_lines[index]
        } else {
            None
        }
    }

    pub fn dma_region(&self, index: usize) -> Option<DmaRegion<'a>> {
        if index < self.dma_region_count {
            self.dma_regions[index]
        } else {
            None
        }
    }
}

pub fn parse_selected(bytes: &'static [u8]) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &SELECTED_MANIFEST)
}

pub fn parse_fallback(bytes: &'static [u8]) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &FALLBACK_MANIFEST)
}

pub fn parse_bad_generation(
    bytes: &'static [u8],
) -> Result<&'static Manifest<'static>, ParseError> {
    parse_static(bytes, &BAD_GENERATION_MANIFEST)
}

fn parse_static(
    bytes: &'static [u8],
    slot: &'static Global<Manifest<'static>>,
) -> Result<&'static Manifest<'static>, ParseError> {
    let manifest = unsafe { &mut *slot.0.get() };
    parse_into(bytes, manifest)?;
    Ok(unsafe { &*slot.0.get() })
}

fn parse_into(bytes: &'static [u8], manifest: &mut Manifest<'static>) -> Result<(), ParseError> {
    let payload = parse_v1_payload(bytes)?;
    parse_compact_into(payload, manifest)?;
    manifest.layout_version = V1_VERSION;
    manifest.record_count = V1_RECORD_COUNT;
    Ok(())
}

fn parse_v1_payload(bytes: &'static [u8]) -> Result<&'static [u8], ParseError> {
    if bytes.len() < V1_MAGIC.len() {
        return Err(ParseError::Truncated);
    }

    let mut reader = Reader::new(bytes);
    if reader.read_exact(V1_MAGIC.len())? != V1_MAGIC {
        return Err(ParseError::BadMagic);
    }

    if reader.read_u16()? != V1_VERSION {
        return Err(ParseError::UnsupportedVersion);
    }

    let header_size = reader.read_u16()? as usize;
    let total_size = reader.read_u32()? as usize;
    let record_table_offset = reader.read_u32()? as usize;
    let record_count = reader.read_u16()? as usize;
    let _reserved = reader.read_u16()?;
    let checksum = reader.read_u32()?;
    let _generation_id = reader.read_fixed_str()?;
    let _parent_generation_id = reader.read_fixed_str_allow_empty()?;

    if header_size != V1_HEADER_SIZE
        || record_table_offset != V1_HEADER_SIZE
        || record_count != V1_RECORD_COUNT
        || total_size != bytes.len()
        || total_size < V1_PAYLOAD_OFFSET
    {
        return Err(ParseError::BadRecordTable);
    }

    if checksum != v1_checksum(bytes) {
        return Err(ParseError::BadChecksum);
    }

    let mut seen = [false; V1_RECORD_COUNT + 1];
    let mut record_index = 0;
    while record_index < record_count {
        let offset = record_table_offset + record_index * V1_RECORD_SIZE;
        let record = Record::read(bytes, offset)?;
        if record.kind == 0 || record.kind as usize >= seen.len() {
            return Err(ParseError::BadRecordTable);
        }
        if seen[record.kind as usize] {
            return Err(ParseError::BadRecordTable);
        }
        seen[record.kind as usize] = true;
        let end = record
            .offset
            .checked_add(record.length)
            .ok_or(ParseError::OutOfBoundsRecord)?;
        if record.offset > bytes.len() || end > bytes.len() {
            return Err(ParseError::OutOfBoundsRecord);
        }
        record_index += 1;
    }

    let mut kind = 1;
    while kind <= V1_RECORD_COUNT {
        if !seen[kind] {
            return Err(ParseError::BadRecordTable);
        }
        kind += 1;
    }

    Ok(&bytes[V1_PAYLOAD_OFFSET..])
}

fn parse_compact_into(
    bytes: &'static [u8],
    manifest: &mut Manifest<'static>,
) -> Result<(), ParseError> {
    let mut reader = Reader::new(bytes);
    if reader.read_exact(COMPACT_MAGIC.len())? != COMPACT_MAGIC {
        return Err(ParseError::BadMagic);
    }

    if reader.read_u16()? != COMPACT_VERSION {
        return Err(ParseError::UnsupportedVersion);
    }

    let boot_module_count = reader.read_count(MAX_BOOT_MODULES, ParseError::TooManyBootModules)?;
    let process_count = reader.read_count(MAX_PROCESSES, ParseError::TooManyProcesses)?;
    let endpoint_count = reader.read_count(MAX_ENDPOINTS, ParseError::TooManyEndpoints)?;
    let grant_count = reader.read_count(MAX_GRANTS, ParseError::TooManyGrants)?;
    let store_object_count =
        reader.read_count(MAX_STORE_OBJECTS, ParseError::TooManyStoreObjects)?;
    let state_volume_count =
        reader.read_count(MAX_STATE_VOLUMES, ParseError::TooManyStateVolumes)?;
    let network_port_count =
        reader.read_count(MAX_NETWORK_PORTS, ParseError::TooManyNetworkPorts)?;
    let io_port_count = reader.read_count(MAX_IO_PORT_RANGES, ParseError::TooManyIoPortRanges)?;
    let mmio_region_count = reader.read_count(MAX_MMIO_REGIONS, ParseError::TooManyMmioRegions)?;
    let interrupt_line_count =
        reader.read_count(MAX_INTERRUPT_LINES, ParseError::TooManyInterruptLines)?;
    let dma_region_count = reader.read_count(MAX_DMA_REGIONS, ParseError::TooManyDmaRegions)?;
    let generation_id = reader.read_fixed_str()?;
    let parent_generation_id = reader.read_fixed_str_allow_empty()?;

    *manifest = Manifest::empty();
    manifest.generation_id = generation_id;
    manifest.parent_generation_id = parent_generation_id;
    manifest.source_base = bytes.as_ptr() as u64;
    manifest.source_len = bytes.len() as u64;
    manifest.boot_module_count = boot_module_count;
    manifest.process_count = process_count;
    manifest.endpoint_count = endpoint_count;
    manifest.grant_count = grant_count;
    manifest.store_object_count = store_object_count;
    manifest.state_volume_count = state_volume_count;
    manifest.network_port_count = network_port_count;
    manifest.io_port_count = io_port_count;
    manifest.mmio_region_count = mmio_region_count;
    manifest.interrupt_line_count = interrupt_line_count;
    manifest.dma_region_count = dma_region_count;

    let mut index = 0;
    while index < boot_module_count {
        manifest.boot_modules[index] = Some(BootModule {
            name: reader.read_fixed_str()?,
            module_string: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < process_count {
        let name = reader.read_fixed_str()?;
        let module_string = reader.read_fixed_str()?;
        let flags = reader.read_u16()?;
        let restart_policy = reader.read_u16()?;
        let service_id = reader.read_fixed_str()?;
        let health_kind = reader.read_fixed_str_allow_empty()?;
        let (start_after, start_after_count) = reader.read_ref_list()?;
        let (requires_endpoint, requires_endpoint_rights, requires_endpoint_count) =
            reader.read_endpoint_requirement_list()?;
        let (provides_endpoint, provides_endpoint_count) = reader.read_ref_list()?;
        manifest.processes[index] = Some(Process {
            name,
            module_string,
            initial: flags & 1 != 0,
            restart_policy,
            service_id,
            health_kind,
            start_after,
            start_after_count,
            requires_endpoint,
            requires_endpoint_rights,
            requires_endpoint_count,
            provides_endpoint,
            provides_endpoint_count,
        });
        index += 1;
    }

    index = 0;
    while index < endpoint_count {
        manifest.endpoints[index] = Some(Endpoint {
            name: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < grant_count {
        let process_index = reader.read_u16()? as usize;
        let object_kind = reader.read_u16()?;
        let object_index = reader.read_u16()? as usize;
        let cap_slot = reader.read_u16()? as u64;
        let rights = reader.read_u16()?;
        let _reserved = reader.read_u16()?;
        manifest.grants[index] = Some(Grant {
            process_index,
            object_kind,
            object_index,
            cap_slot,
            rights,
        });
        index += 1;
    }

    index = 0;
    while index < store_object_count {
        manifest.store_objects[index] = Some(StoreObject {
            id: reader.read_fixed_str()?,
            module_string: reader.read_fixed_str()?,
            hash: reader.read_fixed_str()?,
            size: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < state_volume_count {
        manifest.state_volumes[index] = Some(StateVolume {
            id: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < network_port_count {
        manifest.network_ports[index] = Some(NetworkPort {
            id: reader.read_fixed_str()?,
        });
        index += 1;
    }

    index = 0;
    while index < io_port_count {
        manifest.io_ports[index] = Some(IoPortRange {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < mmio_region_count {
        manifest.mmio_regions[index] = Some(MmioRegion {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < interrupt_line_count {
        manifest.interrupt_lines[index] = Some(InterruptLine {
            id: reader.read_fixed_str()?,
            line: reader.read_u64()?,
        });
        index += 1;
    }

    index = 0;
    while index < dma_region_count {
        manifest.dma_regions[index] = Some(DmaRegion {
            id: reader.read_fixed_str()?,
            base: reader.read_u64()?,
            length: reader.read_u64()?,
        });
        index += 1;
    }

    validate_manifest(manifest)?;

    if !reader.finished() {
        return Err(ParseError::TrailingBytes);
    }

    Ok(())
}

fn validate_manifest(manifest: &Manifest<'_>) -> Result<(), ParseError> {
    let mut initial_count = 0;
    let mut index = 0;
    while index < manifest.process_count {
        let process = manifest
            .process(index)
            .ok_or(ParseError::InvalidReference)?;
        if process.initial {
            initial_count += 1;
        }
        if !has_boot_module(manifest, process.module_string) {
            return Err(ParseError::InvalidReference);
        }
        validate_process_refs(
            process.start_after,
            process.start_after_count,
            manifest.process_count,
        )?;
        validate_process_refs(
            process.requires_endpoint,
            process.requires_endpoint_count,
            manifest.endpoint_count,
        )?;
        validate_endpoint_rights(
            process.requires_endpoint_rights,
            process.requires_endpoint_count,
        )?;
        validate_process_refs(
            process.provides_endpoint,
            process.provides_endpoint_count,
            manifest.endpoint_count,
        )?;
        index += 1;
    }

    if initial_count != 1 {
        return Err(ParseError::InvalidReference);
    }

    index = 0;
    while index < manifest.grant_count {
        let grant = manifest.grant(index).ok_or(ParseError::InvalidReference)?;
        if grant.process_index >= manifest.process_count {
            return Err(ParseError::InvalidReference);
        }
        match grant.object_kind {
            OBJECT_ENDPOINT if grant.object_index < manifest.endpoint_count => {}
            OBJECT_STORE if grant.object_index < manifest.store_object_count => {}
            OBJECT_STATE if grant.object_index < manifest.state_volume_count => {}
            OBJECT_TIMER if grant.object_index == 0 => {}
            OBJECT_NETWORK_PORT if grant.object_index < manifest.network_port_count => {}
            OBJECT_IO_PORT_RANGE if grant.object_index < manifest.io_port_count => {}
            OBJECT_MMIO_REGION if grant.object_index < manifest.mmio_region_count => {}
            OBJECT_INTERRUPT_LINE if grant.object_index < manifest.interrupt_line_count => {}
            OBJECT_DMA_REGION if grant.object_index < manifest.dma_region_count => {}
            OBJECT_ENDPOINT | OBJECT_STORE | OBJECT_STATE | OBJECT_TIMER | OBJECT_NETWORK_PORT => {
                return Err(ParseError::InvalidReference);
            }
            OBJECT_IO_PORT_RANGE
            | OBJECT_MMIO_REGION
            | OBJECT_INTERRUPT_LINE
            | OBJECT_DMA_REGION => {
                return Err(ParseError::InvalidReference);
            }
            _ => return Err(ParseError::InvalidObjectKind),
        }
        if grant.rights == 0
            || grant.rights
                & !(RIGHT_SEND
                    | RIGHT_RECEIVE
                    | RIGHT_READ
                    | RIGHT_WRITE
                    | RIGHT_SNAPSHOT
                    | RIGHT_RESTORE
                    | RIGHT_CONTROL
                    | RIGHT_BIND
                    | RIGHT_LISTEN
                    | RIGHT_MAP)
                != 0
        {
            return Err(ParseError::InvalidRights);
        }
        index += 1;
    }

    Ok(())
}

fn validate_endpoint_rights(
    rights: [u16; MAX_PROCESS_REFS],
    count: usize,
) -> Result<(), ParseError> {
    let mut index = 0;
    while index < count {
        if rights[index] == 0 || rights[index] & !(RIGHT_SEND | RIGHT_RECEIVE) != 0 {
            return Err(ParseError::InvalidRights);
        }
        index += 1;
    }
    Ok(())
}

fn validate_process_refs(
    refs: [u16; MAX_PROCESS_REFS],
    count: usize,
    limit: usize,
) -> Result<(), ParseError> {
    if count > MAX_PROCESS_REFS {
        return Err(ParseError::InvalidReference);
    }

    let mut index = 0;
    while index < count {
        if refs[index] as usize >= limit {
            return Err(ParseError::InvalidReference);
        }
        index += 1;
    }

    Ok(())
}

fn has_boot_module(manifest: &Manifest<'_>, module_string: &str) -> bool {
    let mut index = 0;
    while index < manifest.boot_module_count {
        if let Some(module) = manifest.boot_module(index)
            && module.module_string == module_string
        {
            return true;
        }
        index += 1;
    }
    false
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_count(&mut self, max: usize, error: ParseError) -> Result<usize, ParseError> {
        let count = self.read_u16()? as usize;
        if count > max {
            return Err(error);
        }
        Ok(count)
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, ParseError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_fixed_str(&mut self) -> Result<&'a str, ParseError> {
        let value = self.read_fixed_str_allow_empty()?;
        if value.is_empty() {
            return Err(ParseError::InvalidString);
        }
        Ok(value)
    }

    fn read_fixed_str_allow_empty(&mut self) -> Result<&'a str, ParseError> {
        let bytes = self.read_exact(STRING_LEN)?;
        let mut end = 0;
        while end < bytes.len() && bytes[end] != 0 {
            end += 1;
        }
        str::from_utf8(&bytes[..end]).map_err(|_| ParseError::InvalidString)
    }

    fn read_ref_list(&mut self) -> Result<([u16; MAX_PROCESS_REFS], usize), ParseError> {
        let count = self.read_u16()? as usize;
        if count > MAX_PROCESS_REFS {
            return Err(ParseError::InvalidReference);
        }

        let mut refs = [u16::MAX; MAX_PROCESS_REFS];
        let mut index = 0;
        while index < MAX_PROCESS_REFS {
            refs[index] = self.read_u16()?;
            index += 1;
        }
        Ok((refs, count))
    }

    fn read_endpoint_requirement_list(
        &mut self,
    ) -> Result<([u16; MAX_PROCESS_REFS], [u16; MAX_PROCESS_REFS], usize), ParseError> {
        let count = self.read_u16()? as usize;
        if count > MAX_PROCESS_REFS {
            return Err(ParseError::InvalidReference);
        }

        let mut refs = [u16::MAX; MAX_PROCESS_REFS];
        let mut rights = [0; MAX_PROCESS_REFS];
        let mut index = 0;
        while index < MAX_PROCESS_REFS {
            refs[index] = self.read_u16()?;
            rights[index] = self.read_u16()?;
            index += 1;
        }
        Ok((refs, rights, count))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ParseError> {
        let end = self.offset.checked_add(len).ok_or(ParseError::Truncated)?;
        if end > self.bytes.len() {
            return Err(ParseError::Truncated);
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

struct Record {
    kind: u16,
    offset: usize,
    length: usize,
}

impl Record {
    fn read(bytes: &[u8], offset: usize) -> Result<Self, ParseError> {
        let end = offset
            .checked_add(V1_RECORD_SIZE)
            .ok_or(ParseError::BadRecordTable)?;
        if end > bytes.len() {
            return Err(ParseError::BadRecordTable);
        }

        let kind = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let _id = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
        let raw_offset = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        let raw_length = u32::from_le_bytes([
            bytes[offset + 8],
            bytes[offset + 9],
            bytes[offset + 10],
            bytes[offset + 11],
        ]);

        Ok(Self {
            kind,
            offset: raw_offset as usize,
            length: raw_length as usize,
        })
    }
}

fn v1_checksum(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    let mut index = 0;
    while index < bytes.len() {
        let value = if index >= V1_CHECKSUM_OFFSET && index < V1_CHECKSUM_OFFSET + 4 {
            0
        } else {
            bytes[index]
        };
        hash ^= value as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}
