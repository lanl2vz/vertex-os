mod graph;
mod model;
mod validation;
mod why;

use std::fs;
use std::path::Path;

pub use graph::render_graph_text;
pub use model::*;
pub use validation::{Diagnostic, DiagnosticSeverity, ValidationReport, validate_manifest};
pub use why::explain_authority;

#[derive(Debug)]
pub enum LoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Json {
        path: String,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "failed to read {path}: {source}"),
            Self::Json { path, source } => write!(f, "failed to parse {path}: {source}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<GenerationManifest, LoadError> {
    let path = path.as_ref();
    let display = path.display().to_string();
    let input = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: display.clone(),
        source,
    })?;
    serde_json::from_str(&input).map_err(|source| LoadError::Json {
        path: display,
        source,
    })
}

pub fn to_pretty_json(manifest: &GenerationManifest) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = include_str!("../../../examples/hello-generation.vertex.json");

    #[test]
    fn hello_manifest_validates_without_errors() {
        let manifest: GenerationManifest = serde_json::from_str(HELLO).unwrap();
        let report = validate_manifest(&manifest);

        assert_eq!(report.errors.len(), 0, "{:#?}", report.errors);
    }

    #[test]
    fn why_explains_echo_log_authority() {
        let manifest: GenerationManifest = serde_json::from_str(HELLO).unwrap();
        let explanation = explain_authority(&manifest, "svc:echo-server", "cap:log.sink");

        assert!(explanation.contains("svc:echo-server can use cap:log.sink"));
        assert!(explanation.contains("svc:echo-server declares a requirement"));
    }
}
