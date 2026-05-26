use crate::serial;

const MAX_OBJECTS: usize = 16;
const MAX_CAPABILITIES: usize = 32;

pub const RIGHT_READ: u64 = 1 << 0;
pub const RIGHT_WRITE: u64 = 1 << 1;
pub const RIGHT_MAP: u64 = 1 << 2;
pub const RIGHT_EXECUTE: u64 = 1 << 3;
pub const RIGHT_SEND: u64 = 1 << 4;
pub const RIGHT_RECEIVE: u64 = 1 << 5;
pub const RIGHT_CONTROL: u64 = 1 << 6;
pub const RIGHT_ALLOCATE: u64 = 1 << 7;
pub const RIGHT_SNAPSHOT: u64 = 1 << 8;
pub const RIGHT_RESTORE: u64 = 1 << 9;
pub const RIGHT_BIND: u64 = 1 << 10;
pub const RIGHT_LISTEN: u64 = 1 << 11;
pub const RIGHT_DELEGATE: u64 = 1 << 12;
pub const RIGHT_REVOKE: u64 = 1 << 13;
pub const RIGHT_INSPECT: u64 = 1 << 14;
pub const RIGHT_CREATE: u64 = 1 << 15;
pub const RIGHT_START: u64 = 1 << 16;
pub const RIGHT_KILL: u64 = 1 << 17;
pub const RIGHT_WAIT: u64 = 1 << 18;
pub const RIGHT_DERIVE: u64 = 1 << 19;
pub const RIGHT_SEAL: u64 = 1 << 20;
pub const RIGHT_UNSEAL: u64 = 1 << 21;
pub const RIGHT_INSPECT_METADATA: u64 = 1 << 22;

#[derive(Clone, Copy)]
pub struct ObjectId(u64);

impl ObjectId {
    pub fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub enum KernelObjectKind {
    MemoryObject,
    IpcEndpoint,
    Process,
    Thread,
    BootModule,
    IoPortRange,
    InterruptLine,
    DmaRegion,
}

impl KernelObjectKind {
    fn name(self) -> &'static str {
        match self {
            Self::MemoryObject => "MemoryObject",
            Self::IpcEndpoint => "IpcEndpoint",
            Self::Process => "Process",
            Self::Thread => "Thread",
            Self::BootModule => "BootModule",
            Self::IoPortRange => "IoPortRange",
            Self::InterruptLine => "InterruptLine",
            Self::DmaRegion => "DmaRegion",
        }
    }
}

#[derive(Clone, Copy)]
pub struct KernelObject {
    id: ObjectId,
    kind: KernelObjectKind,
    label: &'static str,
    base: u64,
    length: u64,
}

#[derive(Clone, Copy)]
pub struct Capability {
    slot: usize,
    object_id: ObjectId,
    rights: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
    ObjectTableFull,
    CapabilityTableFull,
}

pub struct CapabilityTable {
    objects: [Option<KernelObject>; MAX_OBJECTS],
    object_count: usize,
    capabilities: [Option<Capability>; MAX_CAPABILITIES],
    capability_count: usize,
    next_object_id: u64,
}

impl CapabilityTable {
    pub const fn new() -> Self {
        Self {
            objects: [None; MAX_OBJECTS],
            object_count: 0,
            capabilities: [None; MAX_CAPABILITIES],
            capability_count: 0,
            next_object_id: 1,
        }
    }

    pub fn add_object(
        &mut self,
        kind: KernelObjectKind,
        label: &'static str,
        base: u64,
        length: u64,
    ) -> Result<ObjectId, TableError> {
        if self.object_count == self.objects.len() {
            return Err(TableError::ObjectTableFull);
        }

        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        self.objects[self.object_count] = Some(KernelObject {
            id,
            kind,
            label,
            base,
            length,
        });
        self.object_count += 1;
        Ok(id)
    }

    pub fn grant(&mut self, object_id: ObjectId, rights: u64) -> Result<usize, TableError> {
        if self.capability_count == self.capabilities.len() {
            return Err(TableError::CapabilityTableFull);
        }

        let slot = self.capability_count;
        self.capabilities[slot] = Some(Capability {
            slot,
            object_id,
            rights,
        });
        self.capability_count += 1;
        Ok(slot)
    }

    pub fn object_count(&self) -> usize {
        self.object_count
    }

    pub fn capability_count(&self) -> usize {
        self.capability_count
    }

    pub fn print(&self) {
        serial::write_str("Kernel object table entries: ");
        serial::write_u64_dec(self.object_count as u64);
        serial::write_str("\n");

        let mut index = 0;
        while index < self.object_count {
            if let Some(object) = self.objects[index] {
                serial::write_str("  obj#");
                serial::write_u64_dec(object.id.raw());
                serial::write_str(" kind=");
                serial::write_str(object.kind.name());
                serial::write_str(" label=");
                serial::write_str(object.label);
                serial::write_str(" base=");
                serial::write_u64_hex(object.base);
                serial::write_str(" length=");
                serial::write_u64_hex(object.length);
                serial::write_str("\n");
            }
            index += 1;
        }

        serial::write_str("Boot capability table entries: ");
        serial::write_u64_dec(self.capability_count as u64);
        serial::write_str("\n");

        let mut index = 0;
        while index < self.capability_count {
            if let Some(capability) = self.capabilities[index] {
                serial::write_str("  cap[");
                serial::write_u64_dec(capability.slot as u64);
                serial::write_str("] object=");
                serial::write_u64_dec(capability.object_id.raw());
                serial::write_str(" rights=");
                print_rights(capability.rights);
                serial::write_str("\n");
            }
            index += 1;
        }
    }
}

fn print_rights(rights: u64) {
    let mut wrote = false;
    wrote = print_right(rights, RIGHT_READ, "read", wrote);
    wrote = print_right(rights, RIGHT_WRITE, "write", wrote);
    wrote = print_right(rights, RIGHT_MAP, "map", wrote);
    wrote = print_right(rights, RIGHT_EXECUTE, "execute", wrote);
    wrote = print_right(rights, RIGHT_SEND, "send", wrote);
    wrote = print_right(rights, RIGHT_RECEIVE, "receive", wrote);
    wrote = print_right(rights, RIGHT_CONTROL, "control", wrote);
    wrote = print_right(rights, RIGHT_ALLOCATE, "allocate", wrote);
    wrote = print_right(rights, RIGHT_SNAPSHOT, "snapshot", wrote);
    wrote = print_right(rights, RIGHT_RESTORE, "restore", wrote);
    wrote = print_right(rights, RIGHT_BIND, "bind", wrote);
    wrote = print_right(rights, RIGHT_LISTEN, "listen", wrote);
    wrote = print_right(rights, RIGHT_DELEGATE, "delegate", wrote);
    wrote = print_right(rights, RIGHT_REVOKE, "revoke", wrote);
    wrote = print_right(rights, RIGHT_INSPECT, "inspect", wrote);
    wrote = print_right(rights, RIGHT_CREATE, "create", wrote);
    wrote = print_right(rights, RIGHT_START, "start", wrote);
    wrote = print_right(rights, RIGHT_KILL, "kill", wrote);
    wrote = print_right(rights, RIGHT_WAIT, "wait", wrote);
    wrote = print_right(rights, RIGHT_DERIVE, "derive", wrote);
    wrote = print_right(rights, RIGHT_SEAL, "seal", wrote);
    wrote = print_right(rights, RIGHT_UNSEAL, "unseal", wrote);
    wrote = print_right(rights, RIGHT_INSPECT_METADATA, "inspect-metadata", wrote);

    if !wrote {
        serial::write_str("none");
    }
}

fn print_right(rights: u64, right: u64, label: &str, wrote: bool) -> bool {
    if rights & right == 0 {
        return wrote;
    }

    if wrote {
        serial::write_str("|");
    }
    serial::write_str(label);
    true
}
