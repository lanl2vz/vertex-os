mod handle;
mod namespace;
mod node;
mod path;
mod state;
mod storage;
mod vertexfs;

pub(crate) use handle::{
    FileDescriptionId, FileHandle, FileHandleSlot, OpenFileDescription, VfsEvent, VfsLock,
    VfsLockMode,
};
pub(crate) use namespace::{
    MAX_NAMESPACE_ENTRIES, NamespaceEntry, NamespaceObject, VfsMountObject, VfsRootObject,
    vfs_authority_path_covers,
};
pub(crate) use node::{VfsBacking, VfsNode, VfsNodeKind};
pub(crate) use path::{
    MAX_VFS_NAME_BYTES, MAX_VFS_PATH_BYTES, VfsName, VfsNodeId, VfsPath, valid_vfs_root_path,
};
pub(crate) use state::{VfsStateOperation, state_volume_mount_component};
pub(crate) use storage::{
    MAX_VFS_MEM_FILE_BYTES, MAX_VFS_PIPE_BYTES, VfsMemoryFile, VfsPipeBuffer,
};
pub(crate) use vertexfs::{
    MAX_VERTEXFS_FILE_BYTES, MAX_VERTEXFS_FILES, VERTEXFS_DIRECTORY_SECTOR,
    VERTEXFS_DIRECTORY_SECTORS, VERTEXFS_DYNAMIC_FILE_CAPACITY, VERTEXFS_FREE_MAP_SECTOR,
    VERTEXFS_IMAGE_BYTES, VERTEXFS_INODE_APP_DIR, VERTEXFS_INODE_TABLE_SECTOR,
    VERTEXFS_INODE_TABLE_SECTORS, VERTEXFS_JOURNAL_PAYLOAD_OFFSET, VERTEXFS_JOURNAL_SECTOR,
    VERTEXFS_MODULE_STRING, VERTEXFS_SECTOR_SIZE, VERTEXFS_SYNC_MAX_DEVICE_WRITES,
    VertexFsDeviceWrite, VertexFsInode, VertexFsSyncResult, VfsVertexFsFile, parse_vertexfs_image,
    vertexfs_checksum32, vertexfs_device_absolute_sector, vertexfs_dynamic_data_sector_at,
    vertexfs_dynamic_inode_at, vertexfs_image_has_inode, vertexfs_image_sector,
    write_vertexfs_dynamic_metadata, write_vertexfs_file_extent, write_vertexfs_inode_record,
    write_vertexfs_journal_clean, write_vertexfs_journal_pending,
};
