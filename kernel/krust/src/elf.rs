pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1 << 0;
pub const PF_W: u32 = 1 << 1;

const ELF_MAGIC: &[u8; 4] = b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 0x3e;
const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: u16 = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    UnsupportedClass,
    UnsupportedEndian,
    UnsupportedVersion,
    UnsupportedType,
    UnsupportedMachine,
    BadProgramHeaders,
}

pub struct Elf<'a> {
    bytes: &'a [u8],
    entry: u64,
    phoff: u64,
    phentsize: u16,
    phnum: u16,
}

#[derive(Clone, Copy)]
pub struct ProgramHeader {
    pub typ: u32,
    pub flags: u32,
    pub offset: u64,
    pub vaddr: u64,
    pub filesz: u64,
    pub memsz: u64,
}

impl<'a> Elf<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ElfError> {
        if bytes.len() < ELF_HEADER_SIZE {
            return Err(ElfError::TooSmall);
        }
        if &bytes[0..4] != ELF_MAGIC {
            return Err(ElfError::BadMagic);
        }
        if bytes[4] != ELFCLASS64 {
            return Err(ElfError::UnsupportedClass);
        }
        if bytes[5] != ELFDATA2LSB {
            return Err(ElfError::UnsupportedEndian);
        }
        if bytes[6] != EV_CURRENT {
            return Err(ElfError::UnsupportedVersion);
        }
        if read_u16(bytes, 16).ok_or(ElfError::TooSmall)? != ET_EXEC {
            return Err(ElfError::UnsupportedType);
        }
        if read_u16(bytes, 18).ok_or(ElfError::TooSmall)? != EM_X86_64 {
            return Err(ElfError::UnsupportedMachine);
        }

        let entry = read_u64(bytes, 24).ok_or(ElfError::TooSmall)?;
        let phoff = read_u64(bytes, 32).ok_or(ElfError::TooSmall)?;
        let phentsize = read_u16(bytes, 54).ok_or(ElfError::TooSmall)?;
        let phnum = read_u16(bytes, 56).ok_or(ElfError::TooSmall)?;

        if phentsize < PROGRAM_HEADER_SIZE {
            return Err(ElfError::BadProgramHeaders);
        }

        let ph_table_size = (phentsize as u64)
            .checked_mul(phnum as u64)
            .ok_or(ElfError::BadProgramHeaders)?;
        let ph_table_end = phoff
            .checked_add(ph_table_size)
            .ok_or(ElfError::BadProgramHeaders)?;
        if ph_table_end > bytes.len() as u64 {
            return Err(ElfError::BadProgramHeaders);
        }

        Ok(Self {
            bytes,
            entry,
            phoff,
            phentsize,
            phnum,
        })
    }

    pub fn entry(&self) -> u64 {
        self.entry
    }

    pub fn program_header_count(&self) -> u16 {
        self.phnum
    }

    pub fn program_header(&self, index: u16) -> Option<ProgramHeader> {
        if index >= self.phnum {
            return None;
        }

        let offset = self.phoff + (index as u64 * self.phentsize as u64);
        let offset = usize::try_from(offset).ok()?;

        Some(ProgramHeader {
            typ: read_u32(self.bytes, offset)?,
            flags: read_u32(self.bytes, offset + 4)?,
            offset: read_u64(self.bytes, offset + 8)?,
            vaddr: read_u64(self.bytes, offset + 16)?,
            filesz: read_u64(self.bytes, offset + 32)?,
            memsz: read_u64(self.bytes, offset + 40)?,
        })
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
        *bytes.get(offset + 4)?,
        *bytes.get(offset + 5)?,
        *bytes.get(offset + 6)?,
        *bytes.get(offset + 7)?,
    ]))
}
