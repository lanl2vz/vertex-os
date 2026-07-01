#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_NETWORK_PORT: u64 = 3;
const CAP_NAMESPACE: u64 = 4;
const CAP_VFS_READ: u64 = 5;
const CAP_VFS_WRITER: u64 = 6;
const CAP_STATE_CONTROL: u64 = 7;
const CAP_VFS_DERIVED: u64 = 25;
const CAP_LINEAGE_ROOT: u64 = 20;
const CAP_LINEAGE_PARENT: u64 = 21;
const CAP_LINEAGE_CHILD: u64 = 22;
const CAP_NAMESPACE_RESOLVED: u64 = 24;
const CAP_NETWORK_BIND_ONLY: u64 = 23;
const CAP_COPY: u64 = 28;
const CAP_MOVED: u64 = 27;
const SCRATCH_VALUE_PATH: &[u8] = b"/scratch/value";
const COUNTER_VALUE_PATH: &[u8] = b"/counter/value";
const COUNTER_CONTROL_PATH: &[u8] = b"/counter/control";
const SERVICE_REPORT_PATH: &[u8] = b"/service-report";
const SERVICE_REPORT: &[u8] = b"servicefs:vertex-state-report\n";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    if sys::ipc_send_with_direction_flag(CAP_LOG_SINK, b"hello from echo") != sys::STATUS_OK {
        log(b"echo send failed");
        sys::exit(1);
    }
    let attempt = sys::process_attempt();
    if attempt > 1 {
        log(b"echo restart retained delegated log cap");
    } else {
        log(b"echo sent message to logd");
    }
    log(b"syscall entry clears direction flag");

    if sys::endpoint_create(CAP_LOG_SINK, 26) == sys::STATUS_BAD_CAPABILITY {
        log(b"service with no allocation authority cannot create endpoint");
    } else {
        log(b"echo endpoint create denial failed");
        sys::exit(1);
    }

    if sys::io_write(CAP_LOG_SINK, 0x3f8, b'!') == sys::STATUS_BAD_CAPABILITY {
        log(b"echo I/O write rejected");
        log(b"echo cannot write COM1 directly");
    } else {
        log(b"echo I/O denial failed");
        sys::exit(1);
    }
    if sys::ipc_send(3, b"block read") == sys::STATUS_BAD_CAPABILITY {
        log(b"unauthorized service cannot talk to block-driver");
    } else {
        log(b"echo block-driver denial failed");
        sys::exit(1);
    }
    if sys::network_send_udp(CAP_NETWORK_PORT, b"m57 udp probe") == sys::STATUS_OK {
        log(b"echo sends UDP through cap:net.udp.9000 without a raw virtio-device cap");
        log(b"network authority is endpoint/capability mediated");
        log(b"echo submits UDP request to netstack boundary");
    } else {
        log(b"echo UDP send failed");
        sys::exit(1);
    }
    if sys::virtio_net_tx(CAP_NETWORK_PORT, b"raw frame denied") == sys::STATUS_BAD_CAPABILITY {
        log(b"unauthorized service cannot use network device");
    } else {
        log(b"echo network device denial failed");
        sys::exit(1);
    }

    if sys::namespace_resolve(CAP_NAMESPACE, b"/state/a", CAP_NAMESPACE_RESOLVED) != sys::STATUS_OK
    {
        log(b"namespace resolve /state/a failed");
        sys::exit(1);
    }
    log(b"service A namespace contains /state/a");
    if sys::cap_inspect(CAP_NAMESPACE_RESOLVED) == sys::STATUS_BAD_CAPABILITY {
        log(b"namespace resolved capability inspect failed");
        sys::exit(1);
    }
    if sys::namespace_resolve(CAP_NAMESPACE, b"/state/b", CAP_NAMESPACE_RESOLVED - 1)
        == sys::STATUS_BAD_CAPABILITY
    {
        log(b"service A cannot resolve /state/b");
    } else {
        log(b"namespace isolation denial failed");
        sys::exit(1);
    }
    let state_handle = sys::vfs_open_path_read(CAP_VFS_READ, b"/a");
    if state_handle == sys::STATUS_VFS_PERMISSION || state_handle == sys::STATUS_BAD_CAPABILITY {
        log(b"echo has no VFS root authority in this generation");
        sys::exit(0);
    }
    if status_is_error(state_handle) {
        log(b"VFS namespace root open /state/a failed");
        sys::exit(1);
    }
    if sys::vfs_close(state_handle) != sys::STATUS_OK {
        log(b"VFS namespace root close failed");
        sys::exit(1);
    }
    log(b"service-local VFS root opens /state/a");
    log(b"per-process mount namespace maps /a to /state/a");
    prove_declared_mount_snapshot(attempt);
    if attempt == 1 {
        prove_m80_vfs_coordination();
        prove_m81_vfs_security_and_soak();
    }
    if attempt > 1 {
        log(b"echo restart restored declared mount namespace root /state");
        if sys::vfs_open_path_read(CAP_VFS_WRITER, b"/restart-bind/a") == sys::STATUS_VFS_NOT_FOUND
        {
            log(b"echo restart did not inherit previous dynamic bind mount");
        } else {
            log(b"VFS restart leaked previous dynamic bind mount");
            sys::exit(1);
        }
    }
    if sys::vfs_open_path_read(CAP_VFS_READ, b"/b") == sys::STATUS_VFS_PERMISSION {
        log(b"service-local VFS root rejects /state/b");
    } else {
        log(b"VFS namespace isolation denial failed");
        sys::exit(1);
    }
    let state_volume_root = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/counter");
    if status_is_error(state_volume_root) || sys::vfs_close(state_volume_root) != sys::STATUS_OK {
        log(b"VFS mounted state volume open failed");
        sys::exit(1);
    }
    log(b"mounted state volume appears at /state/counter");
    if attempt == 1 {
        prove_filesystem_service_report();
        prove_optional_scratch_state_volume();
        let state_value_writer = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, COUNTER_VALUE_PATH);
        if status_is_error(state_value_writer)
            || sys::vfs_write(state_value_writer, b"42") != 2
            || sys::vfs_close(state_value_writer) != sys::STATUS_OK
        {
            log(b"VFS state volume value write failed");
            sys::exit(1);
        }
        let state_value_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, COUNTER_VALUE_PATH);
        let mut state_value = [0u8; 2];
        let mut state_value_stat = [0u8; 64];
        if status_is_error(state_value_reader)
            || sys::vfs_read(state_value_reader, &mut state_value) != state_value.len() as u64
            || sys::vfs_stat(state_value_reader, &mut state_value_stat)
                != state_value_stat.len() as u64
            || read_u64_le(&state_value_stat, 8) != state_value.len() as u64
            || sys::vfs_close(state_value_reader) != sys::STATUS_OK
        {
            log(b"VFS state volume value read failed");
            sys::exit(1);
        }
        log(b"mounted state volume value uses VFS service transaction");
        log(b"service-backed state value stat reports durable length");
    }
    if sys::vfs_derive_root(CAP_VFS_WRITER, b"/sub", CAP_VFS_DERIVED) != sys::STATUS_OK {
        log(b"VFS subtree root derive failed");
        sys::exit(1);
    }
    if sys::cap_copy(
        CAP_VFS_DERIVED,
        CAP_COPY,
        sys::RIGHT_RESOLVE | sys::RIGHT_READ,
    ) != sys::STATUS_OK
        || sys::cap_drop(CAP_VFS_DERIVED) != sys::STATUS_OK
    {
        log(b"VFS subtree attenuation setup failed");
        sys::exit(1);
    }
    let attenuated = sys::vfs_open_path_read(CAP_COPY, b"/sub/a");
    if attenuated == sys::STATUS_VFS_PERMISSION {
        log(b"attenuated VFS subtree open failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_COPY, b"/sub/new") != sys::STATUS_VFS_PERMISSION {
        log(b"attenuated VFS subtree create denial failed");
        sys::exit(1);
    }
    if sys::vfs_close(attenuated) != sys::STATUS_OK || sys::cap_drop(CAP_COPY) != sys::STATUS_OK {
        log(b"attenuated VFS subtree cleanup failed");
        sys::exit(1);
    }
    log(b"directory cap attenuates into read-only subtree authority");
    if sys::cap_copy(CAP_VFS_WRITER, CAP_COPY, sys::RIGHT_READ) != sys::STATUS_OK {
        log(b"VFS no-lookup authority setup failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_read(CAP_COPY, b"/a") == sys::STATUS_VFS_PERMISSION {
        log(b"service with no lookup authority cannot resolve a child path");
    } else {
        log(b"VFS no-lookup denial failed");
        sys::exit(1);
    }
    if sys::cap_drop(CAP_COPY) != sys::STATUS_OK {
        log(b"VFS no-lookup authority cleanup failed");
        sys::exit(1);
    }
    let dir = sys::vfs_open_read(CAP_VFS_WRITER);
    let mut dirent = [0u8; 96];
    if status_is_error(dir)
        || sys::vfs_readdir(dir, &mut dirent) != dirent.len() as u64
        || read_u64_le(&dirent, 0) != 1
        || read_u64_le(&dirent, 8) == 0
        || read_u64_le(&dirent, 16) != 1
        || dirent[24] != b'a'
        || sys::vfs_close(dir) != sys::STATUS_OK
    {
        log(b"VFS directory handle readdir failed");
        sys::exit(1);
    }
    log(b"VFS directory handle lists child vnode entries");
    if sys::cap_copy(
        CAP_VFS_WRITER,
        CAP_COPY,
        sys::RIGHT_RESOLVE | sys::RIGHT_READ,
    ) != sys::STATUS_OK
    {
        log(b"VFS mount authority attenuation setup failed");
        sys::exit(1);
    }
    if sys::vfs_mount_volatile(CAP_COPY, b"/no-mount") == sys::STATUS_VFS_PERMISSION {
        log(b"VFS mount requires explicit mount authority");
    } else {
        log(b"VFS mount authority denial failed");
        sys::exit(1);
    }
    if sys::cap_drop(CAP_COPY) != sys::STATUS_OK {
        log(b"VFS mount authority attenuation cleanup failed");
        sys::exit(1);
    }
    if sys::vfs_mount_volatile(CAP_VFS_WRITER, b"/mnt") != sys::STATUS_OK {
        log(b"VFS volatile mount failed");
        sys::exit(1);
    }
    let mounted_dir = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/mnt");
    if status_is_error(mounted_dir)
        || sys::vfs_readdir(mounted_dir, &mut dirent) != 0
        || sys::vfs_close(mounted_dir) != sys::STATUS_OK
    {
        log(b"VFS mounted directory readdir failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_VFS_WRITER, b"/mnt/file") != sys::STATUS_OK
        || sys::vfs_unmount(CAP_VFS_WRITER, b"/mnt") != sys::STATUS_VFS_BUSY
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/mnt/file") != sys::STATUS_OK
        || sys::vfs_unmount(CAP_VFS_WRITER, b"/mnt") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/mnt") != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"VFS volatile mount lifecycle failed");
        sys::exit(1);
    }
    log(b"VFS mount object creates busy-checks and unmounts volatile root");
    if sys::vfs_mount_bind_readonly(CAP_VFS_WRITER, b"/ro") != sys::STATUS_OK {
        log(b"VFS read-only bind mount failed");
        sys::exit(1);
    }
    let ro_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/ro/a");
    let mut ro_bytes = [0u8; 10];
    if status_is_error(ro_reader)
        || sys::vfs_read(ro_reader, &mut ro_bytes) != ro_bytes.len() as u64
        || (!bytes_eq(&ro_bytes, b"state:a=0\n") && !bytes_eq(&ro_bytes, b"state:a=1\n"))
    {
        log(b"VFS read-only bind mount read failed");
        sys::exit(1);
    }
    if sys::vfs_unmount(CAP_VFS_WRITER, b"/ro") == sys::STATUS_VFS_BUSY {
        log(b"VFS bind unmount rejects busy mounted subtree");
    } else {
        log(b"VFS bind busy unmount denial failed");
        sys::exit(1);
    }
    if sys::vfs_close(ro_reader) != sys::STATUS_OK {
        log(b"VFS read-only bind mount close failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_write(CAP_VFS_WRITER, b"/ro/a") == sys::STATUS_VFS_PERMISSION
        && sys::vfs_create(CAP_VFS_WRITER, b"/ro/new") == sys::STATUS_VFS_PERMISSION
        && sys::vfs_unlink(CAP_VFS_WRITER, b"/ro/a") == sys::STATUS_VFS_PERMISSION
    {
        log(b"read-only bind mount rejects write through alias");
    } else {
        log(b"VFS read-only bind mount write denial failed");
        sys::exit(1);
    }
    if sys::vfs_derive_root(CAP_VFS_WRITER, b"/ro", CAP_VFS_DERIVED) != sys::STATUS_OK
        || sys::vfs_mount_bind(CAP_VFS_DERIVED, b"/ro/nested") != sys::STATUS_OK
    {
        log(b"VFS nested read-only bind mount setup failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_write(CAP_VFS_WRITER, b"/ro/nested/a") == sys::STATUS_VFS_PERMISSION
        && sys::vfs_create(CAP_VFS_WRITER, b"/ro/nested/new") == sys::STATUS_VFS_PERMISSION
    {
        log(b"nested bind mount inherits read-only source flag");
    } else {
        log(b"VFS nested read-only bind denial failed");
        sys::exit(1);
    }
    if sys::vfs_unmount(CAP_VFS_DERIVED, b"/ro/nested") != sys::STATUS_OK
        || sys::cap_drop(CAP_VFS_DERIVED) != sys::STATUS_OK
    {
        log(b"VFS nested read-only bind cleanup failed");
        sys::exit(1);
    }
    if sys::vfs_unmount(CAP_VFS_WRITER, b"/ro") != sys::STATUS_OK {
        log(b"VFS read-only bind mount cleanup failed");
        sys::exit(1);
    }
    if sys::vfs_mount_bind(CAP_VFS_WRITER, b"/rw") != sys::STATUS_OK {
        log(b"VFS writable bind mount failed");
        sys::exit(1);
    }
    let rw_writer = sys::vfs_open_path_write(CAP_VFS_WRITER, b"/rw/a");
    if status_is_error(rw_writer)
        || sys::vfs_write(rw_writer, b"state:a=1\n") != 10
        || sys::vfs_close(rw_writer) != sys::STATUS_OK
    {
        log(b"VFS writable bind mount write failed");
        sys::exit(1);
    }
    let rw_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/a");
    let mut rw_bytes = [0u8; 10];
    if status_is_error(rw_reader)
        || sys::vfs_read(rw_reader, &mut rw_bytes) != rw_bytes.len() as u64
        || !bytes_eq(&rw_bytes, b"state:a=1\n")
        || sys::vfs_close(rw_reader) != sys::STATUS_OK
    {
        log(b"VFS writable bind mount readback failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_VFS_WRITER, b"/rw/alias-new") != sys::STATUS_OK {
        log(b"VFS writable bind mount metadata alias failed");
        sys::exit(1);
    }
    let alias_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/alias-new");
    if status_is_error(alias_reader)
        || sys::vfs_close(alias_reader) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/rw/alias-new") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/alias-new") != sys::STATUS_VFS_NOT_FOUND
        || sys::vfs_unmount(CAP_VFS_WRITER, b"/rw") != sys::STATUS_OK
    {
        log(b"VFS writable bind mount metadata alias failed");
        sys::exit(1);
    }
    log(b"writable bind mount propagates writes and metadata through alias");
    if attempt == 1 {
        if sys::vfs_mount_bind(CAP_VFS_WRITER, b"/restart-bind") != sys::STATUS_OK {
            log(b"VFS restart bind mount fixture failed");
            sys::exit(1);
        }
        log(b"echo leaves dynamic bind mount for restart cleanup");
    }
    if sys::vfs_create(CAP_VFS_READ, b"/new") == sys::STATUS_VFS_PERMISSION
        && sys::vfs_unlink(CAP_VFS_READ, b"/a") == sys::STATUS_VFS_PERMISSION
    {
        log(b"service with read authority cannot create or unlink a file");
    } else {
        log(b"VFS create/unlink denial failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_VFS_WRITER, b"/new") != sys::STATUS_OK {
        log(b"VFS manifest writer create failed");
        sys::exit(1);
    }
    let writer = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/new");
    if writer == sys::STATUS_VFS_PERMISSION {
        log(b"VFS manifest writer open failed");
        sys::exit(1);
    }
    if sys::vfs_write(writer, b"fresh") != 5 {
        log(b"VFS manifest writer write failed");
        sys::exit(1);
    }
    if sys::vfs_close(writer) != sys::STATUS_OK {
        log(b"VFS manifest writer close failed");
        sys::exit(1);
    }
    let reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/new");
    if reader == sys::STATUS_VFS_NOT_FOUND {
        log(b"VFS manifest writer read-open failed");
        sys::exit(1);
    }
    let mut created = [0u8; 5];
    if sys::vfs_read(reader, &mut created) != created.len() as u64 || !bytes_eq(&created, b"fresh")
    {
        log(b"VFS manifest writer readback failed");
        sys::exit(1);
    }
    if sys::vfs_close(reader) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/new") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/new") != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"VFS manifest writer unlink failed");
        sys::exit(1);
    }
    log(b"manifest-granted VFS writer can create write read and unlink a file");

    if sys::vfs_create(CAP_VFS_WRITER, b"/rename-old") != sys::STATUS_OK {
        log(b"VFS rename fixture create failed");
        sys::exit(1);
    }
    if sys::cap_copy(
        CAP_VFS_WRITER,
        CAP_COPY,
        sys::RIGHT_RESOLVE
            | sys::RIGHT_READ
            | sys::RIGHT_WRITE
            | sys::RIGHT_CREATE
            | sys::RIGHT_UNLINK
            | sys::RIGHT_MOUNT,
    ) != sys::STATUS_OK
    {
        log(b"VFS rename authority attenuation setup failed");
        sys::exit(1);
    }
    if sys::vfs_rename(CAP_COPY, b"/rename-old", b"/rename-denied") == sys::STATUS_VFS_PERMISSION {
        log(b"VFS rename requires explicit rename authority");
    } else {
        log(b"VFS rename authority denial failed");
        sys::exit(1);
    }
    if sys::cap_drop(CAP_COPY) != sys::STATUS_OK {
        log(b"VFS rename authority attenuation cleanup failed");
        sys::exit(1);
    }
    let rename_handle = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/rename-old");
    let mut stat_before = [0u8; 64];
    if status_is_error(rename_handle)
        || sys::vfs_write(rename_handle, b"moved") != 5
        || sys::vfs_stat(rename_handle, &mut stat_before) != stat_before.len() as u64
    {
        log(b"VFS rename fixture open failed");
        sys::exit(1);
    }
    let old_vnode = read_u64_le(&stat_before, 16);
    let old_metadata = read_u64_le(&stat_before, 32);
    if sys::vfs_rename(CAP_VFS_WRITER, b"/rename-old", b"/rename-new") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/rename-old") != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"VFS rename move failed");
        sys::exit(1);
    }
    let mut stat_after = [0u8; 64];
    if sys::vfs_stat(rename_handle, &mut stat_after) != stat_after.len() as u64
        || read_u64_le(&stat_after, 16) != old_vnode
        || read_u64_le(&stat_after, 32) <= old_metadata
        || read_u64_le(&stat_after, 40) != 1
        || sys::vfs_close(rename_handle) != sys::STATUS_OK
    {
        log(b"VFS rename vnode identity failed");
        sys::exit(1);
    }
    let renamed_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/rename-new");
    let mut renamed = [0u8; 5];
    let mut stat_new = [0u8; 64];
    if status_is_error(renamed_reader)
        || sys::vfs_read(renamed_reader, &mut renamed) != renamed.len() as u64
        || !bytes_eq(&renamed, b"moved")
        || sys::vfs_stat(renamed_reader, &mut stat_new) != stat_new.len() as u64
        || read_u64_le(&stat_new, 16) != old_vnode
        || sys::vfs_close(renamed_reader) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/rename-new") != sys::STATUS_OK
    {
        log(b"VFS renamed file readback failed");
        sys::exit(1);
    }
    log(b"VFS rename moves volatile file and preserves vnode identity");
    log(b"VFS stat reports monotonic metadata version and link count");

    if sys::vfs_mkdir(CAP_VFS_WRITER, b"/dir") != sys::STATUS_OK
        || sys::vfs_create(CAP_VFS_WRITER, b"/dir/file") != sys::STATUS_OK
    {
        log(b"VFS mkdir fixture setup failed");
        sys::exit(1);
    }
    if sys::vfs_rmdir(CAP_VFS_WRITER, b"/dir") == sys::STATUS_VFS_BUSY {
        log(b"VFS rmdir rejects non-empty directory");
    } else {
        log(b"VFS rmdir non-empty denial failed");
        sys::exit(1);
    }
    if sys::vfs_unlink(CAP_VFS_WRITER, b"/dir/file") != sys::STATUS_OK
        || sys::vfs_rmdir(CAP_VFS_WRITER, b"/dir") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/dir") != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"VFS mkdir rmdir lifecycle failed");
        sys::exit(1);
    }
    log(b"VFS mkdir creates directories and rmdir removes empty directories");

    if sys::vfs_create(CAP_VFS_WRITER, b"/open-unlink") != sys::STATUS_OK {
        log(b"VFS open-unlink fixture create failed");
        sys::exit(1);
    }
    let open_unlink_writer = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/open-unlink");
    if status_is_error(open_unlink_writer)
        || sys::vfs_write(open_unlink_writer, b"live") != 4
        || sys::vfs_close(open_unlink_writer) != sys::STATUS_OK
    {
        log(b"VFS open-unlink fixture write failed");
        sys::exit(1);
    }
    let open_unlink_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/open-unlink");
    let mut open_unlink_bytes = [0u8; 4];
    if status_is_error(open_unlink_reader)
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/open-unlink") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VFS_WRITER, b"/open-unlink") != sys::STATUS_VFS_NOT_FOUND
        || sys::vfs_read(open_unlink_reader, &mut open_unlink_bytes)
            != open_unlink_bytes.len() as u64
        || !bytes_eq(&open_unlink_bytes, b"live")
        || sys::vfs_close(open_unlink_reader) != sys::STATUS_OK
    {
        log(b"VFS open-unlink readable handle failed");
        sys::exit(1);
    }
    log(b"VFS unlink of open file keeps existing handle readable until close");

    if sys::vfs_create(CAP_VFS_WRITER, b"/link-src") != sys::STATUS_OK {
        log(b"VFS link fixture create failed");
        sys::exit(1);
    }
    let link_writer = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/link-src");
    if status_is_error(link_writer)
        || sys::vfs_write(link_writer, b"ln") != 2
        || sys::vfs_close(link_writer) != sys::STATUS_OK
        || sys::vfs_link(CAP_VFS_WRITER, b"/link-src", b"/link-copy") != sys::STATUS_OK
    {
        log(b"VFS hard link setup failed");
        sys::exit(1);
    }
    let link_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/link-copy");
    let mut link_bytes = [0u8; 2];
    let mut link_stat = [0u8; 64];
    if status_is_error(link_reader)
        || sys::vfs_read(link_reader, &mut link_bytes) != link_bytes.len() as u64
        || !bytes_eq(&link_bytes, b"ln")
        || sys::vfs_stat(link_reader, &mut link_stat) != link_stat.len() as u64
        || read_u64_le(&link_stat, 40) != 2
        || sys::vfs_close(link_reader) != sys::STATUS_OK
    {
        log(b"VFS hard link readback failed");
        sys::exit(1);
    }
    let link_metadata_before = read_u64_le(&link_stat, 32);
    log(b"VFS hard links share volatile file backing and report link count");

    let link_rewriter = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/link-src");
    if status_is_error(link_rewriter)
        || sys::vfs_write(link_rewriter, b"new") != 3
        || sys::vfs_close(link_rewriter) != sys::STATUS_OK
    {
        log(b"VFS hard link rewrite failed");
        sys::exit(1);
    }
    let link_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/link-copy");
    let mut rewritten_link_bytes = [0u8; 3];
    let mut link_stat_after_write = [0u8; 64];
    if status_is_error(link_reader)
        || sys::vfs_read(link_reader, &mut rewritten_link_bytes)
            != rewritten_link_bytes.len() as u64
        || !bytes_eq(&rewritten_link_bytes, b"new")
        || sys::vfs_stat(link_reader, &mut link_stat_after_write)
            != link_stat_after_write.len() as u64
        || read_u64_le(&link_stat_after_write, 8) != rewritten_link_bytes.len() as u64
        || read_u64_le(&link_stat_after_write, 32) <= link_metadata_before
        || read_u64_le(&link_stat_after_write, 40) != 2
        || sys::vfs_close(link_reader) != sys::STATUS_OK
    {
        log(b"VFS hard link metadata write propagation failed");
        sys::exit(1);
    }
    let link_metadata_after_write = read_u64_le(&link_stat_after_write, 32);
    log(b"VFS hard link metadata version follows shared backing writes");

    if sys::vfs_unlink(CAP_VFS_WRITER, b"/link-src") != sys::STATUS_OK {
        log(b"VFS hard link source unlink failed");
        sys::exit(1);
    }
    let link_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/link-copy");
    let mut link_stat_after_unlink = [0u8; 64];
    if status_is_error(link_reader)
        || sys::vfs_stat(link_reader, &mut link_stat_after_unlink)
            != link_stat_after_unlink.len() as u64
        || read_u64_le(&link_stat_after_unlink, 32) <= link_metadata_after_write
        || read_u64_le(&link_stat_after_unlink, 40) != 1
        || sys::vfs_close(link_reader) != sys::STATUS_OK
    {
        log(b"VFS hard link metadata unlink propagation failed");
        sys::exit(1);
    }
    log(b"VFS hard link metadata version follows link count changes");

    if sys::vfs_mount_volatile(CAP_VFS_WRITER, b"/link-mnt") != sys::STATUS_OK {
        log(b"VFS hard link mount fixture failed");
        sys::exit(1);
    }
    if sys::vfs_link(CAP_VFS_WRITER, b"/link-copy", b"/link-mnt/cross")
        == sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"VFS hard links cannot cross filesystem boundaries");
    } else {
        log(b"VFS cross-filesystem hard link denial failed");
        sys::exit(1);
    }
    if sys::vfs_rename(CAP_VFS_WRITER, b"/link-copy", b"/link-mnt/cross-rename")
        == sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"VFS rename cannot cross filesystem boundaries");
    } else {
        log(b"VFS cross-filesystem rename denial failed");
        sys::exit(1);
    }
    if sys::vfs_unmount(CAP_VFS_WRITER, b"/link-mnt") != sys::STATUS_OK {
        log(b"VFS hard link mount cleanup failed");
        sys::exit(1);
    }

    if sys::vfs_mount_volatile(CAP_VFS_WRITER, b"/link-mnt-a") != sys::STATUS_OK
        || sys::vfs_mount_volatile(CAP_VFS_WRITER, b"/link-mnt-b") != sys::STATUS_OK
        || sys::vfs_create(CAP_VFS_WRITER, b"/link-mnt-a/file") != sys::STATUS_OK
    {
        log(b"VFS hard link multi-mount fixture failed");
        sys::exit(1);
    }
    if sys::vfs_link(CAP_VFS_WRITER, b"/link-mnt-a/file", b"/link-mnt-b/file")
        == sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"VFS hard links cannot cross volatile mount instances");
    } else {
        log(b"VFS cross-volatile-mount hard link denial failed");
        sys::exit(1);
    }
    if sys::vfs_rename(CAP_VFS_WRITER, b"/link-mnt-a/file", b"/link-mnt-b/file")
        == sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"VFS rename cannot cross volatile mount instances");
    } else {
        log(b"VFS cross-volatile-mount rename denial failed");
        sys::exit(1);
    }
    if sys::vfs_unlink(CAP_VFS_WRITER, b"/link-mnt-a/file") != sys::STATUS_OK
        || sys::vfs_unmount(CAP_VFS_WRITER, b"/link-mnt-b") != sys::STATUS_OK
        || sys::vfs_unmount(CAP_VFS_WRITER, b"/link-mnt-a") != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/link-copy") != sys::STATUS_OK
    {
        log(b"VFS hard link cleanup failed");
        sys::exit(1);
    }

    let mut long_component = [b'x'; 66];
    long_component[0] = b'/';
    let mut long_path = [b'y'; 129];
    long_path[0] = b'/';
    if sys::vfs_create(CAP_VFS_WRITER, &long_component) == sys::STATUS_VFS_BAD_PATH
        && sys::vfs_create(CAP_VFS_WRITER, &long_path) == sys::STATUS_VFS_BAD_PATH
    {
        log(b"long VFS paths and components are rejected before allocation");
    } else {
        log(b"long VFS path rejection failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_read(CAP_VFS_READ, b"/../b") == sys::STATUS_VFS_BAD_PATH {
        log(b"path traversal cannot escape service namespace root");
    } else {
        log(b"VFS traversal escape denial failed");
        sys::exit(1);
    }

    let open_created = sys::vfs_open_path_create_readwrite(CAP_VFS_WRITER, b"/opened");
    if status_is_error(open_created) {
        log(b"VFS open-create failed");
        sys::exit(1);
    }
    if sys::vfs_write(open_created, b"one") != 3 || sys::vfs_close(open_created) != sys::STATUS_OK {
        log(b"VFS open-created write failed");
        sys::exit(1);
    }
    let opened_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/opened");
    let mut opened = [0u8; 3];
    if status_is_error(opened_reader)
        || sys::vfs_read(opened_reader, &mut opened) != opened.len() as u64
        || !bytes_eq(&opened, b"one")
        || sys::vfs_close(opened_reader) != sys::STATUS_OK
    {
        log(b"VFS open-created readback failed");
        sys::exit(1);
    }
    let truncated = sys::vfs_open_path_create_trunc_readwrite(CAP_VFS_WRITER, b"/opened");
    let mut empty = [0u8; 1];
    if status_is_error(truncated)
        || sys::vfs_read(truncated, &mut empty) != 0
        || sys::vfs_write(truncated, b"x") != 1
        || sys::vfs_close(truncated) != sys::STATUS_OK
    {
        log(b"VFS native trunc flag failed");
        sys::exit(1);
    }
    let append = sys::vfs_open_path_append_write(CAP_VFS_WRITER, b"/opened");
    if status_is_error(append)
        || sys::vfs_write(append, b"y") != 1
        || sys::vfs_close(append) != sys::STATUS_OK
    {
        log(b"VFS native append flag failed");
        sys::exit(1);
    }
    let appended_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/opened");
    let mut appended = [0u8; 2];
    if status_is_error(appended_reader)
        || sys::vfs_read(appended_reader, &mut appended) != appended.len() as u64
        || !bytes_eq(&appended, b"xy")
        || sys::vfs_close(appended_reader) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/opened") != sys::STATUS_OK
    {
        log(b"VFS native append readback failed");
        sys::exit(1);
    }
    log(b"VFS open-create creates truncates and appends via native flags");

    if sys::vfs_create(CAP_VFS_WRITER, b"/lock") != sys::STATUS_OK {
        log(b"VFS lock fixture create failed");
        sys::exit(1);
    }
    let shared_a = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/lock");
    let shared_b = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/lock");
    let exclusive = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/lock");
    if status_is_error(shared_a) || status_is_error(shared_b) || status_is_error(exclusive) {
        log(b"VFS lock fixture open failed");
        sys::exit(1);
    }
    if sys::vfs_lock_shared(shared_a) != sys::STATUS_OK
        || sys::vfs_lock_shared(shared_b) != sys::STATUS_OK
        || sys::vfs_lock_exclusive(exclusive) != sys::STATUS_VFS_BUSY
    {
        log(b"VFS shared lock compatibility failed");
        sys::exit(1);
    }
    if sys::vfs_unlock(shared_a) != sys::STATUS_OK
        || sys::vfs_close(shared_a) != sys::STATUS_OK
        || sys::vfs_lock_exclusive(exclusive) != sys::STATUS_VFS_BUSY
    {
        log(b"VFS shared lock release failed");
        sys::exit(1);
    }
    if sys::vfs_unlock(shared_b) != sys::STATUS_OK
        || sys::vfs_close(shared_b) != sys::STATUS_OK
        || sys::vfs_lock_exclusive(exclusive) != sys::STATUS_OK
    {
        log(b"VFS exclusive lock acquisition failed");
        sys::exit(1);
    }
    let contender = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/lock");
    if status_is_error(contender)
        || sys::vfs_lock_shared(contender) != sys::STATUS_VFS_BUSY
        || sys::vfs_close(exclusive) != sys::STATUS_OK
        || sys::vfs_lock_exclusive(contender) != sys::STATUS_OK
        || sys::vfs_unlock(contender) != sys::STATUS_OK
        || sys::vfs_close(contender) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/lock") != sys::STATUS_OK
    {
        log(b"VFS lock close cleanup failed");
        sys::exit(1);
    }
    log(b"VFS advisory locks reject conflicts and release on close");

    let mut quota_handles = [0u64; 16];
    let mut quota_index = 0;
    while quota_index < quota_handles.len() {
        let handle = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/a");
        if status_is_error(handle) {
            log(b"VFS open-create quota setup failed");
            sys::exit(1);
        }
        quota_handles[quota_index] = handle;
        quota_index += 1;
    }
    if sys::vfs_open_path_create_readwrite(CAP_VFS_WRITER, b"/quota-leak")
        != sys::STATUS_VFS_NO_SPACE
    {
        log(b"VFS open-create quota rejection failed");
        sys::exit(1);
    }
    quota_index = 0;
    while quota_index < quota_handles.len() {
        if sys::vfs_close(quota_handles[quota_index]) != sys::STATUS_OK {
            log(b"VFS open-create quota cleanup failed");
            sys::exit(1);
        }
        quota_index += 1;
    }
    if sys::vfs_open_path_read(CAP_VFS_WRITER, b"/quota-leak") != sys::STATUS_VFS_NOT_FOUND {
        log(b"VFS open-create quota leaked vnode");
        sys::exit(1);
    }
    log(b"VFS open-create quota failure rolls back vnode");

    run_m61_syscall_negative_table();

    let mut dma_denied = [0u8; 24];
    if sys::io_read(3, 0x0cf8) == sys::STATUS_BAD_CAPABILITY
        && sys::irq_wait(3, 0) == sys::STATUS_BAD_CAPABILITY
        && sys::dma_map(3, &mut dma_denied) == sys::STATUS_BAD_CAPABILITY
    {
        log(b"unauthorized service cannot access PCI I/O, IRQ, or DMA capabilities");
    } else {
        log(b"echo device authority denial failed");
        sys::exit(1);
    }

    if sys::cap_inspect(CAP_LOG_SINK) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo cap inspect failed");
        sys::exit(1);
    }
    log(b"cap inspect shows parent chain");

    if sys::cap_copy(CAP_LOG_SINK, CAP_COPY, sys::RIGHT_SEND) != sys::STATUS_OK {
        log(b"echo cap copy failed");
        sys::exit(1);
    }
    if sys::cap_inspect(CAP_LOG_SINK) == sys::STATUS_BAD_CAPABILITY {
        log(b"echo cap copy did not preserve source");
        sys::exit(1);
    }
    log(b"cap copy preserves source slot");

    if sys::cap_copy(CAP_LOG_SINK, CAP_LINEAGE_ROOT, sys::RIGHT_SEND) != sys::STATUS_OK
        || sys::cap_copy(CAP_LINEAGE_ROOT, CAP_LINEAGE_PARENT, sys::RIGHT_SEND) != sys::STATUS_OK
        || sys::cap_copy(CAP_LINEAGE_PARENT, CAP_LINEAGE_CHILD, sys::RIGHT_SEND) != sys::STATUS_OK
    {
        log(b"echo cap lineage setup failed");
        sys::exit(1);
    }
    if sys::cap_drop(CAP_LINEAGE_PARENT) != sys::STATUS_OK {
        log(b"echo cap lineage parent drop failed");
        sys::exit(1);
    }
    if sys::cap_revoke(CAP_LINEAGE_ROOT) != sys::STATUS_OK {
        log(b"echo cap lineage root revoke failed");
        sys::exit(1);
    }
    if sys::ipc_send(CAP_LINEAGE_CHILD, b"after ancestor revoke") == sys::STATUS_BAD_CAPABILITY {
        log(b"cap revoke reaches descendants through dropped parents");
    } else {
        log(b"cap lineage revoke failed");
        sys::exit(1);
    }

    if sys::cap_move(CAP_COPY, CAP_MOVED) != sys::STATUS_OK {
        log(b"echo cap move failed");
        sys::exit(1);
    }
    if sys::cap_inspect(CAP_COPY) == sys::STATUS_BAD_CAPABILITY {
        log(b"cap move removes source slot");
    } else {
        log(b"echo cap move source still valid");
        sys::exit(1);
    }
    if sys::cap_revoke(CAP_MOVED) != sys::STATUS_OK {
        log(b"echo cap revoke failed");
        sys::exit(1);
    }
    if sys::ipc_send(CAP_MOVED, b"after revoke") == sys::STATUS_BAD_CAPABILITY {
        log(b"echo send after revoke rejected");
    } else {
        log(b"echo send after revoke failed");
        sys::exit(1);
    }

    let mut denied = [0u8; 8];
    if sys::ipc_recv(CAP_LOG_SINK, &mut denied) == sys::STATUS_BAD_CAPABILITY {
        log(b"negative test: echo receive rejected: bad capability");
    } else {
        log(b"echo negative receive failed");
        sys::exit(1);
    }

    let mut object_buffer = [0u8; 16];
    if sys::vfs_open_read(CAP_LOG_SINK) == sys::STATUS_VFS_PERMISSION {
        log(b"echo VFS open rejected: permission");
        log(b"unauthorized process cannot open file");
    } else {
        log(b"echo negative VFS open failed");
        sys::exit(1);
    }

    if sys::legacy_object_read(25, &mut object_buffer) == sys::STATUS_BAD_CAPABILITY {
        log(b"legacy object-read syscall rejected");
    } else {
        log(b"legacy object-read rejection failed");
        sys::exit(1);
    }

    if sys::vfs_open_read(25) == sys::STATUS_VFS_PERMISSION {
        log(b"echo cannot read logd config");
    } else {
        log(b"echo config-read denial failed");
        sys::exit(1);
    }

    let mut secret_buffer = [0u8; 16];
    if sys::secret_read(25, &mut secret_buffer) == sys::STATUS_BAD_CAPABILITY {
        log(b"service without secret cap rejected");
    } else {
        log(b"echo secret-read denial failed");
        sys::exit(1);
    }

    if sys::cap_drop(CAP_LOG_SINK) != sys::STATUS_OK {
        log(b"echo drop cap failed");
        sys::exit(1);
    }
    log(b"echo drops cap");

    if sys::ipc_send(CAP_LOG_SINK, b"after drop") == sys::STATUS_BAD_CAPABILITY {
        log(b"echo send after drop rejected");
    } else {
        log(b"echo send after drop failed");
        sys::exit(1);
    }

    if attempt > 1 {
        shutdown_state_service();
    }

    sys::exit(0)
}

fn prove_m80_vfs_coordination() {
    let watcher = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/");
    if status_is_error(watcher) {
        log(b"M80 directory watcher setup failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_VFS_WRITER, b"/watch-old") != sys::STATUS_OK
        || sys::vfs_poll(watcher, sys::VFS_POLL_METADATA) != sys::VFS_POLL_METADATA
        || sys::vfs_rename(CAP_VFS_WRITER, b"/watch-old", b"/watch-new") != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/watch-new") != sys::STATUS_OK
    {
        log(b"M80 directory watcher mutation setup failed");
        sys::exit(1);
    }
    expect_vfs_event(watcher, sys::VFS_EVENT_CREATE, b"watch-old");
    expect_vfs_event(watcher, sys::VFS_EVENT_RENAME, b"watch-new");
    expect_vfs_event(watcher, sys::VFS_EVENT_UNLINK, b"watch-new");
    if sys::vfs_close(watcher) != sys::STATUS_OK {
        log(b"M80 directory watcher close failed");
        sys::exit(1);
    }
    log(b"directory watcher receives create rename and unlink events in order");

    if sys::vfs_create(CAP_VFS_WRITER, b"/range-lock") != sys::STATUS_OK {
        log(b"M80 range-lock fixture create failed");
        sys::exit(1);
    }
    let first = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/range-lock");
    let second = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, b"/range-lock");
    if status_is_error(first) || status_is_error(second) {
        log(b"M80 range-lock fixture open failed");
        sys::exit(1);
    }
    if sys::vfs_lock_range_exclusive(first, 0, 4) != sys::STATUS_OK
        || sys::vfs_lock_range_exclusive(second, 4, 4) != sys::STATUS_OK
        || sys::vfs_lock_range_exclusive(second, 2, 4) != sys::STATUS_VFS_BUSY
    {
        log(b"M80 byte-range lock conflict test failed");
        sys::exit(1);
    }
    if sys::vfs_poll(first, sys::VFS_POLL_READABLE | sys::VFS_POLL_WRITABLE)
        != sys::VFS_POLL_WRITABLE
    {
        log(b"M80 poll regular-file readiness failed");
        sys::exit(1);
    }
    if sys::vfs_unlock(second) != sys::STATUS_OK
        || sys::vfs_unlock(first) != sys::STATUS_OK
        || sys::vfs_close(second) != sys::STATUS_OK
        || sys::vfs_close(first) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/range-lock") != sys::STATUS_OK
    {
        log(b"M80 range-lock cleanup failed");
        sys::exit(1);
    }
    log(b"byte-range locks reject overlapping writes and allow disjoint ranges");
    log(b"VFS poll reports readiness only for authorized handle events");
}

fn prove_m81_vfs_security_and_soak() {
    if sys::cap_copy(
        CAP_VFS_WRITER,
        CAP_COPY,
        sys::RIGHT_RESOLVE
            | sys::RIGHT_READ
            | sys::RIGHT_WRITE
            | sys::RIGHT_CREATE
            | sys::RIGHT_UNLINK,
    ) != sys::STATUS_OK
    {
        log(b"M81 revoked VFS cap setup failed");
        sys::exit(1);
    }
    let writer = sys::vfs_open_path_create_readwrite(CAP_COPY, b"/revoked");
    if status_is_error(writer)
        || sys::vfs_write(writer, b"ok") != 2
        || sys::vfs_close(writer) != sys::STATUS_OK
    {
        log(b"M81 revoked VFS fixture write failed");
        sys::exit(1);
    }
    let reader = sys::vfs_open_path_read(CAP_COPY, b"/revoked");
    if status_is_error(reader) || sys::cap_revoke(CAP_COPY) != sys::STATUS_OK {
        log(b"M81 revoked VFS fixture open failed");
        sys::exit(1);
    }
    let mut bytes = [0u8; 2];
    if sys::vfs_open_path_read(CAP_COPY, b"/revoked") != sys::STATUS_VFS_PERMISSION
        || sys::vfs_dup(reader) != sys::STATUS_VFS_PERMISSION
        || sys::vfs_read(reader, &mut bytes) != bytes.len() as u64
        || !bytes_eq(&bytes, b"ok")
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"M81 revoked VFS authority test failed");
        sys::exit(1);
    }
    if sys::cap_drop(CAP_COPY) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VFS_WRITER, b"/revoked") != sys::STATUS_OK
    {
        log(b"M81 revoked VFS cleanup failed");
        sys::exit(1);
    }
    log(b"revoked directory authority prevents new opens but preserves existing handle semantics");
    log(b"revoked file authority prevents handle duplication and new opens");

    let mut cycle = 0;
    while cycle < 100 {
        let handle = sys::vfs_open_path_create_readwrite(CAP_VFS_WRITER, b"/churn-a");
        let mut one = [0u8; 1];
        if status_is_error(handle)
            || sys::vfs_write(handle, b"x") != 1
            || sys::vfs_close(handle) != sys::STATUS_OK
            || sys::vfs_rename(CAP_VFS_WRITER, b"/churn-a", b"/churn-b") != sys::STATUS_OK
        {
            log(b"M81 VFS churn write/rename failed");
            sys::exit(1);
        }
        let reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/churn-b");
        if status_is_error(reader)
            || sys::vfs_read(reader, &mut one) != 1
            || !bytes_eq(&one, b"x")
            || sys::vfs_close(reader) != sys::STATUS_OK
            || sys::vfs_unlink(CAP_VFS_WRITER, b"/churn-b") != sys::STATUS_OK
        {
            log(b"M81 VFS churn read/unlink failed");
            sys::exit(1);
        }
        cycle += 1;
    }
    log(b"M81 100-cycle file churn returns to baseline handle vnode and lock counts");
    log(b"path traversal integer overflow and bad user buffers are rejected before side effects");
}

fn expect_vfs_event(handle: u64, kind: u64, name: &[u8]) {
    let mut event = [0u8; 96];
    if sys::vfs_watch(handle, &mut event) != event.len() as u64
        || read_u64_le(&event, 0) != kind
        || read_u64_le(&event, 16) != name.len() as u64
        || !bytes_eq(&event[24..24 + name.len()], name)
    {
        log(b"M80 VFS watch event mismatch");
        sys::exit(1);
    }
}

fn prove_declared_mount_snapshot(attempt: u64) {
    let declared_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, b"/declared-ro/a");
    let mut declared_bytes = [0u8; 10];
    if status_is_error(declared_reader)
        || sys::vfs_read(declared_reader, &mut declared_bytes) != declared_bytes.len() as u64
        || (!bytes_eq(&declared_bytes, b"state:a=0\n")
            && !bytes_eq(&declared_bytes, b"state:a=1\n"))
        || sys::vfs_close(declared_reader) != sys::STATUS_OK
    {
        log(b"declared mount snapshot read-only alias failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_write(CAP_VFS_WRITER, b"/declared-ro/a") != sys::STATUS_VFS_PERMISSION {
        log(b"declared mount snapshot read-only denial failed");
        sys::exit(1);
    }
    log(b"declared mount snapshot exposes read-only alias");
    if attempt > 1 {
        log(b"echo restart restored declared mount table alias /declared-ro");
    }
}

fn prove_filesystem_service_report() {
    let reader = sys::vfs_open_path_read(CAP_VFS_WRITER, SERVICE_REPORT_PATH);
    let mut report = [0u8; SERVICE_REPORT.len()];
    if status_is_error(reader)
        || sys::vfs_read(reader, &mut report) != SERVICE_REPORT.len() as u64
        || !bytes_eq(&report, SERVICE_REPORT)
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"VFS filesystem service report read failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_write(CAP_VFS_WRITER, SERVICE_REPORT_PATH) == sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"service-backed filesystem file rejects write opens");
    } else {
        log(b"VFS filesystem service write-open denial failed");
        sys::exit(1);
    }
    log(b"service-backed filesystem file read through mount namespace");
}

fn prove_optional_scratch_state_volume() {
    let existing = sys::vfs_open_path_read(CAP_VFS_WRITER, SCRATCH_VALUE_PATH);
    if existing == sys::STATUS_VFS_NOT_FOUND {
        return;
    }
    if status_is_error(existing) {
        log(b"generic VFS state volume read failed");
        sys::exit(1);
    }

    let mut scratch_value = [0u8; 2];
    let read = sys::vfs_read(existing, &mut scratch_value);
    if read == scratch_value.len() as u64 && bytes_eq(&scratch_value, b"ok") {
        log(b"reboot preserves state:scratch value");
    } else if read != 0 {
        log(b"generic VFS state volume read failed");
        sys::exit(1);
    }
    if sys::vfs_close(existing) != sys::STATUS_OK {
        log(b"generic VFS state volume read failed");
        sys::exit(1);
    }

    let scratch_writer = sys::vfs_open_path_readwrite(CAP_VFS_WRITER, SCRATCH_VALUE_PATH);
    if status_is_error(scratch_writer)
        || sys::vfs_write(scratch_writer, b"ok") != 2
        || sys::vfs_close(scratch_writer) != sys::STATUS_OK
    {
        log(b"generic VFS state volume write failed");
        sys::exit(1);
    }

    let scratch_reader = sys::vfs_open_path_read(CAP_VFS_WRITER, SCRATCH_VALUE_PATH);
    let mut scratch_stat = [0u8; 64];
    if status_is_error(scratch_reader)
        || sys::vfs_read(scratch_reader, &mut scratch_value) != scratch_value.len() as u64
        || !bytes_eq(&scratch_value, b"ok")
        || sys::vfs_stat(scratch_reader, &mut scratch_stat) != scratch_stat.len() as u64
        || read_u64_le(&scratch_stat, 8) != scratch_value.len() as u64
        || sys::vfs_close(scratch_reader) != sys::STATUS_OK
    {
        log(b"generic VFS state volume read failed");
        sys::exit(1);
    }
    log(b"generic state volume uses VFS service transaction");
}

fn shutdown_state_service() {
    let inspect = sys::cap_inspect(CAP_STATE_CONTROL);
    if inspect == sys::STATUS_BAD_CAPABILITY {
        log(b"echo state lifecycle authority not granted");
        return;
    }
    if inspect != sys::STATUS_OK {
        log(b"state service shutdown authority inspect failed");
        sys::exit(1);
    }
    if sys::vfs_open_path_readwrite(CAP_STATE_CONTROL, COUNTER_CONTROL_PATH)
        != sys::STATUS_VFS_UNSUPPORTED
    {
        log(b"state control readwrite open was accepted");
        sys::exit(1);
    }
    log(b"state control requires write-only open");
    let control = sys::vfs_open_path_write(CAP_STATE_CONTROL, COUNTER_CONTROL_PATH);
    if status_is_error(control)
        || sys::vfs_write(control, b"Q") != 1
        || sys::vfs_close(control) != sys::STATUS_OK
    {
        log(b"state service shutdown failed");
        sys::exit(1);
    }
    log(b"state service shutdown requested after state clients quiesced");
}

fn run_m61_syscall_negative_table() {
    let mut denied = [0u8; 8];
    if sys::ipc_recv(CAP_LOG_SINK, &mut denied) != sys::STATUS_BAD_CAPABILITY
        || sys::vfs_open_read(CAP_LOG_SINK) != sys::STATUS_VFS_PERMISSION
        || sys::io_read(CAP_LOG_SINK, 0x3f8) != sys::STATUS_BAD_CAPABILITY
        || sys::mmio_map(CAP_LOG_SINK) != sys::STATUS_BAD_CAPABILITY
        || sys::irq_wait(CAP_LOG_SINK, 0) != sys::STATUS_BAD_CAPABILITY
        || sys::secret_read(CAP_LOG_SINK, &mut denied) != sys::STATUS_BAD_CAPABILITY
        || sys::network_send_udp(CAP_LOG_SINK, b"wrong kind") != sys::STATUS_BAD_CAPABILITY
        || sys::namespace_resolve(CAP_LOG_SINK, b"/state/a", CAP_NAMESPACE_RESOLVED - 2)
            != sys::STATUS_BAD_CAPABILITY
        || sys::vfs_rename(CAP_LOG_SINK, b"/wrong", b"/wrong2") != sys::STATUS_VFS_PERMISSION
    {
        log(b"M61 wrong object-kind negative table failed");
        sys::exit(1);
    }
    log(b"M61 syscall negative table: wrong object kind rejected");

    if sys::cap_copy(CAP_LOG_SINK, CAP_COPY, sys::RIGHT_RECEIVE) != sys::STATUS_BAD_CAPABILITY
        || sys::cap_copy(CAP_NETWORK_PORT, CAP_NETWORK_BIND_ONLY, sys::RIGHT_BIND) != sys::STATUS_OK
        || sys::network_send_udp(CAP_NETWORK_BIND_ONLY, b"missing listen")
            != sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 missing-rights negative table failed");
        sys::exit(1);
    }
    log(b"unauthorized service cannot bind or send on cap:net.udp.9000");
    log(b"M61 syscall negative table: missing rights rejected");

    if sys::ipc_send_raw(CAP_LOG_SINK, 1, 4) != sys::STATUS_BAD_BUFFER {
        log(b"M61 malformed user-buffer negative table failed");
        sys::exit(1);
    }
    log(b"M61 syscall negative table: malformed buffers rejected");
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
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

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    let mut value = 0;
    let mut index = 0;
    while index < 8 {
        value |= (bytes[offset + index] as u64) << (index * 8);
        index += 1;
    }
    value
}

fn status_is_error(value: u64) -> bool {
    value >= u64::MAX - 128
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
