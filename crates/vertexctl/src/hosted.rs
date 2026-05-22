use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use vertex_ir::{GenerationManifest, StateVolume, load_manifest, validate_manifest};

#[derive(Debug, Clone)]
pub struct ActivationArgs {
    pub manifest_path: PathBuf,
    pub state_root: PathBuf,
    pub run_once: bool,
}

#[derive(Debug, Clone)]
pub struct RollbackArgs {
    pub state_root: PathBuf,
    pub run_once: bool,
    pub restore_state: bool,
}

#[derive(Debug, Clone)]
pub struct GenerationsArgs {
    pub state_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StatusArgs {
    pub state_root: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct InspectArgs {
    pub selector: String,
    pub state_root: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationPointer {
    activation_id: String,
    generation_id: String,
    manifest_path: String,
    activated_utc_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationRecord {
    activation_id: String,
    generation_id: String,
    source_manifest_path: String,
    stored_manifest_path: String,
    state_root: String,
    activated_utc_ms: u128,
    command: String,
    state_snapshots: Vec<StateSnapshotRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateSnapshotRecord {
    state_id: String,
    current_path: String,
    snapshot_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceSummary {
    id: String,
    executable: String,
    requires: usize,
    provides: usize,
    state: Vec<String>,
    health: Option<HealthSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthSummary {
    kind: String,
    target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapabilitySummary {
    id: String,
    kind: String,
    provider: String,
    rights: Vec<String>,
}

pub fn default_state_root() -> PathBuf {
    PathBuf::from(".vertex")
}

pub fn parse_activation_args(args: &[String]) -> Result<ActivationArgs, String> {
    let mut state_root = default_state_root();
    let mut run_once = false;
    let mut manifest_path = None;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            "--run-once" => run_once = true,
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            _ => {
                if manifest_path.replace(PathBuf::from(&args[idx])).is_some() {
                    return Err(
                        "usage: vertexctl activate <manifest> [--state-root <dir>] [--run-once]"
                            .to_owned(),
                    );
                }
            }
        }
        idx += 1;
    }

    Ok(ActivationArgs {
        manifest_path: manifest_path.ok_or_else(|| {
            "usage: vertexctl activate <manifest> [--state-root <dir>] [--run-once]".to_owned()
        })?,
        state_root,
        run_once,
    })
}

pub fn parse_rollback_args(args: &[String]) -> Result<RollbackArgs, String> {
    let mut state_root = default_state_root();
    let mut run_once = false;
    let mut restore_state = false;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            "--run-once" => run_once = true,
            "--restore-state" => restore_state = true,
            other => return Err(format!("unknown rollback option {other}")),
        }
        idx += 1;
    }

    Ok(RollbackArgs {
        state_root,
        run_once,
        restore_state,
    })
}

pub fn parse_generations_args(args: &[String]) -> Result<GenerationsArgs, String> {
    let mut state_root = default_state_root();
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            other => return Err(format!("unknown generations option {other}")),
        }
        idx += 1;
    }

    Ok(GenerationsArgs { state_root })
}

pub fn parse_status_args(args: &[String]) -> Result<StatusArgs, String> {
    let mut state_root = default_state_root();
    let mut json = false;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            "--json" => json = true,
            other => return Err(format!("unknown status option {other}")),
        }
        idx += 1;
    }

    Ok(StatusArgs { state_root, json })
}

pub fn parse_inspect_args(args: &[String]) -> Result<InspectArgs, String> {
    let mut state_root = default_state_root();
    let mut json = false;
    let mut selector = None;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--state-root" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    return Err("--state-root requires a directory".to_owned());
                };
                state_root = PathBuf::from(path);
            }
            "--json" => json = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown inspect option {other}"));
            }
            _ => {
                if selector.replace(args[idx].clone()).is_some() {
                    return Err(
                        "usage: vertexctl inspect <current|previous|activation-id> [--state-root <dir>] [--json]"
                            .to_owned(),
                    );
                }
            }
        }
        idx += 1;
    }

    Ok(InspectArgs {
        selector: selector.ok_or_else(|| {
            "usage: vertexctl inspect <current|previous|activation-id> [--state-root <dir>] [--json]"
                .to_owned()
        })?,
        state_root,
        json,
    })
}

pub fn activate(args: ActivationArgs, command_name: &str) -> Result<(), String> {
    let manifest = load_and_validate(&args.manifest_path)?;
    let state_root = canonicalize_state_root(&args.state_root)?;
    let now = unix_millis()?;
    let activation_id = format!("{}-{}", now, sanitize_id(&manifest.generation.id));
    let activation_dir = activation_dir(&state_root, &activation_id);
    fs::create_dir_all(&activation_dir).map_err(|source| {
        format!(
            "failed to create activation directory {}: {source}",
            activation_dir.display()
        )
    })?;

    let stored_manifest_path = activation_dir.join("manifest.vertex.json");
    write_json_file(&stored_manifest_path, &manifest)?;

    let state_snapshots = snapshot_state_volumes(&state_root, &activation_dir, &manifest)?;
    let source_manifest_path = canonicalize_existing_path(&args.manifest_path)?;
    let record = ActivationRecord {
        activation_id: activation_id.clone(),
        generation_id: manifest.generation.id.clone(),
        source_manifest_path: source_manifest_path.display().to_string(),
        stored_manifest_path: stored_manifest_path.display().to_string(),
        state_root: state_root.display().to_string(),
        activated_utc_ms: now,
        command: command_name.to_owned(),
        state_snapshots,
    };

    let activation_record_path = activation_dir.join("activation.json");
    write_json_file(&activation_record_path, &record)?;

    run_supervisor(&stored_manifest_path, &state_root, args.run_once)?;
    commit_activation(&state_root, &record)?;

    println!(
        "{} {} as {}",
        command_name, record.generation_id, record.activation_id
    );
    Ok(())
}

pub fn rollback(args: RollbackArgs) -> Result<(), String> {
    let state_root = canonicalize_state_root(&args.state_root)?;
    let previous = read_pointer(&previous_path(&state_root))?.ok_or_else(|| {
        format!(
            "no previous generation recorded under {}",
            state_root.display()
        )
    })?;

    let previous_activation_dir = activation_dir(&state_root, &previous.activation_id);
    let previous_manifest_path = previous_activation_dir.join("manifest.vertex.json");
    let manifest = load_and_validate(&previous_manifest_path)?;

    if args.restore_state {
        restore_snapshots(&state_root, &previous_activation_dir, &manifest)?;
    }

    run_supervisor(&previous_manifest_path, &state_root, args.run_once)?;

    let current = read_pointer(&current_path(&state_root))?;
    write_pointer(&current_path(&state_root), &previous)?;
    if let Some(current) = current {
        write_pointer(&previous_path(&state_root), &current)?;
    }
    append_history_event(
        &state_root,
        serde_json::json!({
            "event": "rollback",
            "activationId": previous.activation_id,
            "generationId": previous.generation_id,
            "restoreState": args.restore_state,
            "utcMs": unix_millis()?
        }),
    )?;

    println!(
        "rollback {} as {}",
        previous.generation_id, previous.activation_id
    );
    Ok(())
}

pub fn generations(args: GenerationsArgs) -> Result<(), String> {
    let state_root = canonicalize_state_root(&args.state_root)?;
    let current = read_pointer(&current_path(&state_root))?;
    let previous = read_pointer(&previous_path(&state_root))?;
    let activations = state_root.join("activations");

    println!("state root: {}", state_root.display());
    print_pointer("current", current.as_ref());
    print_pointer("previous", previous.as_ref());

    if !activations.exists() {
        println!("activations: none");
        return Ok(());
    }

    println!("activations:");
    let mut entries = fs::read_dir(&activations)
        .map_err(|source| format!("failed to read {}: {source}", activations.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| format!("failed to read {}: {source}", activations.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if !entry
            .file_type()
            .map_err(|source| format!("failed to inspect {}: {source}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }
        let activation_path = entry.path().join("activation.json");
        match read_activation_record(&activation_path) {
            Ok(Some(record)) => println!(
                "  {} {} {}",
                record.activation_id, record.generation_id, record.command
            ),
            Ok(None) => println!(
                "  {} <missing activation.json>",
                entry.file_name().to_string_lossy()
            ),
            Err(error) => println!(
                "  {} <invalid: {}>",
                entry.file_name().to_string_lossy(),
                error
            ),
        }
    }

    Ok(())
}

pub fn status(args: StatusArgs) -> Result<(), String> {
    let state_root = canonicalize_state_root(&args.state_root)?;
    let current = read_pointer(&current_path(&state_root))?;
    let previous = read_pointer(&previous_path(&state_root))?;
    let activation_count = activation_count(&state_root)?;
    let last_event = match read_last_jsonl_event(&runtime_events_path(&state_root))? {
        Some(event) => Some(event),
        None => read_last_jsonl_event(&history_path(&state_root))?,
    };

    if args.json {
        print_json(&serde_json::json!({
            "stateRoot": state_root,
            "current": current,
            "previous": previous,
            "activationCount": activation_count,
            "lastEvent": last_event
        }))?;
    } else {
        println!("state root: {}", state_root.display());
        print_pointer("current", current.as_ref());
        print_pointer("previous", previous.as_ref());
        println!("activation count: {activation_count}");
        match last_event {
            Some(event) => println!("last event: {event}"),
            None => println!("last event: none"),
        }
    }

    Ok(())
}

pub fn inspect(args: InspectArgs) -> Result<(), String> {
    let state_root = canonicalize_state_root(&args.state_root)?;
    let pointer = resolve_activation_selector(&state_root, &args.selector)?;
    let activation_dir = activation_dir(&state_root, &pointer.activation_id);
    let record_path = activation_dir.join("activation.json");
    let record = read_activation_record(&record_path)?.ok_or_else(|| {
        format!(
            "activation {} is missing {}",
            pointer.activation_id,
            record_path.display()
        )
    })?;
    let manifest =
        load_manifest(&record.stored_manifest_path).map_err(|error| error.to_string())?;
    let services = service_summaries(&manifest);
    let capabilities = capability_summaries(&manifest);

    if args.json {
        print_json(&serde_json::json!({
            "activation": record,
            "services": services,
            "capabilities": capabilities
        }))?;
    } else {
        println!("activation: {}", record.activation_id);
        println!("generation: {}", record.generation_id);
        println!("command: {}", record.command);
        println!("source manifest: {}", record.source_manifest_path);
        println!("stored manifest: {}", record.stored_manifest_path);
        println!("state root: {}", record.state_root);
        println!("state snapshots:");
        if record.state_snapshots.is_empty() {
            println!("  none");
        } else {
            for snapshot in &record.state_snapshots {
                println!(
                    "  {} {} -> {}",
                    snapshot.state_id, snapshot.current_path, snapshot.snapshot_path
                );
            }
        }
        println!("services:");
        for service in &services {
            let health = service
                .health
                .as_ref()
                .map(|health| match &health.target {
                    Some(target) => format!(" health={}:{}", health.kind, target),
                    None => format!(" health={}", health.kind),
                })
                .unwrap_or_default();
            println!(
                "  {} executable={} requires={} provides={} state=[{}]{}",
                service.id,
                service.executable,
                service.requires,
                service.provides,
                service.state.join(", "),
                health
            );
        }
        println!("capabilities:");
        for capability in &capabilities {
            println!(
                "  {} kind={} provider={} rights=[{}]",
                capability.id,
                capability.kind,
                capability.provider,
                capability.rights.join(", ")
            );
        }
    }

    Ok(())
}

fn load_and_validate(path: &Path) -> Result<GenerationManifest, String> {
    let manifest = load_manifest(path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);
    for diagnostic in &report.errors {
        eprintln!("error: {}", diagnostic.message);
    }
    for diagnostic in &report.warnings {
        eprintln!("warning: {}", diagnostic.message);
    }
    if report.is_valid() {
        Ok(manifest)
    } else {
        Err(format!(
            "manifest {} has {} error(s)",
            manifest.generation.id,
            report.errors.len()
        ))
    }
}

fn snapshot_state_volumes(
    state_root: &Path,
    activation_dir: &Path,
    manifest: &GenerationManifest,
) -> Result<Vec<StateSnapshotRecord>, String> {
    let mut snapshots = Vec::new();
    for state in &manifest.state_volumes {
        let current = state_current_path(state_root, state);
        fs::create_dir_all(&current).map_err(|source| {
            format!(
                "failed to create state volume {}: {source}",
                current.display()
            )
        })?;

        let snapshot = activation_dir
            .join("snapshots")
            .join(sanitize_id(&state.id));
        copy_dir_rejecting_symlinks(&current, &snapshot)?;
        snapshots.push(StateSnapshotRecord {
            state_id: state.id.clone(),
            current_path: current.display().to_string(),
            snapshot_path: snapshot.display().to_string(),
        });
    }
    Ok(snapshots)
}

fn restore_snapshots(
    state_root: &Path,
    activation_dir: &Path,
    manifest: &GenerationManifest,
) -> Result<(), String> {
    for state in &manifest.state_volumes {
        let volume_root = state_volume_root(state_root, state);
        let current = volume_root.join("current");
        let snapshot = activation_dir
            .join("snapshots")
            .join(sanitize_id(&state.id));
        if !snapshot.exists() {
            return Err(format!(
                "snapshot for {} does not exist at {}",
                state.id,
                snapshot.display()
            ));
        }

        fs::create_dir_all(&volume_root).map_err(|source| {
            format!(
                "failed to create state volume root {}: {source}",
                volume_root.display()
            )
        })?;
        let restore_tmp = volume_root.join(format!("restore-{}.tmp", unix_millis()?));
        let backup = volume_root.join(format!("previous-current-{}.bak", unix_millis()?));
        if restore_tmp.exists() {
            fs::remove_dir_all(&restore_tmp).map_err(|source| {
                format!(
                    "failed to remove stale restore temp {}: {source}",
                    restore_tmp.display()
                )
            })?;
        }
        copy_dir_rejecting_symlinks(&snapshot, &restore_tmp)?;
        if current.exists() {
            fs::rename(&current, &backup).map_err(|source| {
                format!(
                    "failed to move current state {} to {}: {source}",
                    current.display(),
                    backup.display()
                )
            })?;
        }
        if let Err(source) = fs::rename(&restore_tmp, &current) {
            if backup.exists() && !current.exists() {
                let _ = fs::rename(&backup, &current);
            }
            return Err(format!(
                "failed to install restored state {}: {source}",
                current.display()
            ));
        }
        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|source| {
                format!(
                    "failed to remove restored state backup {}: {source}",
                    backup.display()
                )
            })?;
        }
    }
    Ok(())
}

fn commit_activation(state_root: &Path, record: &ActivationRecord) -> Result<(), String> {
    let pointer = ActivationPointer {
        activation_id: record.activation_id.clone(),
        generation_id: record.generation_id.clone(),
        manifest_path: record.stored_manifest_path.clone(),
        activated_utc_ms: record.activated_utc_ms,
    };
    let current = read_pointer(&current_path(state_root))?;
    write_pointer(&current_path(state_root), &pointer)?;
    if let Some(current) = current {
        write_pointer(&previous_path(state_root), &current)?;
    }
    append_history_event(
        state_root,
        serde_json::json!({
            "event": record.command,
            "activationId": record.activation_id,
            "generationId": record.generation_id,
            "utcMs": record.activated_utc_ms
        }),
    )
}

fn run_supervisor(manifest_path: &Path, state_root: &Path, run_once: bool) -> Result<(), String> {
    let supervisor = match env::var_os("VERTEX_SUPERVISOR_BIN") {
        Some(path) => PathBuf::from(path),
        None => sibling_binary("vertex-supervisor")?,
    };
    let mut command = Command::new(&supervisor);
    if run_once {
        command.arg("--run-once");
    }
    command.arg("--state-root");
    command.arg(state_root);
    command.arg(manifest_path);

    let status = command.status().map_err(|source| {
        format!(
            "failed to run supervisor {}: {source}",
            supervisor.display()
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("supervisor exited with {status}"))
    }
}

fn copy_dir_rejecting_symlinks(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|source_error| {
        format!("failed to inspect {}: {source_error}", source.display())
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "state snapshot rejects symlink {}",
            source.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "state snapshot source is not a directory: {}",
            source.display()
        ));
    }

    fs::create_dir_all(destination).map_err(|source_error| {
        format!(
            "failed to create snapshot directory {}: {source_error}",
            destination.display()
        )
    })?;
    for entry in fs::read_dir(source)
        .map_err(|source_error| format!("failed to read {}: {source_error}", source.display()))?
    {
        let entry = entry.map_err(|source_error| {
            format!("failed to read {}: {source_error}", source.display())
        })?;
        let entry_path = entry.path();
        let entry_metadata = fs::symlink_metadata(&entry_path).map_err(|source_error| {
            format!("failed to inspect {}: {source_error}", entry_path.display())
        })?;
        if entry_metadata.file_type().is_symlink() {
            return Err(format!(
                "state snapshot rejects symlink {}",
                entry_path.display()
            ));
        }

        let destination_path = destination.join(entry.file_name());
        if entry_metadata.is_dir() {
            copy_dir_rejecting_symlinks(&entry_path, &destination_path)?;
        } else if entry_metadata.is_file() {
            fs::copy(&entry_path, &destination_path).map_err(|source_error| {
                format!(
                    "failed to copy {} to {}: {source_error}",
                    entry_path.display(),
                    destination_path.display()
                )
            })?;
        } else {
            return Err(format!(
                "state snapshot supports only regular files and directories, found {}",
                entry_path.display()
            ));
        }
    }
    Ok(())
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| format!("failed to create {}: {source}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|source| source.to_string())?;
    fs::write(path, json).map_err(|source| format!("failed to write {}: {source}", path.display()))
}

fn write_pointer(path: &Path, pointer: &ActivationPointer) -> Result<(), String> {
    write_json_file(path, pointer)
}

fn read_pointer(path: &Path) -> Result<Option<ActivationPointer>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let input = fs::read_to_string(path)
        .map_err(|source| format!("failed to read {}: {source}", path.display()))?;
    serde_json::from_str(&input)
        .map(Some)
        .map_err(|source| format!("failed to parse {}: {source}", path.display()))
}

fn read_activation_record(path: &Path) -> Result<Option<ActivationRecord>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let input = fs::read_to_string(path)
        .map_err(|source| format!("failed to read {}: {source}", path.display()))?;
    serde_json::from_str(&input)
        .map(Some)
        .map_err(|source| format!("failed to parse {}: {source}", path.display()))
}

fn append_history_event(state_root: &Path, value: serde_json::Value) -> Result<(), String> {
    use std::io::Write;

    fs::create_dir_all(state_root)
        .map_err(|source| format!("failed to create {}: {source}", state_root.display()))?;
    let history = state_root.join("history.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history)
        .map_err(|source| format!("failed to open {}: {source}", history.display()))?;
    let line = serde_json::to_string(&value).map_err(|source| source.to_string())?;
    writeln!(file, "{line}")
        .map_err(|source| format!("failed to append {}: {source}", history.display()))
}

fn activation_count(state_root: &Path) -> Result<usize, String> {
    let activations = state_root.join("activations");
    if !activations.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(&activations)
        .map_err(|source| format!("failed to read {}: {source}", activations.display()))?
    {
        let entry = entry
            .map_err(|source| format!("failed to read {}: {source}", activations.display()))?;
        if entry
            .file_type()
            .map_err(|source| format!("failed to inspect {}: {source}", entry.path().display()))?
            .is_dir()
        {
            count += 1;
        }
    }
    Ok(count)
}

fn resolve_activation_selector(
    state_root: &Path,
    selector: &str,
) -> Result<ActivationPointer, String> {
    match selector {
        "current" => read_pointer(&current_path(state_root))?.ok_or_else(|| {
            format!(
                "no current generation recorded under {}",
                state_root.display()
            )
        }),
        "previous" => read_pointer(&previous_path(state_root))?.ok_or_else(|| {
            format!(
                "no previous generation recorded under {}",
                state_root.display()
            )
        }),
        activation_id => {
            let activation_path = activation_dir(state_root, activation_id).join("activation.json");
            let record = read_activation_record(&activation_path)?.ok_or_else(|| {
                format!(
                    "activation {} does not exist under {}",
                    activation_id,
                    state_root.display()
                )
            })?;
            Ok(ActivationPointer {
                activation_id: record.activation_id,
                generation_id: record.generation_id,
                manifest_path: record.stored_manifest_path,
                activated_utc_ms: record.activated_utc_ms,
            })
        }
    }
}

fn service_summaries(manifest: &GenerationManifest) -> Vec<ServiceSummary> {
    manifest
        .services
        .iter()
        .map(|service| ServiceSummary {
            id: service.id.clone(),
            executable: service.executable.clone(),
            requires: service.requires.len(),
            provides: service.provides.len(),
            state: service.state.clone(),
            health: service.health.as_ref().map(|health| HealthSummary {
                kind: health.kind.clone(),
                target: health.target.clone(),
            }),
        })
        .collect()
}

fn capability_summaries(manifest: &GenerationManifest) -> Vec<CapabilitySummary> {
    manifest
        .capabilities
        .iter()
        .map(|capability| CapabilitySummary {
            id: capability.id.clone(),
            kind: capability.kind.clone(),
            provider: capability.provider.clone(),
            rights: capability.rights.clone(),
        })
        .collect()
}

fn read_last_jsonl_event(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let input = fs::read_to_string(path)
        .map_err(|source| format!("failed to read {}: {source}", path.display()))?;
    let Some(line) = input.lines().rev().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    serde_json::from_str(line)
        .map(Some)
        .map_err(|source| format!("failed to parse last event in {}: {source}", path.display()))
}

fn print_json(value: &serde_json::Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|source| source.to_string())?;
    println!("{json}");
    Ok(())
}

fn state_current_path(state_root: &Path, state: &StateVolume) -> PathBuf {
    state_volume_root(state_root, state).join("current")
}

fn state_volume_root(state_root: &Path, state: &StateVolume) -> PathBuf {
    state_root
        .join("state-volumes")
        .join(sanitize_id(&state.id))
}

fn activation_dir(state_root: &Path, activation_id: &str) -> PathBuf {
    state_root.join("activations").join(activation_id)
}

fn current_path(state_root: &Path) -> PathBuf {
    state_root.join("current.json")
}

fn previous_path(state_root: &Path) -> PathBuf {
    state_root.join("previous.json")
}

fn history_path(state_root: &Path) -> PathBuf {
    state_root.join("history.jsonl")
}

fn runtime_events_path(state_root: &Path) -> PathBuf {
    state_root.join("runtime-events.jsonl")
}

fn canonicalize_state_root(path: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(path)
        .map_err(|source| format!("failed to create state root {}: {source}", path.display()))?;
    path.canonicalize().map_err(|source| {
        format!(
            "failed to canonicalize state root {}: {source}",
            path.display()
        )
    })
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|source| format!("failed to canonicalize {}: {source}", path.display()))
}

fn sibling_binary(name: &str) -> Result<PathBuf, String> {
    let current_exe = env::current_exe()
        .map_err(|source| format!("failed to locate current executable: {source}"))?;
    let Some(dir) = current_exe.parent() else {
        return Err(format!(
            "current executable path {} has no parent",
            current_exe.display()
        ));
    };

    let path = dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "sibling binary {} does not exist; run cargo build first",
            path.display()
        ))
    }
}

fn print_pointer(label: &str, pointer: Option<&ActivationPointer>) {
    match pointer {
        Some(pointer) => println!(
            "{label}: {} {} {}",
            pointer.activation_id, pointer.generation_id, pointer.manifest_path
        ),
        None => println!("{label}: none"),
    }
}

fn unix_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|source| format!("host clock moved backwards: {source}"))
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
