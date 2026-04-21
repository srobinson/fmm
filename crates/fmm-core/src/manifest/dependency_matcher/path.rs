use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use crate::manifest::Manifest;
use crate::resolver::workspace::WorkspaceEcosystem;

const JS_TS_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
const RUST_SOURCE_EXTENSIONS: &[&str] = &["rs"];
const GO_SOURCE_EXTENSIONS: &[&str] = &["go"];

pub fn is_js_ts_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| JS_TS_SOURCE_EXTENSIONS.contains(&ext))
}

pub fn is_rust_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RUST_SOURCE_EXTENSIONS.contains(&ext))
}

pub fn is_go_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| GO_SOURCE_EXTENSIONS.contains(&ext))
}

pub fn is_cargo_workspace_source(path: &Path, manifest: &Manifest) -> bool {
    is_rust_source_file(path)
        && manifest
            .workspace_packages_for(WorkspaceEcosystem::Rust)
            .values()
            .any(|dir| path.starts_with(dir) && dir.join("Cargo.toml").exists())
}

/// Return a reference to the lazily initialised set of source file extensions
/// from the builtin `ParserRegistry`.
///
/// Initialised once on first call; subsequent calls are lock free reads.
pub fn builtin_source_extensions() -> &'static HashSet<String> {
    static EXTS: OnceLock<HashSet<String>> = OnceLock::new();
    EXTS.get_or_init(|| {
        let registry = crate::parser::ParserRegistry::with_builtins();
        registry.source_extensions().clone()
    })
}

/// Strip a file extension from `path` when the suffix is a recognised source file
/// extension. Returns the original string unchanged for compound names like
/// `runtime.exception` or `crypto.utils` where the dot is part of the filename.
///
/// Pass `builtin_source_extensions()` at call sites that do not have a live registry.
pub fn strip_source_ext<'a>(path: &'a str, known_extensions: &HashSet<String>) -> &'a str {
    if let Some((stem, ext)) = path.rsplit_once('.') {
        if known_extensions.contains(ext) {
            stem
        } else {
            path
        }
    } else {
        path
    }
}
