mod hosted;
mod krustboot;
mod vertexdisk;

use std::collections::{BTreeMap, BTreeSet};
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
    let mut package_services = Vec::new();
    let mut package_executables = Vec::new();
    let mut package_configs = Vec::new();
    let mut store_closure = Vec::new();
    for path in package_paths {
        let package = read_json(path)?;
        package_ids.push(json_str(&package, "id").unwrap_or(path).to_owned());
        if let Some(services) = package
            .get("serviceTemplates")
            .and_then(|value| value.as_array())
        {
            package_services.extend(services.iter().cloned());
        }
        if let Some(executables) = package
            .get("executables")
            .and_then(|value| value.as_array())
        {
            for executable in executables {
                package_executables.push(executable.clone());
                if let Some(store_object) = executable.get("storeObject").and_then(|v| v.as_str()) {
                    store_closure.push(serde_json::json!({
                        "id": store_object,
                        "sourcePackage": json_str(&package, "id").unwrap_or("<unknown>")
                    }));
                }
            }
        }
        if let Some(configs) = package.get("configs").and_then(|value| value.as_array()) {
            package_configs.extend(configs.iter().cloned());
        }
    }

    let generation = link_package_generation(
        &package_ids,
        &package_services,
        &package_executables,
        &package_configs,
    )?;
    let generation_path = output_dir.join("generation.vertex.json");
    fs::write(
        &generation_path,
        serde_json::to_string_pretty(&generation).map_err(|source| source.to_string())?,
    )
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
            "services": package_services,
            "configs": package_configs,
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
    let build_output_base = Path::new(build_output_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
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
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "manifest {} is invalid; build import aborted",
            manifest.generation.id
        ));
    }
    let generation_out = output_dir.join("generation.vertex.json");
    fs::write(
        &generation_out,
        to_pretty_json(&manifest).map_err(|source| source.to_string())?,
    )
    .map_err(|source| format!("failed to write {}: {source}", generation_out.display()))?;

    let kernel_path = required_json_path(&build_output, "kernel", &build_output_base)?;
    let kernel_out = output_dir.join("krust.elf");
    copy_file(&kernel_path, &kernel_out, "kernel artifact")?;

    let mut imported_artifacts = Vec::new();
    let artifacts = build_output
        .get("artifacts")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "build-output artifacts must be an array".to_owned())?;
    for artifact in artifacts {
        let id = json_required_str(artifact, "id")?;
        let kind = json_required_str(artifact, "kind")?;
        let artifact_path = required_json_path(artifact, "path", &build_output_base)?;
        verify_declared_hash(artifact, &artifact_path)?;
        let bytes = fs::read(&artifact_path).map_err(|source| {
            format!(
                "failed to read artifact {}: {source}",
                artifact_path.display()
            )
        })?;
        let object_path = store_dir.join(sanitize_filename(id));
        fs::write(&object_path, &bytes)
            .map_err(|source| format!("failed to write {}: {source}", object_path.display()))?;
        imported_artifacts.push(serde_json::json!({
            "id": id,
            "kind": kind,
            "source": artifact_path,
            "imported": object_path,
            "sizeBytes": bytes.len(),
            "hashAlgorithm": "blake3",
            "hash": blake3_hex(&bytes)
        }));
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
            "artifacts": imported_artifacts,
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

fn link_package_generation(
    package_ids: &[String],
    package_services: &[serde_json::Value],
    package_executables: &[serde_json::Value],
    package_configs: &[serde_json::Value],
) -> Result<serde_json::Value, String> {
    let base_path = Path::new("examples/hello-generation.vertex.json");
    let mut generation = read_json(
        base_path
            .to_str()
            .unwrap_or("examples/hello-generation.vertex.json"),
    )?;
    let base = generation.clone();
    let root_service_id = base
        .get("activation")
        .and_then(|value| value.get("rootService"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "linker base generation missing activation.rootService".to_owned())?;

    let package_service_ids = package_services
        .iter()
        .map(|service| json_required_str(service, "id").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    if package_service_ids.is_empty() {
        return Err("graph-link packages declare no serviceTemplates".to_owned());
    }

    let base_services = index_json_array(&base, "services", "id")?;
    let base_executables = index_json_array(&base, "executables", "id")?;
    let base_store = index_json_array(&base, "store", "id")?;
    let base_capabilities = index_json_array(&base, "capabilities", "id")?;
    let base_devices = index_json_array(&base, "devices", "id")?;
    let package_executable_map = index_values(package_executables, "id")?;

    let mut selected_service_ids = BTreeSet::new();
    selected_service_ids.insert(root_service_id.to_owned());
    selected_service_ids.extend(package_service_ids.iter().cloned());

    let mut services = Vec::new();
    for service_id in &selected_service_ids {
        if service_id == root_service_id {
            services.push(
                base_services
                    .get(service_id.as_str())
                    .ok_or_else(|| format!("linker base missing root service {service_id}"))?
                    .clone(),
            );
            continue;
        }

        if let Some(template) = package_services
            .iter()
            .find(|service| service.get("id").and_then(|value| value.as_str()) == Some(service_id))
        {
            services.push(enrich_service_template(template)?);
        } else if let Some(base_service) = base_services.get(service_id.as_str()) {
            services.push(base_service.clone());
        } else {
            return Err(format!("package service template {service_id} disappeared"));
        }
    }

    let mut executable_ids = BTreeSet::new();
    for service in &services {
        executable_ids.insert(json_required_str(service, "executable")?.to_owned());
    }

    let mut executables = Vec::new();
    for executable_id in &executable_ids {
        if let Some(executable) = base_executables.get(executable_id.as_str()) {
            executables.push(executable.clone());
        } else if let Some(executable) = package_executable_map.get(executable_id.as_str()) {
            executables.push((*executable).clone());
        } else {
            return Err(format!(
                "linked service references unknown executable {executable_id}"
            ));
        }
    }

    let mut capability_ids = BTreeSet::new();
    for service in &services {
        collect_string_array(service, "provides", &mut capability_ids);
        if let Some(requires) = service.get("requires").and_then(|value| value.as_array()) {
            for requirement in requires {
                capability_ids.insert(json_required_str(requirement, "capability")?.to_owned());
            }
        }
    }

    let provider_by_capability = package_capability_providers(package_services)?;
    let mut capabilities = Vec::new();
    for capability_id in &capability_ids {
        if let Some(capability) = base_capabilities.get(capability_id.as_str()) {
            let provider = json_required_str(capability, "provider")?;
            if provider.starts_with("svc:") && !selected_service_ids.contains(provider) {
                return Err(format!(
                    "capability {capability_id} is provided by {provider}, which is not in the linked package closure"
                ));
            }
            capabilities.push(capability.clone());
            continue;
        }

        let provider = provider_by_capability.get(capability_id.as_str()).ok_or_else(|| {
            format!("capability {capability_id} is required but no package or platform provider exists")
        })?;
        capabilities.push(serde_json::json!({
            "id": capability_id,
            "kind": "ipc-endpoint",
            "provider": provider,
            "rights": ["send"],
            "properties": {
                "protocol": "vertex.package.v0",
                "role": "request"
            }
        }));
    }

    let mut store_ids = BTreeSet::new();
    if let Some(kernel_store) = generation
        .get("kernel")
        .and_then(|value| value.get("storeObject"))
        .and_then(|value| value.as_str())
    {
        store_ids.insert(kernel_store.to_owned());
    }
    for executable in &executables {
        store_ids.insert(json_required_str(executable, "storeObject")?.to_owned());
    }
    let mut store = Vec::new();
    for store_id in &store_ids {
        if let Some(object) = base_store.get(store_id.as_str()) {
            store.push(object.clone());
        } else {
            store.push(linked_store_object(store_id, package_ids));
        }
    }

    let mut device_ids = BTreeSet::new();
    for capability in &capabilities {
        let provider = json_required_str(capability, "provider")?;
        if base_devices.contains_key(provider) {
            device_ids.insert(provider.to_owned());
        }
    }
    let devices = device_ids
        .iter()
        .filter_map(|id| base_devices.get(id.as_str()).cloned())
        .collect::<Vec<_>>();

    let start_order = linked_start_order(root_service_id, &services, &capabilities)?;
    generation["generation"]["id"] =
        serde_json::Value::String(linked_generation_id(package_ids, &package_service_ids));
    generation["generation"]["description"] = serde_json::Value::String(format!(
        "Linked package generation for {}",
        package_ids.join(", ")
    ));
    generation["generation"]["parent"] = serde_json::Value::Null;
    generation["generation"]["manifestHash"] = serde_json::Value::Null;
    generation["generation"]["linkedPackages"] = serde_json::json!(package_ids);
    generation["generation"]["linkedServices"] = serde_json::json!(package_service_ids);
    generation["store"] = serde_json::Value::Array(store);
    generation["executables"] = serde_json::Value::Array(executables);
    generation["devices"] = serde_json::Value::Array(devices);
    generation["stateVolumes"] = serde_json::json!([]);
    generation["secrets"] = serde_json::json!([]);
    generation["capabilities"] = serde_json::Value::Array(capabilities);
    generation["services"] = serde_json::Value::Array(services);
    generation["activation"]["startOrder"] = serde_json::json!(start_order);
    generation["activation"]["onFailure"] = serde_json::Value::String("stop-activation".to_owned());
    generation["activation"]["rollbackPolicy"] = serde_json::json!({
        "default": "system-only",
        "state": "preserve-unless-explicit"
    });
    if !package_configs.is_empty() {
        generation["generation"]["linkedConfigs"] = serde_json::json!(package_configs);
    }

    let manifest: GenerationManifest =
        serde_json::from_value(generation.clone()).map_err(|source| source.to_string())?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        print_report(&report);
        return Err(format!(
            "linked package generation {} is invalid",
            manifest.generation.id
        ));
    }

    Ok(generation)
}

fn enrich_service_template(template: &serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "id": json_required_str(template, "id")?,
        "name": json_required_str(template, "name")?,
        "executable": json_required_str(template, "executable")?,
        "args": [],
        "env": {},
        "requires": template.get("requires").cloned().unwrap_or_else(|| serde_json::json!([])),
        "provides": template.get("provides").cloned().unwrap_or_else(|| serde_json::json!([])),
        "state": template.get("state").cloned().unwrap_or_else(|| serde_json::json!([])),
        "secrets": template.get("secrets").cloned().unwrap_or_else(|| serde_json::json!([])),
        "restart": template.get("restart").and_then(|value| value.as_str()).unwrap_or("never"),
        "resources": template.get("resources").cloned().unwrap_or_else(|| serde_json::json!({
            "memoryMaxBytes": 33554432,
            "cpuShares": 25
        })),
        "health": template.get("health").cloned().unwrap_or(serde_json::Value::Null),
        "lifecycle": template.get("lifecycle").cloned().unwrap_or_else(|| serde_json::json!({
            "startAfter": [],
            "stopBefore": []
        }))
    }))
}

fn linked_store_object(store_id: &str, package_ids: &[String]) -> serde_json::Value {
    let name = sanitize_filename(store_id.trim_start_matches("store:"));
    let hash = blake3_hex(format!("{}:{store_id}", package_ids.join(",")).as_bytes());
    serde_json::json!({
        "id": store_id,
        "name": name,
        "kind": "executable",
        "path": format!("/vertex/store/{name}"),
        "hashAlgorithm": "blake3",
        "hash": hash,
        "sizeBytes": 0,
        "references": []
    })
}

fn linked_generation_id(package_ids: &[String], service_ids: &[String]) -> String {
    let mut bytes = Vec::new();
    for id in package_ids {
        bytes.extend_from_slice(id.as_bytes());
        bytes.push(0);
    }
    for id in service_ids {
        bytes.extend_from_slice(id.as_bytes());
        bytes.push(0xff);
    }
    format!("gen:linked-{}", &blake3_hex(&bytes)[..12])
}

fn linked_start_order(
    root_service_id: &str,
    services: &[serde_json::Value],
    capabilities: &[serde_json::Value],
) -> Result<Vec<String>, String> {
    let service_ids = services
        .iter()
        .map(|service| json_required_str(service, "id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let provider_by_capability = capabilities
        .iter()
        .map(|capability| {
            Ok((
                json_required_str(capability, "id")?.to_owned(),
                json_required_str(capability, "provider")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;

    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for service in services {
        let service_id = json_required_str(service, "id")?.to_owned();
        let entry = deps.entry(service_id.clone()).or_default();
        if let Some(lifecycle) = service.get("lifecycle")
            && let Some(start_after) = lifecycle
                .get("startAfter")
                .and_then(|value| value.as_array())
        {
            for dependency in start_after.iter().filter_map(|value| value.as_str()) {
                if service_ids.contains(dependency) {
                    entry.insert(dependency.to_owned());
                }
            }
        }
        if let Some(requires) = service.get("requires").and_then(|value| value.as_array()) {
            for requirement in requires {
                let capability_id = json_required_str(requirement, "capability")?;
                if let Some(provider) = provider_by_capability.get(capability_id)
                    && service_ids.contains(provider)
                    && provider != &service_id
                {
                    entry.insert(provider.clone());
                }
            }
        }
    }

    let mut order = Vec::new();
    let mut emitted = BTreeSet::new();
    if service_ids.contains(root_service_id) {
        order.push(root_service_id.to_owned());
        emitted.insert(root_service_id.to_owned());
    }
    while emitted.len() < service_ids.len() {
        let mut made_progress = false;
        for service_id in &service_ids {
            if emitted.contains(service_id) {
                continue;
            }
            let ready = deps
                .get(service_id)
                .is_none_or(|dependencies| dependencies.iter().all(|id| emitted.contains(id)));
            if ready {
                order.push(service_id.clone());
                emitted.insert(service_id.clone());
                made_progress = true;
            }
        }
        if !made_progress {
            return Err(format!(
                "linked package services contain a dependency cycle: {deps:?}"
            ));
        }
    }

    Ok(order)
}

fn package_capability_providers(
    package_services: &[serde_json::Value],
) -> Result<BTreeMap<String, String>, String> {
    let mut providers = BTreeMap::new();
    for service in package_services {
        let service_id = json_required_str(service, "id")?;
        if let Some(provides) = service.get("provides").and_then(|value| value.as_array()) {
            for capability in provides.iter().filter_map(|value| value.as_str()) {
                if let Some(previous) =
                    providers.insert(capability.to_owned(), service_id.to_owned())
                {
                    return Err(format!(
                        "capability {capability} is provided by both {previous} and {service_id}"
                    ));
                }
            }
        }
    }
    Ok(providers)
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

fn json_required_str<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    json_str(value, key).ok_or_else(|| format!("json object missing string field {key}"))
}

fn json_array_len(value: &serde_json::Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(|value| value.as_array())
        .map(Vec::len)
}

fn index_json_array(
    value: &serde_json::Value,
    array_key: &str,
    id_key: &str,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let values = value
        .get(array_key)
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("json object missing array field {array_key}"))?;
    index_values(values, id_key).map(|map| {
        map.into_iter()
            .map(|(key, value)| (key, value.clone()))
            .collect()
    })
}

fn index_values<'a>(
    values: &'a [serde_json::Value],
    id_key: &str,
) -> Result<BTreeMap<String, &'a serde_json::Value>, String> {
    let mut out = BTreeMap::new();
    for value in values {
        let id = json_required_str(value, id_key)?;
        if out.insert(id.to_owned(), value).is_some() {
            return Err(format!("duplicate id {id}"));
        }
    }
    Ok(out)
}

fn collect_string_array(value: &serde_json::Value, key: &str, out: &mut BTreeSet<String>) {
    if let Some(values) = value.get(key).and_then(|value| value.as_array()) {
        out.extend(
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_owned),
        );
    }
}

fn required_json_path(
    value: &serde_json::Value,
    key: &str,
    base: &Path,
) -> Result<PathBuf, String> {
    let raw = json_required_str(value, key)?;
    if raw.is_empty() {
        return Err(format!("json field {key} must not be empty"));
    }
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        base.join(path)
    };
    if !path.is_file() {
        return Err(format!("{key} artifact {} does not exist", path.display()));
    }
    Ok(path)
}

fn copy_file(source: &Path, destination: &Path, label: &str) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create destination directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::copy(source, destination).map_err(|error| {
        format!(
            "failed to copy {label} {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn verify_declared_hash(artifact: &serde_json::Value, path: &Path) -> Result<(), String> {
    let Some(hash) = artifact.get("hash").and_then(|value| value.as_str()) else {
        return Ok(());
    };
    let algorithm = artifact
        .get("hashAlgorithm")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            format!(
                "artifact {} declares hash without hashAlgorithm",
                path.display()
            )
        })?;
    if algorithm != "blake3" {
        return Err(format!(
            "artifact {} uses unsupported hashAlgorithm {algorithm}",
            path.display()
        ));
    }
    let bytes =
        fs::read(path).map_err(|source| format!("failed to read {}: {source}", path.display()))?;
    let actual = blake3_hex(&bytes);
    if actual != hash {
        return Err(format!(
            "artifact {} hash mismatch: declared {hash}, actual {actual}",
            path.display()
        ));
    }
    Ok(())
}

fn blake3_hex(bytes: &[u8]) -> String {
    let digest = blake3::hash(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
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
