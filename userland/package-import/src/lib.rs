#![no_std]

pub const PACKAGE_ID: &[u8] = b"pkg:vertex.package-import";
pub const SERVICE_ID: &[u8] = b"svc:package-import";

pub const EXPECTED_AUTHORITY_DELTA: &[u8] = b"cap:console.output/send,cap:vfs.logd-log-stream/resolve+read,cap:net.udp.9000/listen+bind,cap:log.sink/send,config:logd/read";
pub const EXPECTED_CLOSURE_MATERIAL: &[u8] = b"packages=pkg:logd;services=svc:echo-server,svc:logd;objects=config:logd,store:echo-server-demo,store:logd-demo";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub message: &'static [u8],
}

impl Error {
    pub const fn new(message: &'static [u8]) -> Self {
        Self { message }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportFragment<'a> {
    pub package: &'a [u8],
    pub service: &'a [u8],
    pub capability: &'a [u8],
    pub activated_service: &'a [u8],
    pub candidate: &'a [u8],
    pub authority_delta: &'a [u8],
    pub object: &'a [u8],
    pub object_size_field: &'a [u8],
    pub closure_hash: &'a [u8],
}

pub fn validate_logd_fragment<'a>(
    fragment: &'a [u8],
    config_object: &[u8],
) -> Result<ImportFragment<'a>> {
    validate_fragment_magic(fragment)?;
    assert_field_eq(fragment, b"kind=", b"import")?;

    let package = required_field(fragment, b"package=")?;
    assert_bytes(package, b"pkg:logd")?;
    let candidate = required_field(fragment, b"candidate=")?;
    assert_bytes(candidate, b"gen:package-import-new-0002")?;
    let service = required_field(fragment, b"add_service=")?;
    assert_bytes(service, b"svc:logd")?;
    let capability = required_field(fragment, b"add_capability=")?;
    assert_bytes(capability, b"cap:log.sink")?;
    let activated_service = required_field(fragment, b"activate_service=")?;
    assert_bytes(activated_service, b"svc:echo-server")?;
    assert_field_eq(
        fragment,
        b"requires_base=",
        b"cap:console.output,cap:vfs.logd-log-stream,cap:net.udp.9000",
    )?;
    assert_field_eq(fragment, b"requires_import=", b"cap:log.sink")?;

    let authority_delta = required_field(fragment, b"authority_delta=")?;
    assert_bytes(authority_delta, EXPECTED_AUTHORITY_DELTA)?;
    let object = required_field(fragment, b"object=")?;
    assert_bytes(object, b"config:logd")?;
    let object_size_field = required_field(fragment, b"object_size=")?;
    let object_size = parse_decimal(object_size_field).ok_or(Error::new(
        b"package-import rejected graph fragment: bad object_size",
    ))?;
    if config_object.len() != object_size {
        return Err(Error::new(
            b"package-import rejected store object: size mismatch",
        ));
    }
    let object_hash = required_field(fragment, b"object_hash=")?;
    verify_blake3(config_object, object_hash)
        .map_err(|_| Error::new(b"package-import hash mismatch: store-object hash"))?;

    let closure_material = required_field(fragment, b"closure_material=")?;
    assert_bytes(closure_material, EXPECTED_CLOSURE_MATERIAL)?;
    let closure_hash = required_field(fragment, b"closure_hash=")?;
    verify_blake3(closure_material, closure_hash)
        .map_err(|_| Error::new(b"package-import hash mismatch: closure hash"))?;

    Ok(ImportFragment {
        package,
        service,
        capability,
        activated_service,
        candidate,
        authority_delta,
        object,
        object_size_field,
        closure_hash,
    })
}

pub fn validate_missing_dependency_fragment(fragment: &[u8]) -> Result<&[u8]> {
    validate_fragment_magic(fragment)?;
    assert_field_eq(fragment, b"kind=", b"negative-missing-dependency")?;
    let missing = required_field(fragment, b"require=")?;
    if provider_known(missing) {
        return Err(Error::new(
            b"package-import negative dependency unexpectedly resolved",
        ));
    }
    Ok(missing)
}

pub fn validate_excess_authority_fragment(fragment: &[u8]) -> Result<&[u8]> {
    validate_fragment_magic(fragment)?;
    assert_field_eq(fragment, b"kind=", b"negative-excess-authority")?;
    let grant = required_field(fragment, b"grant=")?;
    if authority_allowed(grant) {
        return Err(Error::new(
            b"package-import negative authority unexpectedly allowed",
        ));
    }
    Ok(grant)
}

fn validate_fragment_magic(fragment: &[u8]) -> Result<()> {
    if starts_with(fragment, b"PKGFRAGV1\n") {
        Ok(())
    } else {
        Err(Error::new(
            b"package-import rejected graph fragment: bad magic",
        ))
    }
}

fn assert_field_eq(fragment: &[u8], key: &[u8], expected: &[u8]) -> Result<()> {
    assert_bytes(required_field(fragment, key)?, expected)
}

fn assert_bytes(value: &[u8], expected: &[u8]) -> Result<()> {
    if bytes_eq(value, expected) {
        Ok(())
    } else {
        Err(Error::new(
            b"package-import rejected graph fragment: unexpected field",
        ))
    }
}

fn required_field<'a>(fragment: &'a [u8], key: &[u8]) -> Result<&'a [u8]> {
    field(fragment, key).ok_or(Error::new(
        b"package-import rejected graph fragment: missing field",
    ))
}

fn field<'a>(fragment: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    let mut start = 0;
    while start <= fragment.len() {
        let mut end = start;
        while end < fragment.len() && fragment[end] != b'\n' {
            end += 1;
        }
        let line = &fragment[start..end];
        if starts_with(line, key) {
            return Some(&line[key.len()..]);
        }
        if end == fragment.len() {
            break;
        }
        start = end + 1;
    }
    None
}

fn verify_blake3(bytes: &[u8], expected_hex: &[u8]) -> core::result::Result<(), ()> {
    let mut actual = [0u8; 64];
    blake3_hex(bytes, &mut actual);
    if expected_hex.len() == actual.len() && bytes_eq(expected_hex, &actual) {
        Ok(())
    } else {
        Err(())
    }
}

fn blake3_hex(bytes: &[u8], out: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = blake3::hash(bytes);
    let raw = digest.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        out[index * 2] = HEX[(raw[index] >> 4) as usize];
        out[index * 2 + 1] = HEX[(raw[index] & 0x0f) as usize];
        index += 1;
    }
}

fn parse_decimal(value: &[u8]) -> Option<usize> {
    if value.is_empty() {
        return None;
    }
    let mut out = 0usize;
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.checked_mul(10)?;
        out = out.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(out)
}

fn provider_known(capability: &[u8]) -> bool {
    bytes_eq(capability, b"cap:console.output")
        || bytes_eq(capability, b"cap:vfs.logd-log-stream")
        || bytes_eq(capability, b"cap:net.udp.9000")
        || bytes_eq(capability, b"cap:log.sink")
}

fn authority_allowed(grant: &[u8]) -> bool {
    bytes_eq(grant, b"cap:console.output/send")
        || bytes_eq(grant, b"cap:vfs.logd-log-stream/resolve+read")
        || bytes_eq(grant, b"cap:net.udp.9000/listen+bind")
        || bytes_eq(grant, b"cap:log.sink/send")
        || bytes_eq(grant, b"config:logd/read")
}

fn starts_with(value: &[u8], prefix: &[u8]) -> bool {
    value.len() >= prefix.len() && bytes_eq(&value[..prefix.len()], prefix)
}

fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && {
        let mut index = 0;
        while index < left.len() {
            if left[index] != right[index] {
                return false;
            }
            index += 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_config_hash() {
        let fragment = b"PKGFRAGV1\nkind=import\npackage=pkg:logd\ncandidate=gen:package-import-new-0002\nadd_service=svc:logd\nadd_capability=cap:log.sink\nactivate_service=svc:echo-server\nrequires_base=cap:console.output,cap:vfs.logd-log-stream,cap:net.udp.9000\nrequires_import=cap:log.sink\nauthority_delta=cap:console.output/send,cap:vfs.logd-log-stream/resolve+read,cap:net.udp.9000/listen+bind,cap:log.sink/send,config:logd/read\nobject=config:logd\nobject_size=3\nobject_hash=0000000000000000000000000000000000000000000000000000000000000000\nclosure_material=packages=pkg:logd;services=svc:echo-server,svc:logd;objects=config:logd,store:echo-server-demo,store:logd-demo\nclosure_hash=0000000000000000000000000000000000000000000000000000000000000000\n";
        assert!(validate_logd_fragment(fragment, b"bad").is_err());
    }
}
