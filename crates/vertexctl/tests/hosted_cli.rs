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
    assert!(stdout.contains("endpoints: 10"));
    assert!(stdout.contains("grants: 43"));
    assert!(stdout.contains("store_objects: 0"));
    assert!(stdout.contains("state_volumes: 1"));
    assert!(stdout.contains("network_ports: 1"));
    assert!(stdout.contains("io_ports: 3"));
    assert!(stdout.contains("mmio_regions: 0"));
    assert!(stdout.contains("interrupt_lines: 1"));
    assert!(stdout.contains("dma_regions: 1"));

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
    assert!(contains_bytes(&bytes, b"block-read-request"));
    assert!(contains_bytes(&bytes, b"vertex-store-block-reply"));
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
    assert!(contains_bytes(&bytes, b"cap:net.tcp.8080"));
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
