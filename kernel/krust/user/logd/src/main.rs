#![no_std]
#![no_main]

mod sys;

use core::panic::PanicInfo;

const CAP_LOG_SINK: u64 = 0;
const CAP_SERIAL_LOG: u64 = 1;
const CAP_READINESS: u64 = 2;
const CAP_SERIAL_DRIVER: u64 = 3;
const CAP_LOG_STREAM: u64 = 4;
const CAP_CONFIG: u64 = 5;
const CAP_SECRET: u64 = 6;
const ECHO_PROCESS_INDEX: u64 = 2;
const PROTOCOL_HEALTH_V0: u16 = 2;
const MESSAGE_READY: u16 = 1;
const ENVELOPE_LEN: usize = 16;

#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let ready = ready_message(b"logd");
    if sys::ipc_send(CAP_READINESS, &ready) != sys::STATUS_OK {
        log(b"logd ready send failed");
        sys::exit(1);
    }
    let log_stream = sys::vfs_open_path_read(CAP_LOG_STREAM, b"/proc/log-stream");
    if status_is_error(log_stream) {
        log(b"logd log-stream open failed");
        sys::exit(1);
    }
    let mut stream = [0u8; 64];
    let stream_len = sys::vfs_read(log_stream, &mut stream);
    if status_is_error(stream_len) || stream_len == 0 || stream_len > stream.len() as u64 {
        log(b"logd log-stream read failed");
        sys::exit(1);
    }
    if sys::vfs_close(log_stream) != sys::STATUS_OK {
        log(b"logd log-stream close failed");
        sys::exit(1);
    }
    log(b"VFS pipe read blocks until writer log");
    log(b"logd ready");

    if sys::ipc_send(CAP_SERIAL_DRIVER, b"logd sends log message") != sys::STATUS_OK {
        log(b"logd serial-driver send failed");
        sys::exit(1);
    }
    if sys::io_write(CAP_SERIAL_DRIVER, 0x3f8, b'!') == sys::STATUS_BAD_CAPABILITY {
        log(b"logd cannot write COM1 directly");
    } else {
        log(b"logd COM1 denial failed");
        sys::exit(1);
    }

    let config_handle = sys::vfs_open_read(CAP_CONFIG);
    if config_handle == sys::STATUS_BAD_CAPABILITY {
        log(b"logd config open failed");
        sys::exit(1);
    }
    let mut config_stat = [0u8; 64];
    if sys::vfs_stat(config_handle, &mut config_stat) != config_stat.len() as u64 {
        log(b"logd config stat failed");
        sys::exit(1);
    }
    let mut config = [0u8; 64];
    let config_len = sys::vfs_read(config_handle, &mut config);
    if config_len == sys::STATUS_BAD_CAPABILITY || config_len > config.len() as u64 {
        log(b"logd config read failed");
        sys::exit(1);
    }
    log(b"logd reads config through VFS handle");
    let mut pread = [0u8; 4];
    let mut eof = [0u8; 4];
    if sys::vfs_pread(config_handle, &mut pread, 0) == pread.len() as u64
        && bytes_eq(&pread, &config[..4])
        && sys::vfs_read(config_handle, &mut eof) == 0
    {
        log(b"VFS pread does not mutate file offset");
    } else {
        log(b"VFS pread offset test failed");
        sys::exit(1);
    }

    let mut secret = [0u8; 64];
    let secret_len = sys::secret_read(CAP_SECRET, &mut secret);
    if secret_len == sys::STATUS_BAD_CAPABILITY
        || secret_len == 0
        || secret_len > secret.len() as u64
    {
        log(b"logd secret read failed");
        sys::exit(1);
    }
    log(b"service with secret cap reads secret");

    if sys::vfs_seek_set(config_handle, 0) != sys::STATUS_OK {
        log(b"logd config seek failed");
        sys::exit(1);
    }
    if sys::ipc_recv_raw(CAP_LOG_SINK, 1, 8) == sys::STATUS_BAD_BUFFER
        && sys::vfs_read_raw(config_handle, 1, 8) == sys::STATUS_BAD_BUFFER
        && sys::legacy_object_read_raw(CAP_CONFIG, config.as_mut_ptr() as u64, 8)
            == sys::STATUS_BAD_CAPABILITY
    {
        log(b"M61 provider malformed receive/read buffers rejected");
        log(b"legacy object-read syscall rejected");
    } else {
        log(b"M61 provider malformed buffer test failed");
        sys::exit(1);
    }
    if sys::vfs_seek_set(config_handle, 0) != sys::STATUS_OK {
        log(b"logd config seek for dup failed");
        sys::exit(1);
    }
    let shared = sys::vfs_dup_shared(config_handle);
    if shared == sys::STATUS_BAD_CAPABILITY {
        log(b"logd VFS shared dup failed");
        sys::exit(1);
    }
    let mut first = [0u8; 4];
    let mut second = [0u8; 4];
    if sys::vfs_read(config_handle, &mut first) == first.len() as u64
        && sys::vfs_read(shared, &mut second) == second.len() as u64
        && bytes_eq(&first, &config[..4])
        && bytes_eq(&second, &config[4..8])
    {
        log(b"VFS dup shared handle shares offset");
    } else {
        log(b"VFS shared dup offset test failed");
        sys::exit(1);
    }
    if sys::vfs_close(shared) != sys::STATUS_OK {
        log(b"logd VFS shared dup close failed");
        sys::exit(1);
    }

    if sys::vfs_seek_set(config_handle, 0) != sys::STATUS_OK {
        log(b"logd config seek for independent dup failed");
        sys::exit(1);
    }
    let independent = sys::vfs_dup(config_handle);
    if independent == sys::STATUS_BAD_CAPABILITY {
        log(b"logd VFS independent dup failed");
        sys::exit(1);
    }
    first = [0u8; 4];
    second = [0u8; 4];
    if sys::vfs_read(config_handle, &mut first) == first.len() as u64
        && sys::vfs_read(independent, &mut second) == second.len() as u64
        && bytes_eq(&first, &config[..4])
        && bytes_eq(&second, &config[..4])
    {
        log(b"VFS dup independent handle keeps separate offset");
    } else {
        log(b"VFS independent dup offset test failed");
        sys::exit(1);
    }
    if sys::vfs_close(independent) != sys::STATUS_OK {
        log(b"logd VFS independent dup close failed");
        sys::exit(1);
    }
    if sys::vfs_write(config_handle, b"x") == sys::STATUS_VFS_PERMISSION {
        log(b"read-only VFS handle rejects write");
    } else {
        log(b"read-only VFS write rejection failed");
        sys::exit(1);
    }
    if sys::vfs_sync(config_handle) != sys::STATUS_OK {
        log(b"logd VFS sync failed");
        sys::exit(1);
    }
    if sys::vfs_close(0) == sys::STATUS_VFS_BAD_HANDLE {
        log(b"invalid VFS close returns controlled error");
    } else {
        log(b"invalid VFS close denial failed");
        sys::exit(1);
    }
    let mut extra_handles = [0u64; 15];
    let mut extra_index = 0;
    while extra_index < extra_handles.len() {
        let extra = sys::vfs_open_read(CAP_CONFIG);
        if extra == sys::STATUS_VFS_NO_SPACE {
            log(b"VFS handle quota setup failed");
            sys::exit(1);
        }
        extra_handles[extra_index] = extra;
        extra_index += 1;
    }
    if sys::vfs_open_read(CAP_CONFIG) == sys::STATUS_VFS_NO_SPACE {
        log(b"VFS handle quota exhaustion rejects without leak");
    } else {
        log(b"VFS handle quota rejection failed");
        sys::exit(1);
    }
    extra_index = 0;
    while extra_index < extra_handles.len() {
        if sys::vfs_close(extra_handles[extra_index]) != sys::STATUS_OK {
            log(b"VFS handle quota cleanup failed");
            sys::exit(1);
        }
        extra_index += 1;
    }
    if sys::vfs_close(config_handle) != sys::STATUS_OK {
        log(b"logd config close failed");
        sys::exit(1);
    }

    let mut buffer = [0u8; 64];
    let received = sys::ipc_recv(CAP_LOG_SINK, &mut buffer);
    if received > buffer.len() as u64 {
        log(b"logd receive failed");
        sys::exit(1);
    }

    log_prefix(b"logd received: ", &buffer[..received as usize]);

    if sys::process_create(CAP_LOG_SINK, ECHO_PROCESS_INDEX) == sys::STATUS_BAD_CAPABILITY {
        log(b"unprivileged service calls SYS_PROCESS_CREATE");
        log(b"negative test: logd process-create rejected: bad capability");
    } else {
        log(b"logd negative process-create failed");
        sys::exit(1);
    }

    sys::exit(0)
}

fn log(message: &[u8]) {
    if sys::log(CAP_SERIAL_LOG, message) != sys::STATUS_OK {
        sys::exit(1);
    }
}

fn log_prefix(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 128];
    let len = append(&mut buffer, 0, prefix);
    let len = append(&mut buffer, len, value);
    log(&buffer[..len]);
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
    value >= u64::MAX - 64
}

fn append(buffer: &mut [u8], mut offset: usize, value: &[u8]) -> usize {
    let mut index = 0;
    while offset < buffer.len() && index < value.len() {
        buffer[offset] = value[index];
        offset += 1;
        index += 1;
    }
    offset
}

fn ready_message(service: &[u8]) -> [u8; 32] {
    let mut message = [0u8; 32];
    write_u16(&mut message, 0, PROTOCOL_HEALTH_V0);
    write_u16(&mut message, 2, MESSAGE_READY);
    write_u32(&mut message, 4, service.len() as u32);
    write_u64(&mut message, 8, 1);
    let mut index = 0;
    while index < service.len() && ENVELOPE_LEN + index < message.len() {
        message[ENVELOPE_LEN + index] = service[index];
        index += 1;
    }
    message
}

fn write_u16(buffer: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buffer[offset] = bytes[0];
    buffer[offset + 1] = bytes[1];
    buffer[offset + 2] = bytes[2];
    buffer[offset + 3] = bytes[3];
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        buffer[offset + index] = bytes[index];
        index += 1;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys::exit(1)
}
