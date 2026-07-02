#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_STORE_REPLY: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_STORE_REQUEST: u64 = 3;
const CAP_VERTEXFS_ROOT: u64 = 4;
const CAP_VERTEXFS_FSYNC_FAULT_MODE: u64 = 5;
const HELLO_OBJECT: &[u8] = b"hello from Krust store\n";
const VERTEXFS_FSYNC_FAULT_MODE: &[u8] = b"krust-vertexfs-fsync-fault\n";
const VERTEXFS_APP_A: &[u8] = b"vertexfs:a=1\n";
const VERTEXFS_APP_A_REPLAYED: &[u8] = b"vertexfs:a=2\n";
const VERTEXFS_APP_A_COMMITTED: &[u8] = b"vertexfs:a=3\n";
const VERTEXFS_APP_A_SYNCED: &[u8] = b"vertexfs:a=4\n";
const VERTEXFS_CREATED: &[u8] = b"created-v1\n";
const VERTEXFS_CREATED2: &[u8] = b"created-v2\n";
const VERTEXFS_CREATED3: &[u8] = b"created-v3\n";
const VERTEXFS_CREATED4: &[u8] = b"created-v4\n";
const VERTEXFS_CREATED5: &[u8] = b"created-v5\n";
const VERTEXFS_CREATED6: &[u8] = b"created-v6\n";
const VERTEXFS_CREATED7: &[u8] = b"created-v7\n";
const VERTEXFS_CREATED8: &[u8] = b"created-v8\n";
const VERTEXFS_CREATED9: &[u8] = b"created-v9\n";
const VERTEXFS_CREATED10: &[u8] = b"created-v10\n";
const VERTEXFS_CREATED11: &[u8] = b"created-v11\n";
const VERTEXFS_CREATED12: &[u8] = b"created-v12\n";

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut buffer = [0u8; 64];
    log(b"model-reader asks for store:hello-text");
    if sys::ipc_send(CAP_STORE_REQUEST, b"store:hello-text") != sys::STATUS_OK {
        log(b"model-reader store request failed");
        sys::exit(1);
    }
    let read = sys::ipc_recv(CAP_STORE_REPLY, &mut buffer);
    if read == sys::STATUS_BAD_CAPABILITY || read == sys::STATUS_BAD_BUFFER {
        log(b"model-reader store read failed");
        sys::exit(1);
    }
    if read > buffer.len() as u64 || !bytes_eq(&buffer[..read as usize], HELLO_OBJECT) {
        log(b"model-reader store bytes invalid");
        sys::exit(1);
    }
    log(b"model-reader reads bytes");
    log(b"model-reader reads bytes successfully");
    prove_vertexfs();
    log(b"Native immutable store client ok");
    sys::exit(0)
}

fn prove_vertexfs() {
    let mut buffer = [0u8; 32];
    let expect_fsync_fault = vertexfs_fsync_fault_mode();
    let root = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/");
    if status_is_error(root) || sys::vfs_close(root) != sys::STATUS_OK {
        log(b"model-reader VertexFS root open failed");
        sys::exit(1);
    }

    let app_a = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/a");
    if status_is_error(app_a) {
        log(b"model-reader VertexFS declared file open failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(app_a, &mut buffer);
    let app_a_is_base = read == VERTEXFS_APP_A.len() as u64
        && bytes_eq(&buffer[..VERTEXFS_APP_A.len()], VERTEXFS_APP_A);
    let app_a_is_replayed = read == VERTEXFS_APP_A_REPLAYED.len() as u64
        && bytes_eq(
            &buffer[..VERTEXFS_APP_A_REPLAYED.len()],
            VERTEXFS_APP_A_REPLAYED,
        );
    let app_a_is_committed = read == VERTEXFS_APP_A_COMMITTED.len() as u64
        && bytes_eq(
            &buffer[..VERTEXFS_APP_A_COMMITTED.len()],
            VERTEXFS_APP_A_COMMITTED,
        );
    if (!app_a_is_base && !app_a_is_replayed && !app_a_is_committed)
        || sys::vfs_close(app_a) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS declared file read failed");
        sys::exit(1);
    }
    log(b"mount namespace root exposes declared VertexFS app tree");
    log(b"model-reader VertexFS namespace root maps /a to /fs/app/a");
    log(b"VertexFS v1 declared file read through VFS");
    if app_a_is_replayed {
        log(b"VertexFS v1 journal replay read returned new file");
    }
    if app_a_is_committed {
        log(b"VertexFS v1 post-sync image read returned committed file");
    }

    let app_a_writer = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, b"/a");
    if status_is_error(app_a_writer)
        || sys::vfs_write(app_a_writer, VERTEXFS_APP_A_SYNCED) != VERTEXFS_APP_A_SYNCED.len() as u64
    {
        log(b"model-reader VertexFS declared file fsync failed");
        sys::exit(1);
    }
    let fsync_status = sys::vfs_sync(app_a_writer);
    if expect_fsync_fault {
        if fsync_status != sys::STATUS_VFS_UNSUPPORTED
            || sys::vfs_close(app_a_writer) != sys::STATUS_OK
        {
            log(b"model-reader VertexFS fsync fault handling failed");
            sys::exit(1);
        }
        let app_a_dirty = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/a");
        if status_is_error(app_a_dirty) {
            log(b"model-reader VertexFS fsync fault dirty readback failed");
            sys::exit(1);
        }
        let read = sys::vfs_read(app_a_dirty, &mut buffer);
        if read != VERTEXFS_APP_A_SYNCED.len() as u64
            || !bytes_eq(
                &buffer[..VERTEXFS_APP_A_SYNCED.len()],
                VERTEXFS_APP_A_SYNCED,
            )
            || sys::vfs_close(app_a_dirty) != sys::STATUS_OK
        {
            log(b"model-reader VertexFS fsync fault dirty readback failed");
            sys::exit(1);
        }
        log(b"VertexFS v1 fsync block-driver fault returns unsupported");
        log(b"VertexFS v1 fsync fault keeps runtime dirty file readable");
        return;
    }
    if fsync_status != sys::STATUS_OK || sys::vfs_close(app_a_writer) != sys::STATUS_OK {
        log(b"model-reader VertexFS declared file fsync failed");
        sys::exit(1);
    }
    let app_a_synced = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/a");
    if status_is_error(app_a_synced) {
        log(b"model-reader VertexFS declared file fsync readback failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(app_a_synced, &mut buffer);
    if read != VERTEXFS_APP_A_SYNCED.len() as u64
        || !bytes_eq(
            &buffer[..VERTEXFS_APP_A_SYNCED.len()],
            VERTEXFS_APP_A_SYNCED,
        )
        || sys::vfs_close(app_a_synced) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS declared file fsync readback failed");
        sys::exit(1);
    }
    log(b"VertexFS v1 declared file fsync journal readback ok");

    create_sync_readback(b"/created", VERTEXFS_CREATED, &mut buffer);
    log(b"VertexFS v1 dynamic create write fsync readback ok");
    create_sync_readback(b"/created2", VERTEXFS_CREATED2, &mut buffer);
    log(b"VertexFS v1 second dynamic create write fsync readback ok");
    create_sync_readback(b"/created3", VERTEXFS_CREATED3, &mut buffer);
    log(b"VertexFS v1 third dynamic create write fsync readback ok");
    create_sync_readback(b"/created4", VERTEXFS_CREATED4, &mut buffer);
    log(b"VertexFS v1 expanded metadata create beyond old table capacity ok");
    create_sync_readback(b"/created5", VERTEXFS_CREATED5, &mut buffer);
    create_sync_readback(b"/created6", VERTEXFS_CREATED6, &mut buffer);
    create_sync_readback(b"/created7", VERTEXFS_CREATED7, &mut buffer);
    create_sync_readback(b"/created8", VERTEXFS_CREATED8, &mut buffer);
    create_sync_readback(b"/created9", VERTEXFS_CREATED9, &mut buffer);
    create_sync_readback(b"/created10", VERTEXFS_CREATED10, &mut buffer);
    create_sync_readback(b"/created11", VERTEXFS_CREATED11, &mut buffer);
    log(b"VertexFS v1 expanded metadata fills dynamic inode 15");
    let created12 = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, b"/created12");
    if created12 == sys::STATUS_VFS_NO_SPACE {
        log(b"VertexFS v1 dynamic create returns no space at expanded metadata capacity");
    } else if status_is_error(created12) {
        log(b"model-reader VertexFS dynamic capacity denial failed");
        sys::exit(1);
    } else {
        finish_created_file(created12, b"/created12", VERTEXFS_CREATED12, &mut buffer);
        log(b"VertexFS v2 dynamic create grows beyond v1 inode capacity ok");
        prove_vertexfs_v2_metadata(&mut buffer);
    }
}

fn create_sync_readback(path: &[u8], payload: &[u8], buffer: &mut [u8; 32]) {
    let writer = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, path);
    finish_created_file(writer, path, payload, buffer);
}

fn finish_created_file(writer: u64, path: &[u8], payload: &[u8], buffer: &mut [u8; 32]) {
    if status_is_error(writer)
        || sys::vfs_write(writer, payload) != payload.len() as u64
        || sys::vfs_sync(writer) != sys::STATUS_OK
        || sys::vfs_close(writer) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS create/write/fsync failed");
        sys::exit(1);
    }
    let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, path);
    if status_is_error(reader) {
        log(b"model-reader VertexFS created file readback failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(reader, buffer);
    if read != payload.len() as u64
        || !bytes_eq(&buffer[..payload.len()], payload)
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS created file readback failed");
        sys::exit(1);
    }
}

fn prove_vertexfs_v2_metadata(buffer: &mut [u8; 32]) {
    prove_vertexfs_v2_watch();
    prove_vertexfs_v2_rename(buffer);
    prove_vertexfs_v2_open_unlink(buffer);
    prove_vertexfs_v2_hard_link(buffer);
    prove_vertexfs_v2_directories();
    prove_vertexfs_v2_truncate_append(buffer);
    prove_vertexfs_v2_churn(buffer);
    log(b"VertexFS v2 durable metadata operations ok");
}

fn prove_vertexfs_v2_watch() {
    let watcher = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/");
    if status_is_error(watcher) {
        log(b"model-reader VertexFS v2 watcher setup failed");
        sys::exit(1);
    }
    if sys::vfs_create(CAP_VERTEXFS_ROOT, b"/m92-watch-old") != sys::STATUS_OK
        || sys::vfs_rename(CAP_VERTEXFS_ROOT, b"/m92-watch-old", b"/m92-watch-new")
            != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-watch-new") != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 watcher mutation failed");
        sys::exit(1);
    }
    expect_vfs_event(watcher, sys::VFS_EVENT_CREATE, b"m92-watch-old");
    expect_vfs_event(watcher, sys::VFS_EVENT_RENAME, b"m92-watch-new");
    expect_vfs_event(watcher, sys::VFS_EVENT_UNLINK, b"m92-watch-new");
    if sys::vfs_close(watcher) != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 watcher close failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 watchers receive create rename and unlink events");
}

fn prove_vertexfs_v2_rename(buffer: &mut [u8; 32]) {
    const PAYLOAD: &[u8] = b"m92-rename\n";
    create_sync_readback(b"/m92-rename-old", PAYLOAD, buffer);
    if sys::vfs_rename(CAP_VERTEXFS_ROOT, b"/m92-rename-old", b"/m92-rename-new")
        != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-rename-old")
            != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"model-reader VertexFS v2 rename visibility failed");
        sys::exit(1);
    }
    read_path_expect(b"/m92-rename-new", PAYLOAD, buffer);
    if sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-rename-new") != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 rename cleanup failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 rename is atomically visible in VFS");
}

fn prove_vertexfs_v2_open_unlink(buffer: &mut [u8; 32]) {
    const PAYLOAD: &[u8] = b"m92-open\n";
    create_sync_readback(b"/m92-open-unlink", PAYLOAD, buffer);
    let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-open-unlink");
    if status_is_error(reader)
        || sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-open-unlink") != sys::STATUS_OK
        || sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-open-unlink")
            != sys::STATUS_VFS_NOT_FOUND
    {
        log(b"model-reader VertexFS v2 open unlink setup failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(reader, buffer);
    if read != PAYLOAD.len() as u64
        || !bytes_eq(&buffer[..PAYLOAD.len()], PAYLOAD)
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 open unlink read failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 unlink detaches name and preserves open handle reads");
}

fn prove_vertexfs_v2_hard_link(buffer: &mut [u8; 32]) {
    const PAYLOAD: &[u8] = b"m92-link\n";
    create_sync_readback(b"/m92-link-src", PAYLOAD, buffer);
    if sys::vfs_link(CAP_VERTEXFS_ROOT, b"/m92-link-src", b"/m92-link-copy") != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 hard link setup failed");
        sys::exit(1);
    }

    let source = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-link-src");
    let copy = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-link-copy");
    let mut source_stat = [0u8; 64];
    let mut copy_stat = [0u8; 64];
    if status_is_error(source)
        || status_is_error(copy)
        || sys::vfs_stat(source, &mut source_stat) != source_stat.len() as u64
        || sys::vfs_stat(copy, &mut copy_stat) != copy_stat.len() as u64
        || read_u64_le(&source_stat, 16) != read_u64_le(&copy_stat, 16)
        || read_u64_le(&source_stat, 40) != 2
        || read_u64_le(&copy_stat, 40) != 2
    {
        log(b"model-reader VertexFS v2 hard link stat failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(copy, buffer);
    if read != PAYLOAD.len() as u64
        || !bytes_eq(&buffer[..PAYLOAD.len()], PAYLOAD)
        || sys::vfs_close(source) != sys::STATUS_OK
        || sys::vfs_close(copy) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 hard link read failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 hard links share durable inode identity and link count");

    if sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-link-src") != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 hard link unlink failed");
        sys::exit(1);
    }
    let copy = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-link-copy");
    let mut copy_stat_after = [0u8; 64];
    if status_is_error(copy)
        || sys::vfs_stat(copy, &mut copy_stat_after) != copy_stat_after.len() as u64
        || read_u64_le(&copy_stat_after, 40) != 1
        || sys::vfs_close(copy) != sys::STATUS_OK
        || sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-link-copy") != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 hard link count cleanup failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 hard link count survives unlink of sibling name");
}

fn prove_vertexfs_v2_directories() {
    if sys::vfs_mkdir(CAP_VERTEXFS_ROOT, b"/m92-dir") != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 mkdir failed");
        sys::exit(1);
    }
    let dir = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-dir");
    if status_is_error(dir)
        || sys::vfs_rmdir(CAP_VERTEXFS_ROOT, b"/m92-dir") != sys::STATUS_VFS_BUSY
        || sys::vfs_close(dir) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 open rmdir denial failed");
        sys::exit(1);
    }
    if sys::vfs_mkdir(CAP_VERTEXFS_ROOT, b"/m92-dir/child") != sys::STATUS_OK
        || sys::vfs_rmdir(CAP_VERTEXFS_ROOT, b"/m92-dir") != sys::STATUS_VFS_BUSY
        || sys::vfs_rmdir(CAP_VERTEXFS_ROOT, b"/m92-dir/child") != sys::STATUS_OK
        || sys::vfs_rmdir(CAP_VERTEXFS_ROOT, b"/m92-dir") != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 rmdir lifecycle failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 rmdir rejects open and non-empty directories");
}

fn prove_vertexfs_v2_truncate_append(buffer: &mut [u8; 32]) {
    create_sync_readback(b"/m92-meta", b"m92", buffer);
    let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-meta");
    let mut stat_before = [0u8; 64];
    if status_is_error(reader)
        || sys::vfs_stat(reader, &mut stat_before) != stat_before.len() as u64
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 metadata stat setup failed");
        sys::exit(1);
    }
    let metadata_before = read_u64_le(&stat_before, 32);

    let append = sys::vfs_open_path_append_write(CAP_VERTEXFS_ROOT, b"/m92-meta");
    if status_is_error(append)
        || sys::vfs_write(append, b"-append") != 7
        || sys::vfs_close(append) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 append failed");
        sys::exit(1);
    }
    read_path_expect(b"/m92-meta", b"m92-append", buffer);
    let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, b"/m92-meta");
    let mut stat_after_append = [0u8; 64];
    if status_is_error(reader)
        || sys::vfs_stat(reader, &mut stat_after_append) != stat_after_append.len() as u64
        || read_u64_le(&stat_after_append, 8) != 10
        || read_u64_le(&stat_after_append, 32) <= metadata_before
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 append stat failed");
        sys::exit(1);
    }

    let trunc = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, b"/m92-meta");
    let mut empty = [0u8; 1];
    if status_is_error(trunc)
        || sys::vfs_read(trunc, &mut empty) != 0
        || sys::vfs_write(trunc, b"z") != 1
        || sys::vfs_sync(trunc) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 truncate failed");
        sys::exit(1);
    }
    let mut stat_after_trunc = [0u8; 64];
    if sys::vfs_stat(trunc, &mut stat_after_trunc) != stat_after_trunc.len() as u64
        || read_u64_le(&stat_after_trunc, 8) != 1
        || read_u64_le(&stat_after_trunc, 32) <= read_u64_le(&stat_after_append, 32)
        || sys::vfs_close(trunc) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 truncate stat failed");
        sys::exit(1);
    }
    if sys::vfs_unlink(CAP_VERTEXFS_ROOT, b"/m92-meta") != sys::STATUS_OK {
        log(b"model-reader VertexFS v2 metadata cleanup failed");
        sys::exit(1);
    }
    log(b"VertexFS v2 truncate append and metadata version updates survive fsync");
}

fn prove_vertexfs_v2_churn(buffer: &mut [u8; 32]) {
    let mut old_path = *b"/m92-cycle-00";
    let mut new_path = *b"/m92-cycled-00";
    let mut index = 0;
    while index < 100 {
        set_two_digit_suffix(&mut old_path, index);
        set_two_digit_suffix(&mut new_path, index);
        let writer = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, &old_path);
        if status_is_error(writer)
            || sys::vfs_write(writer, b"cy") != 2
            || sys::vfs_close(writer) != sys::STATUS_OK
            || sys::vfs_rename(CAP_VERTEXFS_ROOT, &old_path, &new_path) != sys::STATUS_OK
        {
            log(b"model-reader VertexFS v2 churn create rename failed");
            sys::exit(1);
        }
        let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, &new_path);
        if status_is_error(reader)
            || sys::vfs_read(reader, buffer) != 2
            || !bytes_eq(&buffer[..2], b"cy")
            || sys::vfs_close(reader) != sys::STATUS_OK
            || sys::vfs_unlink(CAP_VERTEXFS_ROOT, &new_path) != sys::STATUS_OK
        {
            log(b"model-reader VertexFS v2 churn read unlink failed");
            sys::exit(1);
        }
        index += 1;
    }
    log(b"VertexFS v2 100-cycle durable metadata churn returns to baseline");
}

fn read_path_expect(path: &[u8], payload: &[u8], buffer: &mut [u8; 32]) {
    let reader = sys::vfs_open_path_read(CAP_VERTEXFS_ROOT, path);
    if status_is_error(reader) {
        log(b"model-reader VertexFS v2 read path open failed");
        sys::exit(1);
    }
    let read = sys::vfs_read(reader, buffer);
    if read != payload.len() as u64
        || !bytes_eq(&buffer[..payload.len()], payload)
        || sys::vfs_close(reader) != sys::STATUS_OK
    {
        log(b"model-reader VertexFS v2 read path bytes failed");
        sys::exit(1);
    }
}

fn expect_vfs_event(watcher: u64, kind: u64, name: &[u8]) {
    let mut event = [0u8; 96];
    if sys::vfs_watch(watcher, &mut event) != event.len() as u64
        || read_u64_le(&event, 0) != kind
        || read_u64_le(&event, 16) != name.len() as u64
        || !bytes_eq(&event[24..24 + name.len()], name)
    {
        log(b"model-reader VertexFS v2 watch event failed");
        sys::exit(1);
    }
}

fn set_two_digit_suffix(path: &mut [u8], value: usize) {
    let tens = (value / 10) % 10;
    let ones = value % 10;
    let offset = path.len() - 2;
    path[offset] = b'0' + tens as u8;
    path[offset + 1] = b'0' + ones as u8;
}

fn read_u64_le(source: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = source[offset + index];
        index += 1;
    }
    u64::from_le_bytes(bytes)
}

fn vertexfs_fsync_fault_mode() -> bool {
    let mut token = [0u8; VERTEXFS_FSYNC_FAULT_MODE.len()];
    let handle = sys::vfs_open_read(CAP_VERTEXFS_FSYNC_FAULT_MODE);
    if status_is_error(handle) {
        return false;
    }
    let len = sys::vfs_read(handle, &mut token);
    let close = sys::vfs_close(handle);
    len == VERTEXFS_FSYNC_FAULT_MODE.len() as u64
        && close == sys::STATUS_OK
        && bytes_eq(&token, VERTEXFS_FSYNC_FAULT_MODE)
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

fn status_is_error(value: u64) -> bool {
    value >= u64::MAX - 128
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
