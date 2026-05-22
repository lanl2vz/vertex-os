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
      "rights": ["sendrecv"],
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

    let events_path = state_root.join("runtime-events.jsonl");
    let events = fs::read_to_string(&events_path).expect("read runtime events");
    let parsed = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event should be json"))
        .collect::<Vec<_>>();

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
    assert!(parsed.iter().any(|event| event["event"] == "serviceExit"
        && event["serviceId"] == "svc:test-service"
        && event["success"] == true));
    assert!(
        parsed
            .iter()
            .any(|event| event["event"] == "activationSuccess")
    );
}
