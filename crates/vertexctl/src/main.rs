use std::env;
use std::process::ExitCode;
use vertex_ir::{
    Diagnostic, ValidationReport, explain_authority, load_manifest, render_graph_text,
    validate_manifest,
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
           vertexctl why <manifest> <service> <capability>"
    );
}
