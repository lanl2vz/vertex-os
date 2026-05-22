use core::str;

pub const MODULE_STRING: &[u8] = b"krustboot-manifest";

const MAGIC: &[u8; 16] = b"KRUSTBOOTV0\0\0\0\0\0";
const VERSION: u16 = 0;
const STRING_LEN: usize = 64;
const MAX_BOOT_MODULES: usize = 4;
const MAX_PROCESSES: usize = 4;
const MAX_ENDPOINTS: usize = 4;
const MAX_GRANTS: usize = 8;

pub const RIGHT_SEND: u16 = 1 << 0;
pub const RIGHT_RECEIVE: u16 = 1 << 1;

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
}

#[derive(Clone, Copy)]
pub struct Endpoint<'a> {
    pub name: &'a str,
}

#[derive(Clone, Copy)]
pub struct Grant {
    pub process_index: usize,
    pub endpoint_index: usize,
    pub cap_slot: u64,
    pub rights: u16,
}

pub struct Manifest<'a> {
    generation_id: &'a str,
    boot_modules: [Option<BootModule<'a>>; MAX_BOOT_MODULES],
    boot_module_count: usize,
    processes: [Option<Process<'a>>; MAX_PROCESSES],
    process_count: usize,
    endpoints: [Option<Endpoint<'a>>; MAX_ENDPOINTS],
    endpoint_count: usize,
    grants: [Option<Grant>; MAX_GRANTS],
    grant_count: usize,
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
    InvalidString,
    InvalidReference,
    InvalidRights,
    TrailingBytes,
}

impl<'a> Manifest<'a> {
    pub fn generation_id(&self) -> &'a str {
        self.generation_id
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
    let generation_id = reader.read_fixed_str()?;

    let mut manifest = Manifest {
        generation_id,
        boot_modules: [None; MAX_BOOT_MODULES],
        boot_module_count,
        processes: [None; MAX_PROCESSES],
        process_count,
        endpoints: [None; MAX_ENDPOINTS],
        endpoint_count,
        grants: [None; MAX_GRANTS],
        grant_count,
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
        let _reserved = reader.read_u16()?;
        manifest.processes[index] = Some(Process {
            name,
            module_string,
            initial: flags & 1 != 0,
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
        let endpoint_index = reader.read_u16()? as usize;
        let cap_slot = reader.read_u16()? as u64;
        let rights = reader.read_u16()?;
        if process_index >= process_count || endpoint_index >= endpoint_count {
            return Err(ParseError::InvalidReference);
        }
        if rights == 0 || rights & !(RIGHT_SEND | RIGHT_RECEIVE) != 0 {
            return Err(ParseError::InvalidRights);
        }
        manifest.grants[index] = Some(Grant {
            process_index,
            endpoint_index,
            cap_slot,
            rights,
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
        index += 1;
    }

    if initial_count != 1 {
        return Err(ParseError::InvalidReference);
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

    fn read_fixed_str(&mut self) -> Result<&'a str, ParseError> {
        let bytes = self.read_exact(STRING_LEN)?;
        let mut end = 0;
        while end < bytes.len() && bytes[end] != 0 {
            end += 1;
        }
        if end == 0 {
            return Err(ParseError::InvalidString);
        }
        str::from_utf8(&bytes[..end]).map_err(|_| ParseError::InvalidString)
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
