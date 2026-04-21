use std::collections::HashSet;

mod local;
mod path;
mod reverse;
mod workspace;

pub(crate) use local::try_resolve_local_dep;
pub(crate) use path::{builtin_source_extensions, strip_source_ext};
pub(super) use reverse::build_reverse_deps;

#[cfg(test)]
use super::Manifest;
#[cfg(test)]
use std::path::Path;

/// Check if a dependency path from `dependent_file` resolves to `target_file`.
/// Dependencies are stored as relative paths like "../utils/crypto.utils.js"
/// and need to be resolved against the dependent file's directory.
///
/// `known_extensions` is the set of recognised source-file extensions without
/// the leading dot, typically from `ParserRegistry::source_extensions()`.
pub fn dep_matches(
    dep: &str,
    target_file: &str,
    dependent_file: &str,
    known_extensions: &HashSet<String>,
) -> bool {
    local::match_path_dependency(dep, target_file, dependent_file, known_extensions)
}

/// Match a Python-style relative import (`._run`, `..utils`) against a target
/// file path, given the dependent file's location. Used for downstream detection.
pub fn python_dep_matches(dep: &str, target_file: &str, dependent_file: &str) -> bool {
    local::match_python_relative_dependency(dep, target_file, dependent_file)
}

/// Match a Python absolute module import (`agno.models.message`) against a target
/// file path. Used for downstream detection.
///
/// Returns true when the dotted path resolves to the target file, considering
/// both root-relative paths (`agno/models/message.py`) and src-layout paths
/// (`src/agno/models/message.py`).
pub fn dotted_dep_matches(dep: &str, target_file: &str) -> bool {
    local::match_python_dotted_dependency(dep, target_file)
}

#[cfg(test)]
#[path = "dependency_matcher_go_tests.rs"]
mod go_tests;

#[cfg(test)]
#[path = "dependency_matcher_deno_tests.rs"]
mod deno_tests;

#[cfg(test)]
#[path = "dependency_matcher_review_tests.rs"]
mod review_tests;

#[cfg(test)]
#[path = "dependency_matcher_tests.rs"]
mod tests;
