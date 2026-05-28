use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn vertexctl() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vertexctl"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("vertexctl crate should be under crates/")
        .to_path_buf()
}

fn temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tests")
        .as_millis();
    path.push(format!(
        "vertexctl-test-{name}-{now}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp test dir");
    path
}

fn fake_supervisor(dir: &Path) -> PathBuf {
    let path = dir.join("fake-supervisor.sh");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write fake supervisor");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&path)
            .expect("stat fake supervisor")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod fake supervisor");
    }
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(vertexctl())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("run vertexctl")
}

fn run_with_fake_supervisor(args: &[&str], fake: &Path) -> Output {
    Command::new(vertexctl())
        .args(args)
        .env("VERTEX_SUPERVISOR_BIN", fake)
        .current_dir(repo_root())
        .output()
        .expect("run vertexctl")
}

fn write_missing_executable_manifest(path: &Path) {
    let manifest = serde_json::json!({
        "schema": "vertex.ir.v0",
        "generation": {
            "id": "gen:missing-executable-artifact",
            "createdUtc": "2026-05-27T00:00:00Z",
            "description": "missing native executable artifact test",
            "parent": null,
            "manifestHash": null
        },
        "kernel": {
            "id": "kernel:krust-placeholder",
            "kind": "krust-native",
            "storeObject": "store:kernel-placeholder",
            "abi": "krust.v0",
            "target": "x86_64-unknown-none"
        },
        "init": {
            "id": "init:vertex-init",
            "executable": "exe:missing",
            "mode": "krust-native"
        },
        "store": [
            {
                "id": "store:kernel-placeholder",
                "name": "kernel-placeholder",
                "kind": "kernel-image",
                "path": "/vertex/store/kernel-placeholder",
                "hashAlgorithm": "blake3",
                "hash": "demo-not-a-real-hash-kernel",
                "sizeBytes": 0,
                "references": []
            },
            {
                "id": "store:missing-demo",
                "name": "missing-demo",
                "kind": "executable",
                "path": "/vertex/store/definitely-missing-executable-for-test",
                "hashAlgorithm": "blake3",
                "hash": "demo-not-a-real-hash-missing",
                "sizeBytes": 65536,
                "references": []
            }
        ],
        "executables": [
            {
                "id": "exe:missing",
                "storeObject": "store:missing-demo",
                "entrypoint": "bin/definitely-missing-executable-for-test",
                "abi": "krust-native-process.v0"
            }
        ],
        "devices": [],
        "stateVolumes": [],
        "secrets": [],
        "capabilities": [],
        "services": [
            {
                "id": "svc:vertex-supervisor",
                "name": "vertex-init",
                "executable": "exe:missing",
                "restart": "never",
                "health": null,
                "lifecycle": {
                    "startAfter": [],
                    "stopBefore": []
                }
            }
        ],
        "activation": {
            "rootService": "svc:vertex-supervisor",
            "startOrder": ["svc:vertex-supervisor"],
            "rollbackPolicy": {},
            "onFailure": "stop-activation"
        },
        "policies": {
            "defaultAuthority": "deny",
            "allowAmbientFilesystem": false,
            "allowAmbientNetwork": false,
            "allowAmbientDevices": false,
            "capabilityDelegation": "explicit-only",
            "unknownReferences": "reject"
        }
    });
    fs::write(
        path,
        serde_json::to_string_pretty(&manifest).expect("serialize missing artifact manifest"),
    )
    .expect("write missing artifact manifest");
}

fn assert_success(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "command failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).expect("stdout should be utf-8")
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

#[test]
fn validates_both_example_manifests() {
    let hello = assert_success(run(&["validate", "examples/hello-generation.vertex.json"]));
    assert!(hello.contains("valid: gen:hello-0001"));

    let stateful = assert_success(run(&[
        "validate",
        "examples/hello-stateful-generation.vertex.json",
    ]));
    assert!(stateful.contains("valid: gen:hello-stateful-0001"));

    let deny_log = assert_success(run(&[
        "validate",
        "examples/deny-log-generation.vertex.json",
    ]));
    assert!(deny_log.contains("valid: gen:deny-log-0001"));

    let readiness_timeout = assert_success(run(&[
        "validate",
        "examples/krust-readiness-timeout.vertex.json",
    ]));
    assert!(readiness_timeout.contains("valid: gen:readiness-timeout-0001"));

    let block_driver_fault = assert_success(run(&[
        "validate",
        "examples/krust-block-driver-fault-generation.vertex.json",
    ]));
    assert!(block_driver_fault.contains("valid: gen:block-driver-fault-0001"));
}

#[test]
fn status_and_inspect_json_report_activation_metadata() {
    let dir = temp_dir("status-inspect");
    let state_root = dir.join("state");
    let fake = fake_supervisor(&dir);
    let state_root_arg = state_root.to_string_lossy().to_string();

    assert_success(run_with_fake_supervisor(
        &[
            "activate",
            "examples/hello-stateful-generation.vertex.json",
            "--state-root",
            &state_root_arg,
            "--run-once",
        ],
        &fake,
    ));

    assert!(state_root.join("current.json").exists());
    assert!(state_root.join("history.jsonl").exists());
    let activation_dirs = fs::read_dir(state_root.join("activations"))
        .expect("read activations")
        .collect::<Result<Vec<_>, _>>()
        .expect("activation entries");
    assert_eq!(activation_dirs.len(), 1);
    let activation_dir = activation_dirs[0].path();
    assert!(activation_dir.join("manifest.vertex.json").exists());
    assert!(activation_dir.join("activation.json").exists());

    let status_stdout = assert_success(run(&["status", "--state-root", &state_root_arg, "--json"]));
    let status: Value = serde_json::from_str(&status_stdout).expect("status should be json");
    assert_eq!(status["activationCount"], 1);
    assert_eq!(status["current"]["generationId"], "gen:hello-stateful-0001");

    let inspect_stdout = assert_success(run(&[
        "inspect",
        "current",
        "--state-root",
        &state_root_arg,
        "--json",
    ]));
    let inspect: Value = serde_json::from_str(&inspect_stdout).expect("inspect should be json");
    assert_eq!(
        inspect["activation"]["generationId"],
        "gen:hello-stateful-0001"
    );
    assert!(
        inspect["activation"]["sourceManifestPath"]
            .as_str()
            .expect("source manifest path")
            .ends_with("examples/hello-stateful-generation.vertex.json")
    );
    assert!(
        inspect["activation"]["storedManifestPath"]
            .as_str()
            .expect("stored manifest path")
            .ends_with("manifest.vertex.json")
    );
    assert_eq!(
        inspect["activation"]["stateSnapshots"]
            .as_array()
            .expect("state snapshots")
            .len(),
        1
    );
    assert!(
        inspect["services"]
            .as_array()
            .expect("services array")
            .len()
            >= 4
    );
    assert!(
        inspect["capabilities"]
            .as_array()
            .expect("capabilities array")
            .iter()
            .any(|capability| capability["id"] == "cap:log.sink")
    );
}

#[test]
fn who_can_json_reports_capability_authority() {
    let stdout = assert_success(run(&[
        "who-can",
        "examples/hello-generation.vertex.json",
        "cap:log.sink",
        "--json",
    ]));
    let output: Value = serde_json::from_str(&stdout).expect("who-can should be json");
    assert_eq!(output["capability"], "cap:log.sink");
    assert_eq!(output["provider"], "svc:logd");
    let services = output["services"].as_array().expect("services array");
    assert_eq!(services.len(), 1);
    assert_eq!(services[0]["service"], "svc:echo-server");
    assert_eq!(services[0]["fullyGranted"], true);
}

#[test]
fn compile_boot_manifest_emits_krustboot_plan() {
    let dir = temp_dir("krustboot");
    let output_path = dir.join("hello-generation.krustboot");
    let output_arg = output_path.to_string_lossy().to_string();

    let stdout = assert_success(run(&[
        "compile-boot-manifest",
        "examples/hello-generation.vertex.json",
        &output_arg,
    ]));

    assert!(stdout.contains("format: KrustBoot Manifest v1"));
    assert!(stdout.contains("generation: gen:hello-0001"));
    assert!(stdout.contains("boot_modules: 13"));
    assert!(stdout.contains("processes: 13"));
    assert!(stdout.contains("endpoints: 12"));
    assert!(stdout.contains("grants: 61"));
    assert!(stdout.contains("store_objects: 14"));
    assert!(stdout.contains("state_volumes: 0"));
    assert!(stdout.contains("network_ports: 1"));
    assert!(stdout.contains("io_ports: 3"));
    assert!(stdout.contains("mmio_regions: 0"));
    assert!(stdout.contains("interrupt_lines: 1"));
    assert!(stdout.contains("dma_regions: 1"));
    assert!(stdout.contains("pci_devices: 4"));
    assert!(stdout.contains("virtio_devices: 4"));
    assert!(stdout.contains("namespaces: 2"));

    let bytes = fs::read(&output_path).expect("read krustboot output");
    assert!(bytes.starts_with(b"KRUSTBOOTV1\0\0\0\0\0"));
    assert!(contains_bytes(&bytes, b"gen:hello-0001"));
    assert!(contains_bytes(&bytes, b"vertex-init"));
    assert!(contains_bytes(&bytes, b"serial-driver"));
    assert!(contains_bytes(&bytes, b"serial-log"));
    assert!(contains_bytes(&bytes, b"logd"));
    assert!(contains_bytes(&bytes, b"netstack"));
    assert!(contains_bytes(&bytes, b"block-driver"));
    assert!(contains_bytes(&bytes, b"vertex-store"));
    assert!(contains_bytes(&bytes, b"vertex-state"));
    assert!(contains_bytes(&bytes, b"echo"));
    assert!(contains_bytes(&bytes, b"model-reader"));
    assert!(contains_bytes(&bytes, b"counter-service"));
    assert!(contains_bytes(&bytes, b"reader-service"));
    assert!(contains_bytes(&bytes, b"timer-service"));
    assert!(contains_bytes(&bytes, b"flaky-service"));
    assert!(contains_bytes(&bytes, b"log-sink"));
    assert!(contains_bytes(&bytes, b"serial-console"));
    assert!(contains_bytes(&bytes, b"vertex-store-block-request"));
    assert!(contains_bytes(&bytes, b"vertex-state-block-request"));
    assert!(contains_bytes(&bytes, b"vertex-store-block-reply"));
    assert!(contains_bytes(&bytes, b"vertex-state-block-reply"));
    assert!(contains_bytes(&bytes, b"store-hello-text-request"));
    assert!(contains_bytes(&bytes, b"model-reader-store-reply"));
    assert!(contains_bytes(&bytes, b"state-counter-request"));
    assert!(contains_bytes(&bytes, b"state-reader-state-reply"));
    assert!(contains_bytes(&bytes, b"readiness"));
    assert!(contains_bytes(&bytes, b"cap:io.com1"));
    assert!(contains_bytes(&bytes, b"cap:io.pci-config"));
    assert!(contains_bytes(&bytes, b"cap:io.virtio-blk0"));
    assert!(contains_bytes(&bytes, b"cap:irq.virtio-blk0"));
    assert!(contains_bytes(&bytes, b"cap:dma.virtio-blk0"));
    assert!(contains_bytes(&bytes, b"device:virtio-blk0"));
    assert!(contains_bytes(&bytes, b"virtio-pci-io"));
    assert!(contains_bytes(&bytes, b"cap:net.udp.9000"));
}

#[test]
fn release_profile_validates_m65_krustboot_identity() {
    let dir = temp_dir("release-profile");
    let krustboot_path = dir.join("hello-generation.krustboot");
    let old_krustboot_path = dir.join("old-generation.krustboot");
    let kernel_path = dir.join("krust");
    let vertexdisk_path = dir.join("krust-block.img");
    fs::write(&kernel_path, b"kernel").expect("write dummy kernel");
    fs::write(&vertexdisk_path, b"vertexdisk").expect("write dummy vertexdisk");

    assert_success(run(&[
        "compile-boot-manifest",
        "examples/hello-generation.vertex.json",
        &krustboot_path.to_string_lossy(),
    ]));

    let profile = assert_success(run(&[
        "release-profile",
        "examples/hello-generation.vertex.json",
        &krustboot_path.to_string_lossy(),
        &kernel_path.to_string_lossy(),
        &vertexdisk_path.to_string_lossy(),
    ]));
    assert!(profile.contains("krustboot=Manifest v1 compact KRUSTBOOTM65 version 8"));

    assert_success(run(&[
        "corrupt-boot-manifest",
        "old-compact-magic",
        &krustboot_path.to_string_lossy(),
        &old_krustboot_path.to_string_lossy(),
    ]));

    let stderr = assert_failure(run(&[
        "release-profile",
        "examples/hello-generation.vertex.json",
        &old_krustboot_path.to_string_lossy(),
        &kernel_path.to_string_lossy(),
        &vertexdisk_path.to_string_lossy(),
    ]));
    assert!(stderr.contains("unsupported KrustBoot compact magic"));
    assert!(stderr.contains("expected KRUSTBOOTM65"));
}

#[test]
fn validate_rejects_service_config_without_config_store_object() {
    let dir = temp_dir("missing-config-store");
    let input_path = dir.join("missing-config.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let store = manifest["store"]
        .as_array_mut()
        .expect("store should be an array");
    store.retain(|object| object["id"] != "config:logd");

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad config manifest"),
    )
    .expect("write bad config manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(stderr.contains("references unknown config object config:logd"));
}

#[test]
fn compile_boot_manifest_does_not_inject_implicit_logd_config() {
    let dir = temp_dir("krustboot-no-implicit-logd-config");
    let input_path = dir.join("no-logd-config.vertex.json");
    let output_path = dir.join("no-logd-config.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let services = manifest["services"]
        .as_array_mut()
        .expect("services should be an array");
    let logd = services
        .iter_mut()
        .find(|service| service["id"] == "svc:logd")
        .expect("logd service should exist");
    logd.as_object_mut()
        .expect("service should be an object")
        .remove("configs");
    let store = manifest["store"]
        .as_array_mut()
        .expect("store should be an array");
    store.retain(|object| object["id"] != "config:logd");

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize no-config manifest"),
    )
    .expect("write no-config manifest");

    let stdout = assert_success(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stdout.contains("grants: 60"));
    assert!(stdout.contains("store_objects: 13"));
    let bytes = fs::read(&output_path).expect("read krustboot output");
    assert!(!contains_bytes(&bytes, b"config:logd"));
    assert!(!contains_bytes(&bytes, b"config-logd-v0"));
}

#[test]
fn compile_boot_manifest_rejects_missing_executable_artifact() {
    let dir = temp_dir("krustboot-missing-artifact");
    let manifest_path = dir.join("missing.vertex.json");
    let output_path = dir.join("missing.krustboot");
    write_missing_executable_manifest(&manifest_path);
    let manifest_arg = manifest_path.to_string_lossy().to_string();
    let output_arg = output_path.to_string_lossy().to_string();

    let stderr = assert_failure(run(&["compile-boot-manifest", &manifest_arg, &output_arg]));

    assert!(
        stderr.contains("native store artifact definitely-missing-executable-for-test missing")
    );
    assert!(!output_path.exists());
}

#[test]
fn create_vertex_disk_rejects_missing_executable_artifact() {
    let dir = temp_dir("vertexdisk-missing-artifact");
    let manifest_path = dir.join("missing.vertex.json");
    let output_path = dir.join("missing.img");
    write_missing_executable_manifest(&manifest_path);
    let manifest_arg = manifest_path.to_string_lossy().to_string();
    let output_arg = output_path.to_string_lossy().to_string();

    let stderr = assert_failure(run(&["create-vertex-disk", &output_arg, &manifest_arg]));

    assert!(
        stderr.contains("native store artifact definitely-missing-executable-for-test missing")
    );
    assert!(!output_path.exists());
}

#[test]
fn compile_boot_manifest_rejects_legacy_state_backend_capability() {
    let dir = temp_dir("krustboot-legacy-state");
    let input_path = dir.join("legacy-state.vertex.json");
    let output_path = dir.join("legacy-state.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let capabilities = manifest["capabilities"]
        .as_array_mut()
        .expect("capabilities should be an array");
    let clock_capability = capabilities
        .iter_mut()
        .find(|capability| capability["id"] == "cap:clock.monotonic")
        .expect("clock capability should exist");
    clock_capability["kind"] = Value::String("state-volume".to_owned());

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize legacy state manifest"),
    )
    .expect("write legacy state manifest");

    let stderr = assert_failure(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stderr.contains("legacy state backend capability"));
    assert!(stderr.contains("is not supported"));
    assert!(!output_path.exists());
}

#[test]
fn validate_rejects_driver_without_health_check() {
    let dir = temp_dir("driver-health");
    let input_path = dir.join("driver-health.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let services = manifest["services"]
        .as_array_mut()
        .expect("services should be an array");
    let block_driver = services
        .iter_mut()
        .find(|service| service["id"] == "svc:block-driver")
        .expect("block driver service should exist");
    block_driver["health"] = Value::Null;

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad driver manifest"),
    )
    .expect("write bad driver manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(
        stderr.contains(
            "device device:virtio-blk0 driver svc:block-driver must declare a health check"
        )
    );
}

#[test]
fn validate_rejects_legacy_driver_transport() {
    let dir = temp_dir("driver-legacy-transport");
    let input_path = dir.join("driver-legacy.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let devices = manifest["devices"]
        .as_array_mut()
        .expect("devices should be an array");
    let block_device = devices
        .iter_mut()
        .find(|device| device["id"] == "device:virtio-blk0")
        .expect("block device should exist");
    block_device["properties"]["transport"] = Value::String("virtio-pci-legacy".to_owned());

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize legacy transport manifest"),
    )
    .expect("write legacy transport manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(stderr.contains("device device:virtio-blk0 declares a legacy transport"));
}

#[test]
fn validate_rejects_case_insensitive_legacy_driver_transport() {
    let dir = temp_dir("driver-uppercase-legacy-transport");
    let input_path = dir.join("driver-uppercase-legacy.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let devices = manifest["devices"]
        .as_array_mut()
        .expect("devices should be an array");
    let block_device = devices
        .iter_mut()
        .find(|device| device["id"] == "device:virtio-blk0")
        .expect("block device should exist");
    block_device["properties"]["transport"] = Value::String("virtio-pci-LEGACY".to_owned());

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest)
            .expect("serialize uppercase legacy transport manifest"),
    )
    .expect("write uppercase legacy transport manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(stderr.contains("device device:virtio-blk0 declares a legacy transport"));
}

#[test]
fn validate_rejects_legacy_driver_kind_marker() {
    let dir = temp_dir("driver-legacy-kind");
    let input_path = dir.join("driver-legacy-kind.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let devices = manifest["devices"]
        .as_array_mut()
        .expect("devices should be an array");
    let block_device = devices
        .iter_mut()
        .find(|device| device["id"] == "device:virtio-blk0")
        .expect("block device should exist");
    block_device["kind"] = Value::String("virtio-blk-legacy-pci".to_owned());

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize legacy kind manifest"),
    )
    .expect("write legacy kind manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(stderr.contains("device device:virtio-blk0 declares a legacy transport"));
}

#[test]
fn validate_rejects_hardware_capability_for_non_driver() {
    let dir = temp_dir("driver-hardware-owner");
    let input_path = dir.join("driver-hardware-owner.vertex.json");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let services = manifest["services"]
        .as_array_mut()
        .expect("services should be an array");
    let echo = services
        .iter_mut()
        .find(|service| service["id"] == "svc:echo-server")
        .expect("echo service should exist");
    echo["requires"]
        .as_array_mut()
        .expect("echo requirements should be an array")
        .push(serde_json::json!({
            "capability": "cap:io.com1",
            "rights": ["read"]
        }));

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad hardware manifest"),
    )
    .expect("write bad hardware manifest");

    let stderr = assert_failure(run(&["validate", &input_path.to_string_lossy()]));

    assert!(stderr.contains("service svc:echo-server requires hardware capability cap:io.com1 owned by driver svc:serial-driver"));
}

#[test]
fn compile_boot_manifest_rejects_namespace_hardware_target() {
    let dir = temp_dir("krustboot-namespace-hardware");
    let input_path = dir.join("namespace-hardware.vertex.json");
    let output_path = dir.join("namespace-hardware.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let capabilities = manifest["capabilities"]
        .as_array_mut()
        .expect("capabilities should be an array");
    let namespace = capabilities
        .iter_mut()
        .find(|capability| capability["id"] == "cap:namespace.echo")
        .expect("echo namespace should exist");
    namespace["properties"]["entries"]
        .as_array_mut()
        .expect("namespace entries should be an array")
        .push(serde_json::json!({
            "path": "/dev/com1",
            "capability": "cap:io.com1",
            "rights": ["read"]
        }));

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad namespace manifest"),
    )
    .expect("write bad namespace manifest");

    let stderr = assert_failure(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stderr.contains("namespace entries cannot resolve hardware capability kind io-port"));
    assert!(!output_path.exists());
}

#[test]
fn compile_boot_manifest_rejects_consumer_receive_endpoint_requirement() {
    let dir = temp_dir("krustboot-receive-requirement");
    let input_path = dir.join("bad-receive.vertex.json");
    let output_path = dir.join("bad-receive.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let services = manifest["services"]
        .as_array_mut()
        .expect("services should be an array");
    let echo = services
        .iter_mut()
        .find(|service| service["id"] == "svc:echo-server")
        .expect("echo service should exist");
    let requirements = echo["requires"]
        .as_array_mut()
        .expect("echo requirements should be an array");
    let log_requirement = requirements
        .iter_mut()
        .find(|requirement| requirement["capability"] == "cap:log.sink")
        .expect("echo should require log sink");
    log_requirement["rights"] = serde_json::json!(["send", "receive"]);

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad manifest"),
    )
    .expect("write bad manifest");

    let stderr = assert_failure(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stderr.contains("requires receive authority on ipc endpoint cap:log.sink"));
    assert!(!output_path.exists());
}

#[test]
fn compile_boot_manifest_rejects_implicit_dma_base_zero() {
    let dir = temp_dir("krustboot-dma-zero");
    let input_path = dir.join("bad-dma-zero.vertex.json");
    let output_path = dir.join("bad-dma-zero.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let capabilities = manifest["capabilities"]
        .as_array_mut()
        .expect("capabilities should be an array");
    let dma = capabilities
        .iter_mut()
        .find(|capability| capability["id"] == "cap:dma.virtio-blk0")
        .expect("virtio DMA capability should exist");
    dma["properties"] = serde_json::json!({
        "base": 0,
        "length": 16384
    });

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad manifest"),
    )
    .expect("write bad manifest");

    let stderr = assert_failure(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stderr.contains("must use allocation=kernel-dma instead of base=0"));
    assert!(!output_path.exists());
}

#[test]
fn compile_boot_manifest_rejects_io_port_span_past_16_bit_space() {
    let dir = temp_dir("krustboot-io-span");
    let input_path = dir.join("bad-io-span.vertex.json");
    let output_path = dir.join("bad-io-span.krustboot");
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("examples/hello-generation.vertex.json"))
            .expect("read hello manifest"),
    )
    .expect("hello manifest should be json");

    let capabilities = manifest["capabilities"]
        .as_array_mut()
        .expect("capabilities should be an array");
    let io_port = capabilities
        .iter_mut()
        .find(|capability| capability["id"] == "cap:io.virtio-blk0")
        .expect("virtio I/O capability should exist");
    io_port["properties"] = serde_json::json!({
        "base": "0xfffe",
        "length": 4
    });

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&manifest).expect("serialize bad manifest"),
    )
    .expect("write bad manifest");

    let stderr = assert_failure(run(&[
        "compile-boot-manifest",
        &input_path.to_string_lossy(),
        &output_path.to_string_lossy(),
    ]));

    assert!(stderr.contains("range exceeds x86 I/O port space"));
    assert!(!output_path.exists());
}

#[test]
fn explain_krustboot_reports_derived_authority() {
    let stdout = assert_success(run(&[
        "explain-krustboot",
        "examples/hello-generation.vertex.json",
    ]));

    assert!(stdout.contains("svc:echo-server receives send authority to endpoint log-sink"));
    assert!(stdout.contains("because it requires cap:log.sink/send"));
    assert!(stdout.contains("and svc:logd provides cap:log.sink"));
}

#[test]
fn graph_link_resolves_package_service_closure() {
    let dir = temp_dir("graph-link");
    let output_arg = dir.to_string_lossy().to_string();

    assert_success(run(&[
        "graph-link",
        &output_arg,
        "examples/packages/serial-driver.vertexpkg",
        "examples/packages/logd.vertexpkg",
        "examples/packages/echo.vertexpkg",
    ]));

    let generation_path = dir.join("generation.vertex.json");
    let linked: Value = serde_json::from_str(
        &fs::read_to_string(&generation_path).expect("read linked generation"),
    )
    .expect("linked generation should be json");
    assert_ne!(linked["generation"]["id"], "gen:hello-0001");
    assert_eq!(
        linked["generation"]["linkedPackages"]
            .as_array()
            .expect("linked packages")
            .len(),
        3
    );

    let services = linked["services"].as_array().expect("services array");
    assert!(
        services
            .iter()
            .any(|service| service["id"] == "svc:serial-driver")
    );
    assert!(services.iter().any(|service| service["id"] == "svc:logd"));
    assert!(
        services
            .iter()
            .any(|service| service["id"] == "svc:echo-server")
    );
    assert!(
        !services
            .iter()
            .any(|service| service["id"] == "svc:counter-service")
    );

    let capabilities = linked["capabilities"]
        .as_array()
        .expect("capabilities array");
    assert!(
        capabilities
            .iter()
            .any(|capability| capability["id"] == "cap:serial.console"
                && capability["provider"] == "svc:serial-driver")
    );
    assert!(capabilities.iter().any(
        |capability| capability["id"] == "cap:log.sink" && capability["provider"] == "svc:logd"
    ));
    let logd = services
        .iter()
        .find(|service| service["id"] == "svc:logd")
        .expect("logd service should be linked");
    assert_eq!(logd["configs"], serde_json::json!(["config:logd"]));
    let store = linked["store"].as_array().expect("store array");
    assert!(
        store
            .iter()
            .any(|object| object["id"] == "config:logd" && object["kind"] == "config")
    );

    let validation = assert_success(run(&["validate", &generation_path.to_string_lossy()]));
    assert!(validation.contains("valid: gen:linked-"));

    let store_closure: Value = serde_json::from_str(
        &fs::read_to_string(dir.join("store-closure.json")).expect("read store closure"),
    )
    .expect("store closure should be json");
    assert!(
        store_closure["objects"]
            .as_array()
            .expect("store closure objects")
            .iter()
            .any(|object| object["id"] == "config:logd")
    );
}

#[test]
fn build_import_rejects_missing_kernel_and_artifact_paths() {
    let dir = temp_dir("build-import-rejects");
    let missing_kernel = dir.join("missing-kernel.json");
    fs::write(
        &missing_kernel,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "vertex.build-output.v0",
            "generationManifest": "examples/hello-generation.vertex.json",
            "kernel": null,
            "artifacts": []
        }))
        .expect("serialize missing kernel build output"),
    )
    .expect("write missing kernel build output");

    let stderr = assert_failure(run(&[
        "build-import",
        &missing_kernel.to_string_lossy(),
        "--output",
        &dir.join("out-missing-kernel").to_string_lossy(),
    ]));
    assert!(stderr.contains("missing string field kernel"));

    let fake_kernel = dir.join("krust.elf");
    fs::write(&fake_kernel, b"fake kernel\n").expect("write fake kernel");
    let missing_artifact = dir.join("missing-artifact.json");
    fs::write(
        &missing_artifact,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "vertex.build-output.v0",
            "generationManifest": "examples/hello-generation.vertex.json",
            "kernel": fake_kernel,
            "artifacts": [
                {
                    "id": "kernel:krust",
                    "kind": "kernel-image"
                }
            ]
        }))
        .expect("serialize missing artifact build output"),
    )
    .expect("write missing artifact build output");

    let stderr = assert_failure(run(&[
        "build-import",
        &missing_artifact.to_string_lossy(),
        "--output",
        &dir.join("out-missing-artifact").to_string_lossy(),
    ]));
    assert!(stderr.contains("missing string field path"));
}

#[test]
fn state_snapshot_restore_round_trip() {
    let dir = temp_dir("restore");
    let state_root = dir.join("state");
    let fake = fake_supervisor(&dir);
    let state_root_arg = state_root.to_string_lossy().to_string();

    let state_current = state_root
        .join("state-volumes")
        .join("state-echo-data")
        .join("current");
    fs::create_dir_all(&state_current).expect("create state dir");
    fs::write(state_current.join("message.txt"), "original\n").expect("write original state");

    assert_success(run_with_fake_supervisor(
        &[
            "activate",
            "examples/hello-stateful-generation.vertex.json",
            "--state-root",
            &state_root_arg,
            "--run-once",
        ],
        &fake,
    ));
    fs::write(state_current.join("message.txt"), "mutated\n").expect("mutate state");
    assert_success(run_with_fake_supervisor(
        &[
            "switch",
            "examples/hello-generation.vertex.json",
            "--state-root",
            &state_root_arg,
            "--run-once",
        ],
        &fake,
    ));
    assert_success(run_with_fake_supervisor(
        &[
            "rollback",
            "--state-root",
            &state_root_arg,
            "--run-once",
            "--restore-state",
        ],
        &fake,
    ));

    let restored = fs::read_to_string(state_current.join("message.txt")).expect("read restored");
    assert_eq!(restored, "original\n");
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(unix)]
#[test]
fn state_snapshot_rejects_symlinks() {
    let dir = temp_dir("symlink");
    let state_root = dir.join("state");
    let fake = fake_supervisor(&dir);
    let state_current = state_root
        .join("state-volumes")
        .join("state-echo-data")
        .join("current");
    fs::create_dir_all(&state_current).expect("create state dir");
    fs::write(dir.join("target.txt"), "target\n").expect("write target");
    std::os::unix::fs::symlink(dir.join("target.txt"), state_current.join("link.txt"))
        .expect("create symlink");

    let stderr = assert_failure(run_with_fake_supervisor(
        &[
            "activate",
            "examples/hello-stateful-generation.vertex.json",
            "--state-root",
            &state_root.to_string_lossy(),
            "--run-once",
        ],
        &fake,
    ));

    assert!(stderr.contains("state snapshot rejects symlink"));
}
