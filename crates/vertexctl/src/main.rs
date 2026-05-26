mod hosted;
mod krustboot;
mod vertexdisk;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use vertex_ir::{
    Diagnostic, GenerationManifest, ValidationReport, explain_authority, load_manifest,
    render_graph_text, to_pretty_json, validate_manifest,
};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "validate" => validate_cmd(&args[1..]),
        "graph" => graph_cmd(&args[1..]),
        "why" => why_cmd(&args[1..]),
        "materialize-demo" => materialize_demo_cmd(&args[1..]),
        "compile-boot-manifest" => compile_boot_manifest_cmd(&args[1..]),
        "corrupt-boot-manifest" => corrupt_boot_manifest_cmd(&args[1..]),
        "explain-krustboot" => explain_krustboot_cmd(&args[1..]),
        "create-vertex-disk" => create_vertex_disk_cmd(&args[1..]),
        "corrupt-vertex-disk" => corrupt_vertex_disk_cmd(&args[1..]),
        "activate" => activate_cmd(&args[1..]),
        "switch" => switch_cmd(&args[1..]),
        "rollback" => rollback_cmd(&args[1..]),
        "generations" => generations_cmd(&args[1..]),
        "status" => status_cmd(&args[1..]),
        "inspect" => inspect_cmd(&args[1..]),
        "who-can" => who_can_cmd(&args[1..]),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn validate_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path] = args else {
        return Err("usage: vertexctl validate <manifest>".to_owned());
    };
    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);

    print_report(&report);

    if report.is_valid() {
        println!("valid: {}", manifest.generation.id);
        Ok(())
    } else {
        Err(format!(
            "manifest {} has {} error(s)",
            manifest.generation.id,
            report.errors.len()
        ))
    }
}

fn graph_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path] = args else {
        return Err("usage: vertexctl graph <manifest>".to_owned());
    };
    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);

    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; graph output suppressed",
            manifest.generation.id
        ));
    }

    println!("{}", render_graph_text(&manifest));
    Ok(())
}

fn why_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path, service_id, capability_id] = args else {
        return Err("usage: vertexctl why <manifest> <service> <capability>".to_owned());
    };
    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);

    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; why output suppressed",
            manifest.generation.id
        ));
    }

    println!(
        "{}",
        explain_authority(&manifest, service_id, capability_id)
    );
    Ok(())
}

fn materialize_demo_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path, output_dir] = args else {
        return Err("usage: vertexctl materialize-demo <manifest> <output-dir>".to_owned());
    };

    let mut manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; materialization aborted",
            manifest.generation.id
        ));
    }

    let output_dir = PathBuf::from(output_dir);
    fs::create_dir_all(&output_dir).map_err(|source| {
        format!(
            "failed to create output directory {}: {source}",
            output_dir.display()
        )
    })?;
    let output_dir = output_dir.canonicalize().map_err(|source| {
        format!(
            "failed to canonicalize output directory {}: {source}",
            output_dir.display()
        )
    })?;

    materialize_demo_store(&mut manifest, &output_dir)?;

    let manifest_out = output_dir.join("hello-generation.hosted.vertex.json");
    let json = to_pretty_json(&manifest).map_err(|source| source.to_string())?;
    fs::write(&manifest_out, json)
        .map_err(|source| format!("failed to write {}: {source}", manifest_out.display()))?;

    println!("{}", manifest_out.display());
    Ok(())
}

fn compile_boot_manifest_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path, output_path] = args else {
        return Err("usage: vertexctl compile-boot-manifest <manifest> <output>".to_owned());
    };

    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; krustboot output suppressed",
            manifest.generation.id
        ));
    }

    let bytes = krustboot::compile(&manifest)?;
    fs::write(output_path, &bytes)
        .map_err(|source| format!("failed to write {output_path}: {source}"))?;
    println!(
        "{}",
        krustboot::summary(&manifest, output_path, bytes.len())
    );
    Ok(())
}

fn corrupt_boot_manifest_cmd(args: &[String]) -> Result<(), String> {
    let [mode, input_path, output_path] = args else {
        return Err("usage: vertexctl corrupt-boot-manifest <mode> <input> <output>".to_owned());
    };

    let bytes =
        fs::read(input_path).map_err(|source| format!("failed to read {input_path}: {source}"))?;
    let corrupted = krustboot::corrupt(&bytes, mode)?;
    fs::write(output_path, &corrupted)
        .map_err(|source| format!("failed to write {output_path}: {source}"))?;
    println!(
        "wrote corrupted KrustBoot manifest: mode={mode} input={input_path} output={output_path}"
    );
    Ok(())
}

fn explain_krustboot_cmd(args: &[String]) -> Result<(), String> {
    let [manifest_path] = args else {
        return Err("usage: vertexctl explain-krustboot <manifest>".to_owned());
    };

    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; krustboot explanation suppressed",
            manifest.generation.id
        ));
    }

    print!("{}", krustboot::explain(&manifest)?);
    Ok(())
}

fn create_vertex_disk_cmd(args: &[String]) -> Result<(), String> {
    let [output_path] = args else {
        return Err("usage: vertexctl create-vertex-disk <output>".to_owned());
    };

    let image = vertexdisk::create_image();
    fs::write(output_path, &image)
        .map_err(|source| format!("failed to write {output_path}: {source}"))?;
    println!(
        "wrote VertexDisk v0 image: {output_path} sectors={} sector_size={}",
        vertexdisk::sectors(),
        vertexdisk::sector_size()
    );
    Ok(())
}

fn corrupt_vertex_disk_cmd(args: &[String]) -> Result<(), String> {
    let [mode, input_path, output_path] = args else {
        return Err("usage: vertexctl corrupt-vertex-disk <mode> <input> <output>".to_owned());
    };

    let bytes =
        fs::read(input_path).map_err(|source| format!("failed to read {input_path}: {source}"))?;
    let corrupted = vertexdisk::corrupt(&bytes, mode)?;
    fs::write(output_path, &corrupted)
        .map_err(|source| format!("failed to write {output_path}: {source}"))?;
    println!(
        "wrote corrupted VertexDisk image: mode={mode} input={input_path} output={output_path}"
    );
    Ok(())
}

fn activate_cmd(args: &[String]) -> Result<(), String> {
    hosted::activate(hosted::parse_activation_args(args)?, "activate")
}

fn switch_cmd(args: &[String]) -> Result<(), String> {
    hosted::activate(hosted::parse_activation_args(args)?, "switch")
}

fn rollback_cmd(args: &[String]) -> Result<(), String> {
    hosted::rollback(hosted::parse_rollback_args(args)?)
}

fn generations_cmd(args: &[String]) -> Result<(), String> {
    hosted::generations(hosted::parse_generations_args(args)?)
}

fn status_cmd(args: &[String]) -> Result<(), String> {
    hosted::status(hosted::parse_status_args(args)?)
}

fn inspect_cmd(args: &[String]) -> Result<(), String> {
    hosted::inspect(hosted::parse_inspect_args(args)?)
}

fn who_can_cmd(args: &[String]) -> Result<(), String> {
    let mut json = false;
    let mut positional = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown who-can option {other}"));
            }
            _ => positional.push(arg.clone()),
        }
    }

    let [manifest_path, capability_id] = positional.as_slice() else {
        return Err("usage: vertexctl who-can <manifest> <capability> [--json]".to_owned());
    };

    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; who-can output suppressed",
            manifest.generation.id
        ));
    }

    let capability = manifest
        .capability(capability_id)
        .ok_or_else(|| format!("capability {capability_id} does not exist"))?;
    let granted: std::collections::BTreeSet<&str> =
        capability.rights.iter().map(String::as_str).collect();
    let entries = manifest
        .services
        .iter()
        .filter_map(|service| {
            service
                .requires
                .iter()
                .find(|requirement| requirement.capability == *capability_id)
                .map(|requirement| {
                    let fully_granted = requirement
                        .rights
                        .iter()
                        .all(|right| granted.contains(right.as_str()));
                    serde_json::json!({
                        "service": service.id,
                        "requestedRights": requirement.rights,
                        "fullyGranted": fully_granted
                    })
                })
        })
        .collect::<Vec<_>>();

    if json {
        let output = serde_json::json!({
            "capability": capability.id,
            "kind": capability.kind,
            "provider": capability.provider,
            "rights": capability.rights,
            "services": entries
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(|source| source.to_string())?
        );
    } else {
        println!(
            "{} kind={} provider={} rights=[{}]",
            capability.id,
            capability.kind,
            capability.provider,
            capability.rights.join(", ")
        );
        if entries.is_empty() {
            println!("no services require {}", capability.id);
        } else {
            for entry in entries {
                let service = entry
                    .get("service")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<unknown>");
                let rights = entry
                    .get("requestedRights")
                    .and_then(|value| value.as_array())
                    .map(|rights| {
                        rights
                            .iter()
                            .filter_map(|right| right.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let fully_granted = entry
                    .get("fullyGranted")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                println!("  {service} rights=[{rights}] fully_granted={fully_granted}");
            }
        }
    }

    Ok(())
}

fn materialize_demo_store(
    manifest: &mut GenerationManifest,
    output_dir: &Path,
) -> Result<(), String> {
    let store_root = output_dir.join("store");
    fs::create_dir_all(&store_root).map_err(|source| {
        format!(
            "failed to create store root {}: {source}",
            store_root.display()
        )
    })?;

    for store in &mut manifest.store {
        let local_path = store_root.join(&store.name);
        fs::create_dir_all(&local_path).map_err(|source| {
            format!(
                "failed to create store object {}: {source}",
                local_path.display()
            )
        })?;
        store.path = local_path.display().to_string();
        store.size_bytes = 0;
    }

    let executables = manifest.executables.clone();
    for executable in executables {
        let binary_name = demo_binary_name(&executable.id, &executable.entrypoint);
        let source = sibling_binary(&binary_name)?;
        let destination_store_path = manifest
            .store_object(&executable.store_object)
            .ok_or_else(|| {
                format!(
                    "executable {} references unknown store object {}",
                    executable.id, executable.store_object
                )
            })?
            .path
            .clone();
        let destination = PathBuf::from(destination_store_path).join(&executable.entrypoint);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                format!(
                    "failed to create destination directory {}: {source}",
                    parent.display()
                )
            })?;
        }

        fs::copy(&source, &destination).map_err(|source_error| {
            format!(
                "failed to copy {} to {}: {source_error}",
                source.display(),
                destination.display()
            )
        })?;

        let size = fs::metadata(&destination)
            .map_err(|source| format!("failed to stat {}: {source}", destination.display()))?
            .len();
        if let Some(store) = manifest
            .store
            .iter_mut()
            .find(|store| store.id == executable.store_object)
        {
            store.size_bytes = size;
        }
    }

    Ok(())
}

fn demo_binary_name(executable_id: &str, entrypoint: &str) -> String {
    match executable_id {
        "exe:vertex-init" => "vertex-supervisor".to_owned(),
        "exe:logd" => "logd".to_owned(),
        "exe:netstack" => "netstack".to_owned(),
        "exe:echo-server" => "echo-server".to_owned(),
        _ => Path::new(entrypoint)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(entrypoint)
            .to_owned(),
    }
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
            "demo binary {} does not exist; run cargo build first",
            path.display()
        ))
    }
}

fn print_report(report: &ValidationReport) {
    for diagnostic in &report.errors {
        print_diagnostic("error", diagnostic);
    }
    for diagnostic in &report.warnings {
        print_diagnostic("warning", diagnostic);
    }
}

fn print_diagnostic(label: &str, diagnostic: &Diagnostic) {
    eprintln!("{label}: {}", diagnostic.message);
}

fn print_usage() {
    println!(
        "vertexctl\n\n\
         usage:\n\
           vertexctl validate <manifest>\n\
           vertexctl graph <manifest>\n\
           vertexctl why <manifest> <service> <capability>\n\
           vertexctl materialize-demo <manifest> <output-dir>\n\
           vertexctl compile-boot-manifest <manifest> <output>\n\
           vertexctl explain-krustboot <manifest>\n\
           vertexctl create-vertex-disk <output>\n\
           vertexctl corrupt-vertex-disk <mode> <input> <output>\n\
           vertexctl activate <manifest> [--state-root <dir>] [--run-once]\n\
           vertexctl switch <manifest> [--state-root <dir>] [--run-once]\n\
           vertexctl rollback [--state-root <dir>] [--run-once] [--restore-state]\n\
           vertexctl generations [--state-root <dir>]\n\
           vertexctl status [--state-root <dir>] [--json]\n\
           vertexctl inspect <current|previous|activation-id> [--state-root <dir>] [--json]\n\
           vertexctl who-can <manifest> <capability> [--json]"
    );
}
