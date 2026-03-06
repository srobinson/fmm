//! Call-site detection via tree-sitter second pass (ALP-789).
//!
//! Given a method name and a list of candidate files (from the index superset),
//! returns only the files where the method is actually called at a call site.
//!
//! Used by `tool_glossary()` to refine `used_by` for dotted queries
//! like `ClassName.method`. Non-dotted queries bypass this entirely.
//!
//! Fallback semantics: if a file cannot be read, parsed, or is an unsupported
//! language, it is INCLUDED in results to avoid false negatives.

use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TSParser, Query, QueryCursor};

/// For each candidate file, check whether `method_name` appears as a call site.
/// Returns the subset of `candidate_files` that contain a call to the method.
///
/// Graceful fallback: unreadable files, unsupported extensions, and parse
/// failures are all included in the result set (no false negatives).
pub fn find_call_sites(root: &Path, method_name: &str, candidate_files: &[String]) -> Vec<String> {
    candidate_files
        .iter()
        .filter(|rel_path| file_calls_method(root, rel_path, method_name))
        .cloned()
        .collect()
}

/// Returns true if `s` is a valid identifier in all supported languages: `[a-zA-Z_][a-zA-Z0-9_]*`.
fn is_valid_identifier(s: &str) -> bool {
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

/// Returns true if the file at `rel_path` (relative to `root`) calls `method_name`.
/// Returns true on any error (graceful fallback — no false negatives).
fn file_calls_method(root: &Path, rel_path: &str, method_name: &str) -> bool {
    // Guard: method_name must be a valid identifier to safely embed in query strings.
    // Non-identifier input would malform the tree-sitter query and produce false positives;
    // return true (include file) to preserve the "no false negatives" invariant.
    if !is_valid_identifier(method_name) {
        return true;
    }

    let abs = root.join(rel_path);

    let source = match std::fs::read(&abs) {
        Ok(bytes) => bytes,
        Err(_) => return true, // unreadable -> include
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => {
            call_exists_ts(&source, method_name).unwrap_or(true)
        }
        "py" => call_exists_py(&source, method_name).unwrap_or(true),
        "rs" => call_exists_rs(&source, method_name).unwrap_or(true),
        _ => true, // unsupported extension -> include
    }
}

/// Check for TypeScript/JS call expression: `something.METHOD_NAME(...)`
fn call_exists_ts(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call_expression
            function: (member_expression
                property: (property_identifier) @prop
                (#eq? @prop "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(has_any_match(&query, tree.root_node(), source))
}

/// Check for Python call expression: `something.METHOD_NAME(...)`
fn call_exists_py(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call
            function: (attribute
                attribute: (identifier) @attr
                (#eq? @attr "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(has_any_match(&query, tree.root_node(), source))
}

/// Check for Rust method call: `something.METHOD_NAME(...)`
fn call_exists_rs(source: &[u8], method_name: &str) -> Option<bool> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = TSParser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let query_src = format!(
        r#"(call_expression
            function: (field_expression
                field: (field_identifier) @field
                (#eq? @field "{}")))
        "#,
        method_name
    );
    let query = Query::new(&lang, &query_src).ok()?;
    Some(has_any_match(&query, tree.root_node(), source))
}

/// Returns true if the query has at least one match in the tree.
fn has_any_match(query: &Query, root: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = QueryCursor::new();
    let mut iter = cursor.matches(query, root, source);
    iter.next().is_some()
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
        return BareCallSiteResult::DirectCaller; // guard: include on invalid input (no false neg)
    }

    let abs = root.join(rel_path);
    let source = match std::fs::read(&abs) {
        Ok(bytes) => bytes,
        Err(_) => return BareCallSiteResult::DirectCaller, // unreadable → include
    };

    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => {
            bare_call_result_ts(&source, fn_name).unwrap_or(BareCallSiteResult::DirectCaller)
        }
        _ => BareCallSiteResult::DirectCaller, // unsupported → include
    }
}

/// Analyse a TS/JS source file for bare function calls of `fn_name`.
///
/// Algorithm:
/// 1. Parse imports to find how `fn_name` is imported locally (direct, aliased, namespace, none).
/// 2. For direct/aliased imports, run a call_expression query for the local name.
/// 3. For namespace imports, return `NamespaceCaller` immediately (can't narrow).
/// 4. If not imported at all, return `NotACaller`.
fn bare_call_result_ts(source: &[u8], fn_name: &str) -> Option<BareCallSiteResult> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();

    // --- 1. Find how fn_name is imported ---

    // Query A: direct named import `import { fn_name }`
    let q_direct_src = format!(
        r#"(import_statement
            (import_clause
                (named_imports
                    (import_specifier !alias
                        name: (identifier) @name
                        (#eq? @name "{}")))))
        "#,
        fn_name
    );

    // Query B: aliased named import `import { fn_name as alias }`  — capture @local is the alias
    let q_aliased_src = format!(
        r#"(import_statement
            (import_clause
                (named_imports
                    (import_specifier
                        name: (identifier) @orig
                        alias: (identifier) @local
                        (#eq? @orig "{}")))))
        "#,
        fn_name
    );

    // Query C: namespace import `import * as ns`
    let q_ns_src = r#"(import_statement
        (import_clause
            (namespace_import (identifier) @ns)))"#;

    let q_direct = Query::new(&lang, &q_direct_src).ok()?;
    let q_aliased = Query::new(&lang, &q_aliased_src).ok()?;
    let q_ns = Query::new(&lang, q_ns_src).ok()?;

    // Collect all local names to search for calls
    let mut local_names: Vec<String> = Vec::new();
    let mut ns_names: Vec<String> = Vec::new();
    let mut has_direct = false;

    // Direct import — local name is fn_name itself
    {
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_direct, root, source);
        if iter.next().is_some() {
            has_direct = true;
            local_names.push(fn_name.to_string());
        }
    }

    // Aliased import — collect all aliases
    {
        let local_idx = q_aliased
            .capture_index_for_name("local")
            .unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_aliased, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == local_idx {
                    if let Ok(text) = cap.node.utf8_text(source) {
                        local_names.push(text.to_string());
                    }
                }
            }
        }
    }

    // Namespace imports — collect all namespace names
    {
        let ns_idx = q_ns.capture_index_for_name("ns").unwrap_or(u32::MAX);
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&q_ns, root, source);
        while let Some(m) = iter.next() {
            for cap in m.captures {
                if cap.index == ns_idx {
                    if let Ok(text) = cap.node.utf8_text(source) {
                        ns_names.push(text.to_string());
                    }
                }
            }
        }
    }

    // --- 2. If namespace import found, return NamespaceCaller ---
    // Note: a file can have both a namespace import and a direct import (rare but possible).
    // If it also has a direct import, prefer DirectCaller (more precise).
    if !ns_names.is_empty() && local_names.is_empty() {
        return Some(BareCallSiteResult::NamespaceCaller(
            ns_names.into_iter().next().unwrap(),
        ));
    }

    // --- 3. If no direct/aliased import, return NotACaller ---
    if local_names.is_empty() {
        return Some(BareCallSiteResult::NotACaller);
    }

    // --- 4. Search for bare call expressions using each local name ---
    let _ = has_direct; // suppress unused warning
    for local_name in &local_names {
        if !is_valid_identifier(local_name) {
            continue;
        }
        let q_call_src = format!(
            r#"(call_expression function: (identifier) @callee (#eq? @callee "{}"))"#,
            local_name
        );
        if let Ok(q_call) = Query::new(&lang, &q_call_src) {
            if has_any_match(&q_call, root, source) {
                return Some(BareCallSiteResult::DirectCaller);
            }
        }
    }

    // Imported but never called directly
    Some(BareCallSiteResult::NotACaller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        name.to_string()
    }

    #[test]
    fn ts_caller_is_included() {
        let dir = TempDir::new().unwrap();
        let caller = write_file(
            &dir,
            "caller.ts",
            "import { Foo } from './foo';\nconst f = new Foo();\nf.doThing();\n",
        );
        let bystander = write_file(
            &dir,
            "bystander.ts",
            "import { Foo } from './foo';\nconst f = new Foo();\n// never calls doThing\n",
        );
        let result = find_call_sites(dir.path(), "doThing", &[caller.clone(), bystander.clone()]);
        assert!(result.contains(&caller), "caller should be included");
        assert!(
            !result.contains(&bystander),
            "bystander should be excluded; got: {:?}",
            result
        );
    }

    #[test]
    fn py_caller_is_included() {
        let dir = TempDir::new().unwrap();
        let caller = write_file(
            &dir,
            "caller.py",
            "from foo import Foo\nf = Foo()\nf.do_thing()\n",
        );
        let bystander = write_file(
            &dir,
            "bystander.py",
            "from foo import Foo\nf = Foo()\n# no call\n",
        );
        let result = find_call_sites(dir.path(), "do_thing", &[caller.clone(), bystander.clone()]);
        assert!(result.contains(&caller), "py caller included");
        assert!(!result.contains(&bystander), "py bystander excluded");
    }

    #[test]
    fn rs_caller_is_included() {
        let dir = TempDir::new().unwrap();
        let caller = write_file(
            &dir,
            "caller.rs",
            "fn main() { let f = Foo::new(); f.do_thing(); }\n",
        );
        let bystander = write_file(
            &dir,
            "bystander.rs",
            "fn main() { let f = Foo::new(); /* no do_thing */ }\n",
        );
        let result = find_call_sites(dir.path(), "do_thing", &[caller.clone(), bystander.clone()]);
        assert!(result.contains(&caller), "rs caller included");
        assert!(!result.contains(&bystander), "rs bystander excluded");
    }

    #[test]
    fn unreadable_file_is_included() {
        let dir = TempDir::new().unwrap();
        let ghost = "ghost.ts".to_string();
        let result = find_call_sites(dir.path(), "someMethod", std::slice::from_ref(&ghost));
        assert!(
            result.contains(&ghost),
            "unreadable file included by fallback"
        );
    }

    #[test]
    fn unsupported_extension_is_included() {
        let dir = TempDir::new().unwrap();
        let f = write_file(&dir, "module.go", "package main\nfunc main() {}\n");
        let result = find_call_sites(dir.path(), "someMethod", std::slice::from_ref(&f));
        assert!(result.contains(&f), "unsupported ext included by fallback");
    }

    #[test]
    fn invalid_identifier_returns_all_candidates() {
        // method_name with a double-quote would malform the query; guard should include all files.
        let dir = TempDir::new().unwrap();
        let f = write_file(&dir, "some.ts", "const x = 1;\n");
        let result = find_call_sites(dir.path(), "bad\"name", std::slice::from_ref(&f));
        assert!(
            result.contains(&f),
            "invalid identifier falls back to include-all"
        );
    }

    #[test]
    fn is_valid_identifier_accepts_common_names() {
        assert!(is_valid_identifier("doThing"));
        assert!(is_valid_identifier("_private"));
        assert!(is_valid_identifier("camelCase123"));
    }

    #[test]
    fn is_valid_identifier_rejects_bad_input() {
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123abc"));
        assert!(!is_valid_identifier("has\"quote"));
        assert!(!is_valid_identifier("has.dot"));
        assert!(!is_valid_identifier("has space"));
    }

    // --- ALP-866: bare function call-site tests ---

    /// Fixture 1: direct call `scheduleUpdate()` — must appear in confirmed callers.
    #[test]
    fn bare_fn_direct_call_is_confirmed() {
        let dir = TempDir::new().unwrap();
        let caller = write_file(
            &dir,
            "caller.ts",
            "import { scheduleUpdate } from './scheduler';\nscheduleUpdate();\n",
        );
        let bystander = write_file(
            &dir,
            "bystander.ts",
            "import { scheduleUpdate } from './scheduler';\n// never calls it\nconst x = 1;\n",
        );
        let (confirmed, ns) = find_bare_function_callers(
            dir.path(),
            "scheduleUpdate",
            &[caller.clone(), bystander.clone()],
        );
        assert!(
            confirmed.contains(&caller),
            "direct caller should be confirmed"
        );
        assert!(
            !confirmed.contains(&bystander),
            "non-caller should be excluded"
        );
        assert!(ns.is_empty(), "no namespace callers expected");
    }

    /// Fixture 2: `import { scheduleUpdate as su }` + calls `su()` — must appear.
    #[test]
    fn bare_fn_aliased_import_is_resolved() {
        let dir = TempDir::new().unwrap();
        let aliased = write_file(
            &dir,
            "aliased.ts",
            "import { scheduleUpdate as su } from './scheduler';\nsu();\n",
        );
        let (confirmed, ns) =
            find_bare_function_callers(dir.path(), "scheduleUpdate", &[aliased.clone()]);
        assert!(
            confirmed.contains(&aliased),
            "aliased caller should be confirmed"
        );
        assert!(ns.is_empty());
    }

    /// Fixture 3: `import * as wl` — must appear as namespace caller.
    #[test]
    fn bare_fn_namespace_import_becomes_namespace_caller() {
        let dir = TempDir::new().unwrap();
        let ns_file = write_file(
            &dir,
            "ns_user.ts",
            "import * as wl from './scheduler';\nwl.scheduleUpdate();\n",
        );
        let (confirmed, ns) =
            find_bare_function_callers(dir.path(), "scheduleUpdate", &[ns_file.clone()]);
        assert!(
            !confirmed.contains(&ns_file),
            "namespace user should NOT be in confirmed"
        );
        assert!(
            ns.iter().any(|(f, _)| f == &ns_file),
            "namespace user should be in ns_callers"
        );
        let ns_name = ns
            .iter()
            .find(|(f, _)| f == &ns_file)
            .map(|(_, n)| n.as_str())
            .unwrap_or("");
        assert_eq!(ns_name, "wl", "namespace name should be 'wl'");
    }

    /// Fixture 4: imports but never calls — must NOT appear.
    #[test]
    fn bare_fn_import_without_call_is_excluded() {
        let dir = TempDir::new().unwrap();
        let importer = write_file(
            &dir,
            "importer.ts",
            "import { scheduleUpdate } from './scheduler';\n// never calls scheduleUpdate\nconst x = 42;\n",
        );
        let (confirmed, ns) =
            find_bare_function_callers(dir.path(), "scheduleUpdate", &[importer.clone()]);
        assert!(
            !confirmed.contains(&importer),
            "importer-without-call should be excluded"
        );
        assert!(ns.is_empty());
    }

    /// Fixture 5: re-exports but doesn't call — must NOT appear.
    #[test]
    fn bare_fn_reexport_without_call_is_excluded() {
        let dir = TempDir::new().unwrap();
        let reexporter = write_file(
            &dir,
            "index.ts",
            "export { scheduleUpdate } from './scheduler';\n",
        );
        let (confirmed, ns) =
            find_bare_function_callers(dir.path(), "scheduleUpdate", &[reexporter.clone()]);
        assert!(
            !confirmed.contains(&reexporter),
            "re-exporter should be excluded"
        );
        assert!(ns.is_empty());
    }

    /// A file importing a different function from same module should be excluded.
    #[test]
    fn bare_fn_unrelated_import_from_same_module_is_excluded() {
        let dir = TempDir::new().unwrap();
        let other = write_file(
            &dir,
            "other.ts",
            "import { otherFn } from './scheduler';\notherFn();\n",
        );
        let (confirmed, ns) =
            find_bare_function_callers(dir.path(), "scheduleUpdate", &[other.clone()]);
        assert!(
            !confirmed.contains(&other),
            "unrelated importer should be excluded"
        );
        assert!(ns.is_empty());
    }
}
