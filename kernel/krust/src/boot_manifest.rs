use core::str;

pub const MODULE_STRING: &[u8] = b"krustboot-manifest";

const MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const VERSION: u16 = 2;
const STRING_LEN: usize = 64;
const MAX_BOOT_MODULES: usize = 16;
const MAX_PROCESSES: usize = 16;
const MAX_ENDPOINTS: usize = 16;
const MAX_GRANTS: usize = 64;
const MAX_STORE_OBJECTS: usize = 4;
const MAX_STATE_VOLUMES: usize = 4;
pub const MAX_PROCESS_REFS: usize = 4;

pub const RIGHT_SEND: u16 = 1 << 0;
pub const RIGHT_RECEIVE: u16 = 1 << 1;
pub const RIGHT_READ: u16 = 1 << 2;
pub const RIGHT_WRITE: u16 = 1 << 3;
pub const RIGHT_SNAPSHOT: u16 = 1 << 4;
pub const RIGHT_RESTORE: u16 = 1 << 5;
pub const RIGHT_CONTROL: u16 = 1 << 6;

pub const OBJECT_ENDPOINT: u16 = 1;
pub const OBJECT_STORE: u16 = 2;
pub const OBJECT_STATE: u16 = 3;
pub const OBJECT_TIMER: u16 = 4;

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

pub struct Manifest<'a> {
    generation_id: &'a str,
    parent_generation_id: &'a str,
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
}

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
    InvalidString,
    InvalidReference,
    InvalidRights,
    InvalidObjectKind,
    TrailingBytes,
}

impl<'a> Manifest<'a> {
    pub fn generation_id(&self) -> &'a str {
        self.generation_id
    }

    pub fn parent_generation_id(&self) -> &'a str {
        self.parent_generation_id
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

    pub fn boot_module(&self, index: usize) -> Option<BootModule<'a>> {
        self.boot_modules.get(index).copied().flatten()
    }

    pub fn process(&self, index: usize) -> Option<Process<'a>> {
        self.processes.get(index).copied().flatten()
    }

    pub fn endpoint(&self, index: usize) -> Option<Endpoint<'a>> {
        self.endpoints.get(index).copied().flatten()
    }

    pub fn grant(&self, index: usize) -> Option<Grant> {
        self.grants.get(index).copied().flatten()
    }

    pub fn store_object(&self, index: usize) -> Option<StoreObject<'a>> {
        self.store_objects.get(index).copied().flatten()
    }

    pub fn state_volume(&self, index: usize) -> Option<StateVolume<'a>> {
        self.state_volumes.get(index).copied().flatten()
    }
}

pub fn parse(bytes: &'static [u8]) -> Result<Manifest<'static>, ParseError> {
    let mut reader = Reader::new(bytes);
    if reader.read_exact(MAGIC.len())? != MAGIC {
        return Err(ParseError::BadMagic);
    }

    if reader.read_u16()? != VERSION {
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
    let generation_id = reader.read_fixed_str()?;
    let parent_generation_id = reader.read_fixed_str_allow_empty()?;

    let mut manifest = Manifest {
        generation_id,
        parent_generation_id,
        boot_modules: [None; MAX_BOOT_MODULES],
        boot_module_count,
        processes: [None; MAX_PROCESSES],
        process_count,
        endpoints: [None; MAX_ENDPOINTS],
        endpoint_count,
        grants: [None; MAX_GRANTS],
        grant_count,
        store_objects: [None; MAX_STORE_OBJECTS],
        store_object_count,
        state_volumes: [None; MAX_STATE_VOLUMES],
        state_volume_count,
    };

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
        let (requires_endpoint, requires_endpoint_count) = reader.read_ref_list()?;
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

    validate_manifest(&manifest)?;

    if !reader.finished() {
        return Err(ParseError::TrailingBytes);
    }

    Ok(manifest)
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
            OBJECT_ENDPOINT | OBJECT_STORE | OBJECT_STATE | OBJECT_TIMER => {
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
                    | RIGHT_CONTROL)
                != 0
        {
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
