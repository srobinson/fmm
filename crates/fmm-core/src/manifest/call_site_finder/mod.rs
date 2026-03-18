//! Call-site detection via tree-sitter second pass (ALP-789).
//!
//! Given a method name and a list of candidate files (from the index superset),
//! returns only the files where the method is actually called at a call site.
//!
//! Used by `tool_glossary()` to refine `used_by` for dotted queries
//! like `ClassName.method`. Non-dotted queries bypass this entirely.
//!
//! Language support is extensible: add a new `CallSiteVerifier` impl in a
//! per-language file and register it in `verifiers()`.
//!
//! Fallback semantics: if a file cannot be read, parsed, or is an unsupported
//! language, it is INCLUDED in results to avoid false negatives.

mod python;
mod rust;
mod typescript;

#[cfg(test)]
mod tests;

use std::path::Path;
use tree_sitter::{Query, QueryCursor};

/// Language-specific tree-sitter verification of call sites.
///
/// Used by `fmm_glossary` to filter false-positive dependents.
pub(crate) trait CallSiteVerifier {
    /// File extensions this verifier handles (without leading dot).
    fn extensions(&self) -> &'static [&'static str];

    /// Check whether `source` contains a method call expression
    /// matching `object.method_name(...)`.
    ///
    /// Returns `None` on parse failure (caller treats as `true` to avoid false negatives).
    fn method_call_exists(&self, source: &[u8], method_name: &str) -> Option<bool>;

    /// Analyse bare function call sites for `fn_name`.
    ///
    /// Returns how the function is called (direct, namespace, not at all).
    /// Default: assume direct caller (conservative — no false negatives).
    fn bare_call_result(&self, source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
        let _ = (source, fn_name);
        Some(BareCallSiteResult::DirectCaller)
    }
}

/// Result of bare function call-site analysis for a single candidate file.
#[derive(Debug, PartialEq, Eq)]
pub enum BareCallSiteResult {
    /// File contains a confirmed call `localAlias()` for the function.
    DirectCaller,
    /// File imports the function via a namespace (`import * as ns`) — call-site precision
    /// unavailable. The string is the namespace name (e.g. `"ns"` for `ns.fn()`).
    NamespaceCaller(String),
    /// File does not import or call the function.
    NotACaller,
}

/// All registered language-specific verifiers.
///
/// To add support for a new language: create a per-language file implementing
/// `CallSiteVerifier` and add it here. No other code needs to change.
fn verifiers() -> Vec<Box<dyn CallSiteVerifier>> {
    vec![
        Box::new(typescript::TsCallSiteVerifier),
        Box::new(python::PyCallSiteVerifier),
        Box::new(rust::RsCallSiteVerifier),
    ]
}

/// Find the verifier for the given file extension.
fn find_verifier(ext: &str) -> Option<Box<dyn CallSiteVerifier>> {
    verifiers()
        .into_iter()
        .find(|v| v.extensions().contains(&ext))
}

/// Returns true if `s` is a valid identifier in all supported languages: `[a-zA-Z_][a-zA-Z0-9_]*`.
pub(crate) fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Returns true if the query has at least one match in the tree.
pub(super) fn has_any_match(query: &Query, root: tree_sitter::Node, source: &[u8]) -> bool {
    use streaming_iterator::StreamingIterator;
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root, source);
    iter.next().is_some()
}

/// For each candidate file, check whether `method_name` appears as a call site.
///
/// Returns the subset of `candidate_files` that contain a call to the method.
/// Graceful fallback: unreadable files, unsupported extensions, and parse
/// failures are all included in the result set (no false negatives).
pub fn find_call_sites(root: &Path, method_name: &str, candidate_files: &[String]) -> Vec<String> {
    candidate_files
        .iter()
        .filter(|rel_path| file_calls_method(root, rel_path, method_name))
        .cloned()
        .collect()
}

/// Returns true if the file at `rel_path` (relative to `root`) calls `method_name`.
///
/// Returns true on any error (graceful fallback — no false negatives).
fn file_calls_method(root: &Path, rel_path: &str, method_name: &str) -> bool {
    if !is_valid_identifier(method_name) {
        return true;
    }

    let abs = root.join(rel_path);
    let source = match std::fs::read(&abs) {
        Ok(bytes) => bytes,
        Err(_) => return true,
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match find_verifier(&ext) {
        Some(verifier) => verifier
            .method_call_exists(&source, method_name)
            .unwrap_or(true),
        None => true,
    }
}

/// For module-level exported functions, determine which candidate files actually call
/// `fn_name`. Handles direct import, aliased import (`import { fn as alias }`), and
/// namespace imports (`import * as ns` — file-level fallback).
///
/// Returns `(confirmed_callers, namespace_callers)` where:
/// - `confirmed_callers`: files with a confirmed `fn_name()` or `alias()` call expression.
/// - `namespace_callers`: `(file, namespace_name)` pairs using a namespace import.
///
/// Files that cannot be read or parsed are included in `confirmed_callers` (no false negatives).
pub fn find_bare_function_callers(
    root: &Path,
    fn_name: &str,
    candidate_files: &[String],
) -> (Vec<String>, Vec<(String, String)>) {
    let mut confirmed = Vec::new();
    let mut namespace = Vec::new();

    for file in candidate_files {
        match file_bare_call_result(root, file, fn_name) {
            BareCallSiteResult::DirectCaller => confirmed.push(file.clone()),
            BareCallSiteResult::NamespaceCaller(ns) => namespace.push((file.clone(), ns)),
            BareCallSiteResult::NotACaller => {}
        }
    }

    (confirmed, namespace)
}

/// Classify a single file for bare function call analysis.
fn file_bare_call_result(root: &Path, rel_path: &str, fn_name: &str) -> BareCallSiteResult {
    if !is_valid_identifier(fn_name) {
        return BareCallSiteResult::DirectCaller;
    }

    let abs = root.join(rel_path);
    let source = match std::fs::read(&abs) {
        Ok(bytes) => bytes,
        Err(_) => return BareCallSiteResult::DirectCaller,
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match find_verifier(&ext) {
        Some(verifier) => verifier
            .bare_call_result(&source, fn_name)
            .unwrap_or(BareCallSiteResult::DirectCaller),
        None => BareCallSiteResult::DirectCaller,
    }
}
