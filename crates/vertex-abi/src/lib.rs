#![no_std]

pub mod graph {
    use core::str;

    pub const STRING_LEN: usize = 64;
    pub const NODE_RECORD_LEN: usize = 4 + STRING_LEN * 2;
    pub const EDGE_RECORD_LEN: usize = 8 + STRING_LEN;

    pub const NODE_GENERATION: u16 = 1;
    pub const NODE_SERVICE: u16 = 2;
    pub const NODE_ENDPOINT: u16 = 3;
    pub const NODE_STORE_OBJECT: u16 = 4;
    pub const NODE_CONFIG: u16 = 5;
    pub const NODE_STATE_VOLUME: u16 = 6;
    pub const NODE_DEVICE: u16 = 7;
    pub const NODE_NAMESPACE: u16 = 8;
    pub const NODE_VFS_ROOT: u16 = 9;
    pub const NODE_TIMER: u16 = 10;
    pub const NODE_SECRET: u16 = 11;
    pub const NODE_PACKAGE: u16 = 12;

    pub const EDGE_ACTIVATION: u16 = 1;
    pub const EDGE_CAPABILITY: u16 = 2;
    pub const EDGE_MOUNT: u16 = 3;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct NodeRecord<'a> {
        pub kind: u16,
        pub object_kind: u16,
        pub id: &'a str,
        pub label: &'a str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct EdgeRecord<'a> {
        pub kind: u16,
        pub from_index: usize,
        pub to_index: usize,
        pub rights: u16,
        pub id: &'a str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct RecordSet<'a> {
        records: &'a [u8],
        node_count: usize,
        edge_count: usize,
    }

    impl<'a> RecordSet<'a> {
        pub fn new(records: &'a [u8], node_count: usize, edge_count: usize) -> Option<Self> {
            let expected_len = Self::expected_len(node_count, edge_count)?;
            if records.len() != expected_len {
                return None;
            }
            Some(Self {
                records,
                node_count,
                edge_count,
            })
        }

        pub fn expected_len(node_count: usize, edge_count: usize) -> Option<usize> {
            node_count
                .checked_mul(NODE_RECORD_LEN)?
                .checked_add(edge_count.checked_mul(EDGE_RECORD_LEN)?)
        }

        pub fn bytes(self) -> &'a [u8] {
            self.records
        }

        pub fn node_count(self) -> usize {
            self.node_count
        }

        pub fn edge_count(self) -> usize {
            self.edge_count
        }

        pub fn node(self, index: usize) -> Option<NodeRecord<'a>> {
            if index >= self.node_count {
                return None;
            }
            let offset = index.checked_mul(NODE_RECORD_LEN)?;
            let record = self.records.get(offset..offset + NODE_RECORD_LEN)?;
            Some(NodeRecord {
                kind: read_u16(record, 0)?,
                object_kind: read_u16(record, 2)?,
                id: fixed_str_at(record, 4, false)?,
                label: fixed_str_at(record, 4 + STRING_LEN, true)?,
            })
        }

        pub fn edge(self, index: usize) -> Option<EdgeRecord<'a>> {
            if index >= self.edge_count {
                return None;
            }
            let edge_base = self.node_count.checked_mul(NODE_RECORD_LEN)?;
            let offset = edge_base.checked_add(index.checked_mul(EDGE_RECORD_LEN)?)?;
            let record = self.records.get(offset..offset + EDGE_RECORD_LEN)?;
            Some(EdgeRecord {
                kind: read_u16(record, 0)?,
                from_index: read_u16(record, 2)? as usize,
                to_index: read_u16(record, 4)? as usize,
                rights: read_u16(record, 6)?,
                id: fixed_str_at(record, 8, false)?,
            })
        }
    }

    pub fn fixed_str_at(buffer: &[u8], offset: usize, allow_empty: bool) -> Option<&str> {
        if offset + STRING_LEN > buffer.len() {
            return None;
        }
        let mut len = 0;
        while len < STRING_LEN && buffer[offset + len] != 0 {
            len += 1;
        }
        if len == 0 && !allow_empty {
            return None;
        }
        let mut padding = len;
        while padding < STRING_LEN {
            if buffer[offset + padding] != 0 {
                return None;
            }
            padding += 1;
        }
        str::from_utf8(&buffer[offset..offset + len]).ok()
    }

    pub fn fixed_str_eq(buffer: &[u8], offset: usize, value: &[u8]) -> bool {
        if offset + STRING_LEN > buffer.len() || value.len() > STRING_LEN {
            return false;
        }
        let mut index = 0;
        while index < value.len() {
            if buffer[offset + index] != value[index] {
                return false;
            }
            index += 1;
        }
        value.len() == STRING_LEN || buffer[offset + value.len()] == 0
    }

    fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]))
    }
}

pub mod krustboot {
    pub const COMPACT_MAGIC: &[u8; 16] = b"KRUSTBOOTM86\0\0\0\0";
    pub const COMPACT_VERSION: u16 = 19;
    pub const POLICY_VERSION: u16 = 3;
    pub const V1_MAGIC: &[u8; 16] = b"KRUSTBOOTV1\0\0\0\0\0";
    pub const V1_VERSION: u16 = 1;
    pub const V1_HEADER_SIZE: usize = 164;
    pub const V1_CHECKSUM_OFFSET: usize = 32;
    pub const V1_RECORD_SIZE: usize = 12;
    pub const V1_RECORD_COUNT: usize = 9;
    pub const V1_PAYLOAD_OFFSET: usize = V1_HEADER_SIZE + V1_RECORD_COUNT * V1_RECORD_SIZE;

    pub const GRAPH_HEADER_SIZE: usize = 8;
    pub const COMPACT_GRAPH_NODE_COUNT_OFFSET: usize = 178;
    pub const COMPACT_GRAPH_EDGE_COUNT_OFFSET: usize = 180;
    pub const COMPACT_GRAPH_CHECKSUM_OFFSET: usize = 182;
    pub const COMPACT_HEADER_SIZE: usize = 178 + GRAPH_HEADER_SIZE;
}

pub mod vertexdisk {
    use crate::graph;

    pub const MAGIC: &[u8; 16] = b"VERTEXDISKV1\0\0\0\0";
    pub const GENERATION_METADATA_MAGIC: &[u8; 16] = b"VDISKGENMETAV0\0\0";
    pub const STORE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTOREV0\0\0\0\0";
    pub const STATE_INDEX_MAGIC: &[u8; 16] = b"VDISKSTATEV0\0\0\0\0";
    pub const GRAPH_STORE_MAGIC: &[u8; 16] = b"VDISKGRAPHV0\0\0\0\0";
    pub const VERSION: u16 = 3;
    pub const SECTOR_SIZE: usize = 512;
    pub const VERSION_OFFSET: usize = 16;
    pub const GRAPH_STORE_FORMAT_VERSION: u16 = 1;
    pub const GRAPH_STORE_FORMAT_VERSION_OFFSET: usize = 18;
    pub const CHECKSUM_OFFSET: usize = 20;
    pub const TOTAL_SECTORS_OFFSET: usize = 24;
    pub const SECTION_TABLE_OFFSET: usize = 32;
    pub const SECTION_RECORD_LEN: usize = 16;

    pub const SECTION_GENERATION_METADATA: usize = 0;
    pub const SECTION_STORE_INDEX: usize = 1;
    pub const SECTION_STORE_DATA: usize = 2;
    pub const SECTION_STATE_INDEX: usize = 3;
    pub const SECTION_STATE_DATA: usize = 4;
    pub const SECTION_JOURNAL: usize = 5;
    pub const SECTION_VERTEXFS: usize = 6;
    pub const SECTION_GRAPH_STORE: usize = 7;

    pub const GENERATION_METADATA_COUNT_OFFSET: usize = 18;
    pub const GENERATION_METADATA_TRANSACTION_STATE_OFFSET: usize = 24;
    pub const GENERATION_METADATA_FAILURE_REASON_OFFSET: usize = 26;
    pub const GENERATION_METADATA_SELECTED_OFFSET: usize = 32;
    pub const GENERATION_METADATA_PREVIOUS_OFFSET: usize =
        GENERATION_METADATA_SELECTED_OFFSET + graph::STRING_LEN;
    pub const GENERATION_METADATA_KNOWN_GOOD_OFFSET: usize =
        GENERATION_METADATA_PREVIOUS_OFFSET + graph::STRING_LEN;
    pub const GENERATION_METADATA_TRANSACTION_TARGET_OFFSET: usize =
        GENERATION_METADATA_KNOWN_GOOD_OFFSET + graph::STRING_LEN;
    pub const GENERATION_METADATA_ENTRY_OFFSET: usize =
        GENERATION_METADATA_TRANSACTION_TARGET_OFFSET + graph::STRING_LEN;
    pub const GENERATION_METADATA_ENTRY_LEN: usize = graph::STRING_LEN;
    pub const GENERATION_TRANSACTION_CLEAN: u16 = 0;
    pub const GENERATION_TRANSACTION_PREPARE: u16 = 1;
    pub const GENERATION_TRANSACTION_COMMIT: u16 = 2;
    pub const GENERATION_TRANSACTION_ROLLBACK: u16 = 3;
    pub const GENERATION_TRANSACTION_ABORT: u16 = 4;
    pub const GENERATION_FAILURE_NONE: u16 = 0;
    pub const GENERATION_FAILURE_ACTIVATION_FAILED: u16 = 1;
    pub const GENERATION_FAILURE_VERIFICATION_FAILED: u16 = 2;
    pub const GENERATION_FAILURE_RUNTIME_BUILD_FAILED: u16 = 3;
    pub const GENERATION_FAILURE_ROLLBACK_BUILD_FAILED: u16 = 4;

    pub const GRAPH_GENERATION_OFFSET: usize = 32;
    pub const GRAPH_NODE_COUNT_OFFSET: usize = 96;
    pub const GRAPH_EDGE_COUNT_OFFSET: usize = 98;
    pub const GRAPH_DATA_SECTOR_OFFSET: usize = 100;
    pub const GRAPH_BYTE_LEN_OFFSET: usize = 108;
    pub const GRAPH_RECORD_CHECKSUM_OFFSET: usize = 112;
    pub const GRAPH_HASH_OFFSET: usize = 116;

    pub const JOURNAL_RECORD_MAGIC: &[u8; 16] = b"VDISKJOURNALV0\0\0";
    pub const JOURNAL_RECORD_STATE_WRITE: u16 = 1;
    pub const JOURNAL_STATE_ID_OFFSET: usize = 48;
    pub const JOURNAL_VALUE_OFFSET: usize = 128;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum WriteHeaderError {
        BufferTooShort,
        CountTooLarge,
        ByteLenTooLarge,
        FixedStringTooLong,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct GraphStoreHeaderFields<'a> {
        pub generation_id: &'a str,
        pub node_count: usize,
        pub edge_count: usize,
        pub data_sector: u64,
        pub byte_len: usize,
        pub record_checksum: u32,
        pub hash: &'a str,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct GraphStoreHeader<'a> {
        bytes: &'a [u8],
    }

    impl<'a> GraphStoreHeader<'a> {
        pub fn new(bytes: &'a [u8]) -> Option<Self> {
            let bytes = bytes.get(..SECTOR_SIZE)?;
            if !starts_with(bytes, GRAPH_STORE_MAGIC)
                || read_u16(bytes, VERSION_OFFSET)? != VERSION
                || !metadata_checksum_valid(bytes)
            {
                return None;
            }
            Some(Self { bytes })
        }

        pub fn generation_id(self) -> Option<&'a str> {
            graph::fixed_str_at(self.bytes, GRAPH_GENERATION_OFFSET, false)
        }

        pub fn node_count(self) -> usize {
            read_u16(self.bytes, GRAPH_NODE_COUNT_OFFSET).unwrap_or(0) as usize
        }

        pub fn edge_count(self) -> usize {
            read_u16(self.bytes, GRAPH_EDGE_COUNT_OFFSET).unwrap_or(0) as usize
        }

        pub fn data_sector(self) -> u64 {
            read_u64(self.bytes, GRAPH_DATA_SECTOR_OFFSET).unwrap_or(0)
        }

        pub fn byte_len(self) -> Option<usize> {
            read_u32(self.bytes, GRAPH_BYTE_LEN_OFFSET).map(|value| value as usize)
        }

        pub fn record_checksum(self) -> u32 {
            read_u32(self.bytes, GRAPH_RECORD_CHECKSUM_OFFSET).unwrap_or(0)
        }

        pub fn hash(self) -> Option<&'a str> {
            graph::fixed_str_at(self.bytes, GRAPH_HASH_OFFSET, false)
        }
    }

    pub fn write_graph_store_header(
        sector: &mut [u8],
        fields: GraphStoreHeaderFields<'_>,
    ) -> Result<(), WriteHeaderError> {
        if sector.len() < SECTOR_SIZE {
            return Err(WriteHeaderError::BufferTooShort);
        }
        if fields.node_count > u16::MAX as usize || fields.edge_count > u16::MAX as usize {
            return Err(WriteHeaderError::CountTooLarge);
        }
        if fields.byte_len > u32::MAX as usize {
            return Err(WriteHeaderError::ByteLenTooLarge);
        }

        let sector = &mut sector[..SECTOR_SIZE];
        let mut index = 0;
        while index < sector.len() {
            sector[index] = 0;
            index += 1;
        }

        sector[..GRAPH_STORE_MAGIC.len()].copy_from_slice(GRAPH_STORE_MAGIC);
        write_u16(sector, VERSION_OFFSET, VERSION);
        write_u16(
            sector,
            GRAPH_STORE_FORMAT_VERSION_OFFSET,
            GRAPH_STORE_FORMAT_VERSION,
        );
        write_fixed_str(sector, GRAPH_GENERATION_OFFSET, fields.generation_id)?;
        write_u16(sector, GRAPH_NODE_COUNT_OFFSET, fields.node_count as u16);
        write_u16(sector, GRAPH_EDGE_COUNT_OFFSET, fields.edge_count as u16);
        write_u64(sector, GRAPH_DATA_SECTOR_OFFSET, fields.data_sector);
        write_u32(sector, GRAPH_BYTE_LEN_OFFSET, fields.byte_len as u32);
        write_u32(sector, GRAPH_RECORD_CHECKSUM_OFFSET, fields.record_checksum);
        write_fixed_str(sector, GRAPH_HASH_OFFSET, fields.hash)?;
        write_metadata_checksum(sector);
        Ok(())
    }

    pub fn metadata_checksum_valid(bytes: &[u8]) -> bool {
        if bytes.len() < CHECKSUM_OFFSET + 4 {
            return false;
        }
        let Some(stored) = read_u32(bytes, CHECKSUM_OFFSET) else {
            return false;
        };
        let mut checksum = 0u32;
        let mut index = 0;
        while index < bytes.len() {
            let value = if index >= CHECKSUM_OFFSET && index < CHECKSUM_OFFSET + 4 {
                0
            } else {
                bytes[index]
            };
            checksum = checksum.wrapping_add((value as u32).wrapping_mul(index as u32 + 1));
            index += 1;
        }
        checksum == stored
    }

    fn write_metadata_checksum(bytes: &mut [u8]) {
        write_u32(bytes, CHECKSUM_OFFSET, 0);
        let checksum = checksum32(bytes);
        write_u32(bytes, CHECKSUM_OFFSET, checksum);
    }

    fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
        bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix
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

    fn write_fixed_str(
        buffer: &mut [u8],
        offset: usize,
        value: &str,
    ) -> Result<(), WriteHeaderError> {
        let bytes = value.as_bytes();
        if bytes.len() > graph::STRING_LEN {
            return Err(WriteHeaderError::FixedStringTooLong);
        }
        if offset + graph::STRING_LEN > buffer.len() {
            return Err(WriteHeaderError::BufferTooShort);
        }
        let mut index = 0;
        while index < graph::STRING_LEN {
            buffer[offset + index] = 0;
            index += 1;
        }
        buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
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
}

#[cfg(test)]
mod tests {
    use super::vertexdisk::{
        GraphStoreHeader, GraphStoreHeaderFields, SECTOR_SIZE, write_graph_store_header,
    };

    #[test]
    fn graph_store_header_writer_round_trips_and_requires_checksum() {
        let mut sector = [0u8; SECTOR_SIZE];
        write_graph_store_header(
            &mut sector,
            GraphStoreHeaderFields {
                generation_id: "gen:test",
                node_count: 2,
                edge_count: 1,
                data_sector: 42,
                byte_len: 328,
                record_checksum: 1234,
                hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            },
        )
        .unwrap();

        let header = GraphStoreHeader::new(&sector).unwrap();
        assert_eq!(header.generation_id(), Some("gen:test"));
        assert_eq!(header.node_count(), 2);
        assert_eq!(header.edge_count(), 1);
        assert_eq!(header.data_sector(), 42);
        assert_eq!(header.byte_len(), Some(328));
        assert_eq!(header.record_checksum(), 1234);

        sector[32] ^= 1;
        assert!(GraphStoreHeader::new(&sector).is_none());
    }
}
