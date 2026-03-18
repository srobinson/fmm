//! On-demand private member extraction via tree-sitter (ALP-827).
//!
//! Extracts private methods and fields from class bodies that are NOT indexed
//! in sidecars (by design — sidecars track only exported/public symbols).
//! Used by `fmm_file_outline(include_private: true)` and the private-method
//! fallback in `fmm_read_symbol("ClassName._method")`.
//!
//! Language support is extensible: add a new `PrivateMemberExtractor` impl
//! in a per-language file and register it in `extractors()`.

mod python;
mod rust;
mod typescript;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// A non-exported top-level declaration (function, arrow function, or class).
///
/// Extracted on demand when `include_private: true` is requested.
/// Also used by `fmm_read_symbol` for the `file:symbol` notation.
#[derive(Debug, Clone)]
pub struct TopLevelFunction {
    /// Declaration name.
    pub name: String,
    /// 1-based start line.
    pub start: usize,
    /// 1-based end line.
    pub end: usize,
}

/// A private class member (method or field) extracted on demand.
#[derive(Debug, Clone)]
pub struct PrivateMember {
    /// Method or field name.
    pub name: String,
    /// 1-based start line.
    pub start: usize,
    /// 1-based end line.
    pub end: usize,
    /// true = method (has a body that can be read); false = field.
    pub is_method: bool,
}

/// Language-specific on-demand tree-sitter extraction of private/non-exported symbols.
///
/// Each language that supports `include_private` features implements this trait.
/// Register new implementations in [`extractors`].
pub(crate) trait PrivateMemberExtractor {
    /// File extensions this extractor handles (without leading dot).
    fn extensions(&self) -> &'static [&'static str];

    /// Extract non-exported top-level function declarations.
    ///
    /// `exports` contains names of known exports to exclude from the result.
    fn extract_top_level_functions(
        &self,
        source: &[u8],
        exports: &[&str],
    ) -> Result<Vec<TopLevelFunction>>;

    /// Extract private members (methods, fields) from classes/structs/impls.
    ///
    /// `class_names` specifies which classes to inspect.
    fn extract_private_members(
        &self,
        source: &[u8],
        class_names: &[&str],
    ) -> Result<HashMap<String, Vec<PrivateMember>>>;
}

/// All registered language-specific extractors.
///
/// To add support for a new language: create a per-language file implementing
/// `PrivateMemberExtractor` and add it here. No other code needs to change.
fn extractors() -> Vec<Box<dyn PrivateMemberExtractor>> {
    vec![
        Box::new(typescript::TsPrivateMemberExtractor),
        Box::new(python::PyPrivateMemberExtractor),
        Box::new(rust::RsPrivateMemberExtractor),
    ]
}

/// Find the extractor for the given file extension.
fn find_extractor(ext: &str) -> Option<Box<dyn PrivateMemberExtractor>> {
    extractors()
        .into_iter()
        .find(|e| e.extensions().contains(&ext))
}

/// Extract non-exported top-level functions and classes from `rel_file`.
///
/// Returns items sorted by start line. Any name present in `exports` is excluded
/// to avoid duplicating symbols already shown in the main outline section.
/// Returns an empty vec on any read/parse error (no false positives).
pub fn extract_top_level_functions(
    root: &Path,
    rel_file: &str,
    exports: &[&str],
) -> Vec<TopLevelFunction> {
    let abs = root.join(rel_file);
    let source = match std::fs::read(&abs) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match find_extractor(&ext) {
        Some(extractor) => extractor
            .extract_top_level_functions(&source, exports)
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Find the line range `(start, end)` of a top-level function or class by name.
///
/// Searches bare top-level declarations only (non-`export`-prefixed). Declarations
/// written as `export function foo()` are wrapped in an `export_statement` node by
/// tree-sitter and will NOT be found here; use the plain-name lookup path for those.
/// Used by `fmm_read_symbol` for the `file:symbol` colon notation.
pub fn find_top_level_function_range(
    root: &Path,
    rel_file: &str,
    fn_name: &str,
) -> Option<(usize, usize)> {
    // Empty exports slice = no filtering = find any top-level function.
    let fns = extract_top_level_functions(root, rel_file, &[]);
    fns.into_iter()
        .find(|f| f.name == fn_name)
        .map(|f| (f.start, f.end))
}

/// Extract private members for each class named in `class_names` from `rel_file`.
///
/// Returns a map of `class_name → Vec<PrivateMember>` sorted by start line.
/// Returns an empty map on any read/parse error (no false positives).
pub fn extract_private_members(
    root: &Path,
    rel_file: &str,
    class_names: &[&str],
) -> HashMap<String, Vec<PrivateMember>> {
    if class_names.is_empty() {
        return HashMap::new();
    }

    let abs = root.join(rel_file);
    let source = match std::fs::read(&abs) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match find_extractor(&ext) {
        Some(extractor) => extractor
            .extract_private_members(&source, class_names)
            .unwrap_or_default(),
        None => HashMap::new(),
    }
}

/// Find the line range `(start, end)` of a specific private method in a class.
///
/// Returns `None` if the file cannot be read, the class is not found, or the
/// method is not a private method of that class.
pub fn find_private_method_range(
    root: &Path,
    rel_file: &str,
    class_name: &str,
    method_name: &str,
) -> Option<(usize, usize)> {
    let members = extract_private_members(root, rel_file, &[class_name]);
    members
        .get(class_name)?
        .iter()
        .find(|m| m.is_method && m.name == method_name)
        .map(|m| (m.start, m.end))
}
