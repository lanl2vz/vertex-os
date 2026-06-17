use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn supervisor() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vertex-supervisor"))
}

fn temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tests")
        .as_millis();
    path.push(format!(
        "vertex-supervisor-test-{name}-{now}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp test dir");
    path
}

fn write_manifest(dir: &Path) -> PathBuf {
    let true_path = if Path::new("/usr/bin/true").exists() {
        "/usr/bin"
    } else {
        "/bin"
    };
    let manifest = format!(
        r#"{{
  "schema": "vertex.ir.v0",
  "generation": {{
    "id": "gen:runtime-events",
    "createdUtc": "2026-05-22T00:00:00Z",
    "description": "Runtime events test",
    "parent": null,
    "manifestHash": null
  }},
  "kernel": {{
    "id": "kernel:hosted-linux-placeholder",
    "kind": "hosted-linux",
    "storeObject": "store:true",
    "abi": "hosted-linux.v0",
    "target": "x86_64-linux"
  }},
  "init": {{
    "id": "init:vertex-init",
    "executable": "exe:true",
    "mode": "hosted-linux"
  }},
  "store": [
    {{
      "id": "store:true",
      "name": "true",
      "kind": "executable",
      "path": "{true_path}",
      "hashAlgorithm": "demo",
      "hash": "demo",
      "sizeBytes": 0,
      "references": []
    }}
  ],
  "executables": [
    {{
      "id": "exe:true",
      "storeObject": "store:true",
      "entrypoint": "true",
      "abi": "hosted-linux-process.v0",
      "argsDefault": []
    }}
  ],
  "devices": [],
  "stateVolumes": [],
  "secrets": [],
  "capabilities": [
    {{
      "id": "cap:clock.monotonic",
      "kind": "clock",
      "provider": "kernel:hosted-linux-placeholder",
      "rights": ["read"],
      "properties": {{"clock": "monotonic"}}
    }},
    {{
      "id": "cap:test.api",
      "kind": "ipc-endpoint",
      "provider": "svc:test-service",
      "rights": ["send"],
      "properties": {{"protocol": "test"}}
    }}
  ],
  "services": [
    {{
      "id": "svc:vertex-supervisor",
      "name": "vertex-supervisor",
      "executable": "exe:true",
      "args": [],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": [], "stopBefore": []}}
    }},
    {{
      "id": "svc:test-service",
      "name": "test-service",
      "executable": "exe:true",
      "args": [],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": ["cap:test.api"],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": {{"kind": "ipc-ping", "target": "cap:test.api"}},
      "lifecycle": {{"startAfter": ["svc:vertex-supervisor"], "stopBefore": []}}
    }}
  ],
  "activation": {{
    "rootService": "svc:vertex-supervisor",
    "startOrder": ["svc:vertex-supervisor", "svc:test-service"],
    "rollbackPolicy": {{"default": "system-only", "state": "preserve-unless-explicit"}},
    "onFailure": "stop-activation"
  }},
  "policies": {{
    "defaultAuthority": "deny",
    "allowAmbientFilesystem": false,
    "allowAmbientNetwork": false,
    "allowAmbientDevices": false,
    "capabilityDelegation": "explicit-only",
    "unknownReferences": "reject"
  }}
}}"#
    );
    let path = dir.join("runtime-events.vertex.json");
    fs::write(&path, manifest).expect("write test manifest");
    path
}

fn write_negative_grant_manifest(dir: &Path) -> PathBuf {
    let shell_root = if Path::new("/bin/sh").exists() {
        "/bin"
    } else {
        "/usr/bin"
    };
    let manifest = format!(
        r#"{{
  "schema": "vertex.ir.v0",
  "generation": {{
    "id": "gen:negative-grant",
    "createdUtc": "2026-05-22T00:00:00Z",
    "description": "Negative grant test",
    "parent": null,
    "manifestHash": null
  }},
  "kernel": {{
    "id": "kernel:hosted-linux-placeholder",
    "kind": "hosted-linux",
    "storeObject": "store:sh",
    "abi": "hosted-linux.v0",
    "target": "x86_64-linux"
  }},
  "init": {{
    "id": "init:vertex-init",
    "executable": "exe:sh",
    "mode": "hosted-linux"
  }},
  "store": [
    {{
      "id": "store:sh",
      "name": "sh",
      "kind": "executable",
      "path": "{shell_root}",
      "hashAlgorithm": "demo",
      "hash": "demo",
      "sizeBytes": 0,
      "references": []
    }}
  ],
  "executables": [
    {{
      "id": "exe:sh",
      "storeObject": "store:sh",
      "entrypoint": "sh",
      "abi": "hosted-linux-process.v0",
      "argsDefault": []
    }}
  ],
  "devices": [],
  "stateVolumes": [],
  "secrets": [],
  "capabilities": [
    {{
      "id": "cap:clock.monotonic",
      "kind": "clock",
      "provider": "kernel:hosted-linux-placeholder",
      "rights": ["read"],
      "properties": {{"clock": "monotonic"}}
    }},
    {{
      "id": "cap:log.sink",
      "kind": "ipc-endpoint",
      "provider": "svc:logd",
      "rights": ["send"],
      "properties": {{"protocol": "vertex.log.v1"}}
    }}
  ],
  "services": [
    {{
      "id": "svc:vertex-supervisor",
      "name": "vertex-supervisor",
      "executable": "exe:sh",
      "args": ["-c", "exit 0"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": [], "stopBefore": []}}
    }},
    {{
      "id": "svc:logd",
      "name": "logd",
      "executable": "exe:sh",
      "args": ["-c", "exit 0"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": ["cap:log.sink"],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": ["svc:vertex-supervisor"], "stopBefore": []}}
    }},
    {{
      "id": "svc:echo-server",
      "name": "echo-server",
      "executable": "exe:sh",
      "args": ["-c", "case \"$VERTEX_GRANTED_CAPS\" in *cap:log.sink*) exit 1;; *) exit 2;; esac"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": ["svc:logd"], "stopBefore": []}}
    }}
  ],
  "activation": {{
    "rootService": "svc:vertex-supervisor",
    "startOrder": ["svc:vertex-supervisor", "svc:logd", "svc:echo-server"],
    "rollbackPolicy": {{"default": "system-only", "state": "preserve-unless-explicit"}},
    "onFailure": "stop-activation"
  }},
  "policies": {{
    "defaultAuthority": "deny",
    "allowAmbientFilesystem": false,
    "allowAmbientNetwork": false,
    "allowAmbientDevices": false,
    "capabilityDelegation": "explicit-only",
    "unknownReferences": "reject"
  }}
}}"#
    );
    let path = dir.join("negative-grant.vertex.json");
    fs::write(&path, manifest).expect("write negative grant manifest");
    path
}

fn write_state_owner_mismatch_manifest(dir: &Path) -> PathBuf {
    let shell_root = if Path::new("/bin/sh").exists() {
        "/bin"
    } else {
        "/usr/bin"
    };
    let manifest = format!(
        r#"{{
  "schema": "vertex.ir.v0",
  "generation": {{
    "id": "gen:state-owner-mismatch",
    "createdUtc": "2026-05-22T00:00:00Z",
    "description": "State owner mismatch test",
    "parent": null,
    "manifestHash": null
  }},
  "kernel": {{
    "id": "kernel:hosted-linux-placeholder",
    "kind": "hosted-linux",
    "storeObject": "store:sh",
    "abi": "hosted-linux.v0",
    "target": "x86_64-linux"
  }},
  "init": {{
    "id": "init:vertex-init",
    "executable": "exe:sh",
    "mode": "hosted-linux"
  }},
  "store": [
    {{
      "id": "store:sh",
      "name": "sh",
      "kind": "executable",
      "path": "{shell_root}",
      "hashAlgorithm": "demo",
      "hash": "demo",
      "sizeBytes": 0,
      "references": []
    }}
  ],
  "executables": [
    {{
      "id": "exe:sh",
      "storeObject": "store:sh",
      "entrypoint": "sh",
      "abi": "hosted-linux-process.v0",
      "argsDefault": []
    }}
  ],
  "devices": [],
  "stateVolumes": [
    {{
      "id": "state:data",
      "name": "data",
      "kind": "hosted-local-directory",
      "owner": "svc:owner",
      "mountIntent": "private",
      "snapshotPolicy": {{"enabled": true, "mode": "directory-copy"}},
      "backupPolicy": {{"enabled": false}}
    }}
  ],
  "secrets": [],
  "capabilities": [
    {{
      "id": "cap:clock.monotonic",
      "kind": "clock",
      "provider": "kernel:hosted-linux-placeholder",
      "rights": ["read"],
      "properties": {{"clock": "monotonic"}}
    }}
  ],
  "services": [
    {{
      "id": "svc:vertex-supervisor",
      "name": "vertex-supervisor",
      "executable": "exe:sh",
      "args": ["-c", "exit 0"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": [], "stopBefore": []}}
    }},
    {{
      "id": "svc:owner",
      "name": "owner",
      "executable": "exe:sh",
      "args": ["-c", "exit 0"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": [],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": ["svc:vertex-supervisor"], "stopBefore": []}}
    }},
    {{
      "id": "svc:consumer",
      "name": "consumer",
      "executable": "exe:sh",
      "args": ["-c", "exit 0"],
      "env": {{}},
      "mountRoot": "/",
      "requires": [
        {{"capability": "cap:clock.monotonic", "rights": ["read"]}}
      ],
      "provides": [],
      "state": ["state:data"],
      "secrets": [],
      "restart": "never",
      "resources": {{}},
      "health": null,
      "lifecycle": {{"startAfter": ["svc:vertex-supervisor"], "stopBefore": []}}
    }}
  ],
  "activation": {{
    "rootService": "svc:vertex-supervisor",
    "startOrder": ["svc:vertex-supervisor", "svc:consumer"],
    "rollbackPolicy": {{"default": "system-only", "state": "preserve-unless-explicit"}},
    "onFailure": "stop-activation"
  }},
  "policies": {{
    "defaultAuthority": "deny",
    "allowAmbientFilesystem": false,
    "allowAmbientNetwork": false,
    "allowAmbientDevices": false,
    "capabilityDelegation": "explicit-only",
    "unknownReferences": "reject"
  }}
}}"#
    );
    let path = dir.join("state-owner-mismatch.vertex.json");
    fs::write(&path, manifest).expect("write state mismatch manifest");
    path
}

fn assert_success(output: Output) {
    if !output.status.success() {
        panic!(
            "command failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_failure(output: Output) -> String {
    if output.status.success() {
        panic!(
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn read_events(state_root: &Path) -> Vec<Value> {
    let events_path = state_root.join("runtime-events.jsonl");
    let events = fs::read_to_string(&events_path).expect("read runtime events");
    events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event should be json"))
        .collect::<Vec<_>>()
}

#[test]
fn supervisor_writes_runtime_events_with_health_metadata() {
    let dir = temp_dir("events");
    let manifest = write_manifest(&dir);
    let state_root = dir.join("state");

    assert_success(
        Command::new(supervisor())
            .arg("--state-root")
            .arg(&state_root)
            .arg("--run-once")
            .arg(&manifest)
            .output()
            .expect("run supervisor"),
    );

    let parsed = read_events(&state_root);

    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "activationStart")
    );
    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "serviceSkippedRoot")
    );
    assert!(parsed.iter().any(|event| event["event"] == "serviceStart"
        && event["serviceId"] == "svc:test-service"
        && event["health"]["target"] == "cap:test.api"));
    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "capabilityGrant"
                && event["capabilityId"] == "cap:clock.monotonic"
                && event["consumer"] == "svc:test-service"
                && event["endpoint"]["type"] == "hostClock")
    );
    assert!(parsed.iter().any(|event| {
        event["event"] == "serviceStart"
            && event["serviceId"] == "svc:test-service"
            && event["providedCapabilities"]
                .as_array()
                .expect("provided capabilities")
                .iter()
                .any(|capability| capability["id"] == "cap:test.api")
    }));
    assert!(parsed.iter().any(|event| event["event"] == "serviceExit"
        && event["serviceId"] == "svc:test-service"
        && event["success"] == true));
    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "activationSuccess")
    );
}

#[test]
fn undeclared_capability_is_not_granted_to_consumer() {
    let dir = temp_dir("negative-grant");
    let manifest = write_negative_grant_manifest(&dir);
    let state_root = dir.join("state");

    let stderr = assert_failure(
        Command::new(supervisor())
            .arg("--state-root")
            .arg(&state_root)
            .arg("--run-once")
            .arg(&manifest)
            .output()
            .expect("run supervisor"),
    );
    assert!(stderr.contains("svc:echo-server exited"));

    let parsed = read_events(&state_root);
    let echo_start = parsed
        .iter()
        .find(|event| event["event"] == "serviceStart" && event["serviceId"] == "svc:echo-server")
        .expect("echo start event");
    assert!(
        !echo_start["grantedCapabilities"]
            .as_array()
            .expect("granted capabilities")
            .iter()
            .any(|capability| capability["id"] == "cap:log.sink")
    );
    assert!(
        !parsed
            .iter()
            .any(|event| event["event"] == "capabilityGrant"
                && event["capabilityId"] == "cap:log.sink"
                && event["consumer"] == "svc:echo-server")
    );

    let log_ready_index = parsed
        .iter()
        .position(|event| event["event"] == "serviceReady" && event["serviceId"] == "svc:logd")
        .expect("logd ready event");
    let echo_start_index = parsed
        .iter()
        .position(|event| {
            event["event"] == "serviceStart" && event["serviceId"] == "svc:echo-server"
        })
        .expect("echo start event index");
    assert!(log_ready_index < echo_start_index);
    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "activationFailure")
    );
}

#[test]
fn state_volume_grants_reject_non_owner_consumers() {
    let dir = temp_dir("state-owner");
    let manifest = write_state_owner_mismatch_manifest(&dir);
    let state_root = dir.join("state");

    let stderr = assert_failure(
        Command::new(supervisor())
            .arg("--state-root")
            .arg(&state_root)
            .arg("--run-once")
            .arg(&manifest)
            .output()
            .expect("run supervisor"),
    );

    assert!(stderr.contains("state volume state:data is owned by svc:owner"));
    let parsed = read_events(&state_root);
    assert!(parsed.iter().any(|event| {
        event["event"] == "activationFailure"
            && event["error"]
                .as_str()
                .expect("activation failure error")
                .contains("cannot be granted")
    }));
}
