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
    if sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, b"/created12")
        == sys::STATUS_VFS_NO_SPACE
    {
        log(b"VertexFS v1 dynamic create returns no space at expanded metadata capacity");
    } else {
        log(b"model-reader VertexFS dynamic capacity denial failed");
        sys::exit(1);
    }
}

fn create_sync_readback(path: &[u8], payload: &[u8], buffer: &mut [u8; 32]) {
    let writer = sys::vfs_open_path_create_trunc_readwrite(CAP_VERTEXFS_ROOT, path);
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
