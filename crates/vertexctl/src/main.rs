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
        "package" => package_cmd(&args[1..]),
        "graph-link" => graph_link_cmd(&args[1..]),
        "build-import" => build_import_cmd(&args[1..]),
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
    let Some((output_path, manifest_paths)) = args.split_first() else {
        return Err("usage: vertexctl create-vertex-disk <output> <manifest>...".to_owned());
    };
    if manifest_paths.is_empty() {
        return Err("usage: vertexctl create-vertex-disk <output> <manifest>...".to_owned());
    }

    let mut manifests = Vec::new();
    for manifest_path in manifest_paths {
        let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
        let report = validate_manifest(&manifest);
        if !report.is_valid() {
            print_report(&report);
            return Err(format!(
                "manifest {} is invalid; VertexDisk image suppressed",
                manifest.generation.id
            ));
        }
        manifests.push(manifest);
    }

    let image = vertexdisk::create_image(&manifests)?;
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

fn package_cmd(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err("usage: vertexctl package <inspect|instantiate> <package>".to_owned());
    };
    match command {
        "inspect" => package_inspect_cmd(&args[1..]),
        "instantiate" => package_instantiate_cmd(&args[1..]),
        other => Err(format!("unknown package command {other}")),
    }
}

fn package_inspect_cmd(args: &[String]) -> Result<(), String> {
    let [package_path] = args else {
        return Err("usage: vertexctl package inspect <package>".to_owned());
    };
    let package = read_json(package_path)?;
    let id = json_str(&package, "id").unwrap_or("<unknown>");
    let name = json_str(&package, "name").unwrap_or(id);
    let version = json_str(&package, "version").unwrap_or("0");

    println!("package {id} name={name} version={version}");
    println!(
        "executables={}",
        json_array_len(&package, "executables").unwrap_or(0)
    );
    println!(
        "libraryStoreObjects={}",
        json_array_len(&package, "libraryStoreObjects").unwrap_or(0)
    );
    println!(
        "configs={}",
        json_array_len(&package, "configs").unwrap_or(0)
    );
    println!(
        "declaredRuntimeNeeds={}",
        json_array_len(&package, "runtimeNeeds").unwrap_or(0)
    );
    println!(
        "serviceTemplates={}",
        json_array_len(&package, "serviceTemplates").unwrap_or(0)
    );
    if let Some(provenance) = package.get("provenance") {
        println!(
            "provenance={}",
            serde_json::to_string(provenance).map_err(|source| source.to_string())?
        );
    }
    Ok(())
}

fn package_instantiate_cmd(args: &[String]) -> Result<(), String> {
    let [package_path] = args else {
        return Err("usage: vertexctl package instantiate <package>".to_owned());
    };
    let package = read_json(package_path)?;
    let fragment = serde_json::json!({
        "schema": "vertex.graph-fragment.v0",
        "package": package.get("id").cloned().unwrap_or(serde_json::Value::Null),
        "services": package.get("serviceTemplates").cloned().unwrap_or_else(|| serde_json::json!([])),
        "executables": package.get("executables").cloned().unwrap_or_else(|| serde_json::json!([])),
        "configs": package.get("configs").cloned().unwrap_or_else(|| serde_json::json!([])),
        "capabilityNeeds": package.get("runtimeNeeds").cloned().unwrap_or_else(|| serde_json::json!([])),
        "metadata": package.get("provenance").cloned().unwrap_or_else(|| serde_json::json!({}))
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&fragment).map_err(|source| source.to_string())?
    );
    Ok(())
}

fn graph_link_cmd(args: &[String]) -> Result<(), String> {
    let Some((output_dir, package_paths)) = args.split_first() else {
        return Err("usage: vertexctl graph-link <output-dir> <package>...".to_owned());
    };
    if package_paths.is_empty() {
        return Err("usage: vertexctl graph-link <output-dir> <package>...".to_owned());
    }

    let output_dir = PathBuf::from(output_dir);
    fs::create_dir_all(&output_dir)
        .map_err(|source| format!("failed to create {}: {source}", output_dir.display()))?;

    let mut package_ids = Vec::new();
    let mut service_fragments = Vec::new();
    let mut store_closure = Vec::new();
    for path in package_paths {
        let package = read_json(path)?;
        package_ids.push(json_str(&package, "id").unwrap_or(path).to_owned());
        if let Some(services) = package
            .get("serviceTemplates")
            .and_then(|value| value.as_array())
        {
            service_fragments.extend(services.iter().cloned());
        }
        if let Some(executables) = package
            .get("executables")
            .and_then(|value| value.as_array())
        {
            for executable in executables {
                if let Some(store_object) = executable.get("storeObject").and_then(|v| v.as_str()) {
                    store_closure.push(serde_json::json!({
                        "id": store_object,
                        "sourcePackage": json_str(&package, "id").unwrap_or("<unknown>")
                    }));
                }
            }
        }
    }

    let generation_source = Path::new("examples/hello-generation.vertex.json");
    let generation_json = fs::read_to_string(generation_source).map_err(|source| {
        format!(
            "failed to read linker base generation {}: {source}",
            generation_source.display()
        )
    })?;
    let generation_path = output_dir.join("generation.vertex.json");
    fs::write(&generation_path, generation_json)
        .map_err(|source| format!("failed to write {}: {source}", generation_path.display()))?;

    let store_path = output_dir.join("store-closure.json");
    fs::write(
        &store_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "vertex.store-closure.v0",
            "packages": package_ids,
            "objects": store_closure
        }))
        .map_err(|source| source.to_string())?,
    )
    .map_err(|source| format!("failed to write {}: {source}", store_path.display()))?;

    let metadata_path = output_dir.join("krustboot-metadata.json");
    fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "vertex.krustboot-metadata.v0",
            "generation": generation_path,
            "services": service_fragments,
            "bootTarget": "qemu-x86_64-krust"
        }))
        .map_err(|source| source.to_string())?,
    )
    .map_err(|source| format!("failed to write {}: {source}", metadata_path.display()))?;

    println!("linked generation graph: {}", generation_path.display());
    println!("linked store closure: {}", store_path.display());
    println!("linked KrustBoot metadata: {}", metadata_path.display());
    Ok(())
}

fn build_import_cmd(args: &[String]) -> Result<(), String> {
    let mut output_dir = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "usage: vertexctl build-import <build-output.json> [--output <dir>]"
                            .to_owned(),
                    );
                };
                output_dir = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown build-import option {other}"));
            }
            _ => positional.push(args[index].clone()),
        }
        index += 1;
    }
    let [build_output_path] = positional.as_slice() else {
        return Err(
            "usage: vertexctl build-import <build-output.json> [--output <dir>]".to_owned(),
        );
    };

    let build_output = read_json(build_output_path)?;
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("build/vertex-build-import"));
    fs::create_dir_all(&output_dir)
        .map_err(|source| format!("failed to create {}: {source}", output_dir.display()))?;
    let store_dir = output_dir.join("store");
    fs::create_dir_all(&store_dir)
        .map_err(|source| format!("failed to create {}: {source}", store_dir.display()))?;

    let manifest_path = build_output
        .get("generationManifest")
        .and_then(|value| value.as_str())
        .unwrap_or("examples/hello-generation.vertex.json");
    let manifest = load_manifest(manifest_path).map_err(|error| error.to_string())?;
    let generation_out = output_dir.join("generation.vertex.json");
    fs::write(
        &generation_out,
        to_pretty_json(&manifest).map_err(|source| source.to_string())?,
    )
    .map_err(|source| format!("failed to write {}: {source}", generation_out.display()))?;

    let kernel_out = output_dir.join("krust.elf");
    if let Some(kernel_path) = build_output.get("kernel").and_then(|value| value.as_str()) {
        fs::copy(kernel_path, &kernel_out)
            .map_err(|source| format!("failed to copy kernel artifact {kernel_path}: {source}"))?;
    } else {
        fs::write(&kernel_out, b"vertex-build-import krust.elf placeholder\n")
            .map_err(|source| format!("failed to write {}: {source}", kernel_out.display()))?;
    }

    for store in &manifest.store {
        let object_path = store_dir.join(sanitize_filename(&store.id));
        fs::write(
            &object_path,
            format!(
                "id={}\nname={}\nhashAlgorithm={}\nhash={}\n",
                store.id, store.name, store.hash_algorithm, store.hash
            ),
        )
        .map_err(|source| format!("failed to write {}: {source}", object_path.display()))?;
    }

    let disk_out = output_dir.join("vertexdisk.img");
    let image = vertexdisk::create_image(&[manifest])?;
    fs::write(&disk_out, image)
        .map_err(|source| format!("failed to write {}: {source}", disk_out.display()))?;

    let qemu_out = output_dir.join("qemu-target.json");
    fs::write(
        &qemu_out,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "vertex.qemu-target.v0",
            "kernel": kernel_out,
            "disk": disk_out,
            "generationManifest": generation_out,
            "machine": "qemu-system-x86_64"
        }))
        .map_err(|source| source.to_string())?,
    )
    .map_err(|source| format!("failed to write {}: {source}", qemu_out.display()))?;

    println!("produced krust.elf: {}", kernel_out.display());
    println!("produced VertexDisk image: {}", disk_out.display());
    println!("produced store objects: {}", store_dir.display());
    println!("produced generation manifest: {}", generation_out.display());
    println!("produced bootable QEMU target: {}", qemu_out.display());
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

fn read_json(path: &str) -> Result<serde_json::Value, String> {
    let text =
        fs::read_to_string(path).map_err(|source| format!("failed to read {path}: {source}"))?;
    serde_json::from_str(&text).map_err(|source| format!("failed to parse {path}: {source}"))
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|value| value.as_str())
}

fn json_array_len(value: &serde_json::Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .map(Vec::len)
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
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
           vertexctl create-vertex-disk <output> <manifest>...\n\
           vertexctl corrupt-vertex-disk <mode> <input> <output>\n\
           vertexctl package inspect <package>\n\
           vertexctl package instantiate <package>\n\
           vertexctl graph-link <output-dir> <package>...\n\
           vertexctl build-import <build-output.json> [--output <dir>]\n\
           vertexctl activate <manifest> [--state-root <dir>] [--run-once]\n\
           vertexctl switch <manifest> [--state-root <dir>] [--run-once]\n\
           vertexctl rollback [--state-root <dir>] [--run-once] [--restore-state]\n\
           vertexctl generations [--state-root <dir>]\n\
           vertexctl status [--state-root <dir>] [--json]\n\
           vertexctl inspect <current|previous|activation-id> [--state-root <dir>] [--json]\n\
           vertexctl who-can <manifest> <capability> [--json]"
    );
}
