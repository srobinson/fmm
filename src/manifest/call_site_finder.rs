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

/// Returns true if the file at `rel_path` (relative to `root`) calls `method_name`.
/// Returns true on any error (graceful fallback — no false negatives).
fn file_calls_method(root: &Path, rel_path: &str, method_name: &str) -> bool {
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
}
