//! On-demand private member extraction via tree-sitter (ALP-827).
//!
//! Extracts private methods and fields from class bodies that are NOT indexed
//! in sidecars (by design — sidecars track only exported/public symbols).
//! Used by `fmm_file_outline(include_private: true)` and the private-method
//! fallback in `fmm_read_symbol("ClassName._method")`.
//!
//! Supported languages: TypeScript/TSX/JS/JSX, Python.
//! Other languages return an empty map (graceful fallback, no false positives).

use std::collections::HashMap;
use std::path::Path;

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

    match ext.as_str() {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => {
            extract_ts_private(&source, class_names).unwrap_or_default()
        }
        "py" => extract_py_private(&source, class_names).unwrap_or_default(),
        _ => HashMap::new(),
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

// ---------------------------------------------------------------------------
// TypeScript / JS extraction
// ---------------------------------------------------------------------------

fn extract_ts_private(
    source: &[u8],
    class_names: &[&str],
) -> Option<HashMap<String, Vec<PrivateMember>>> {
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result: HashMap<String, Vec<PrivateMember>> = HashMap::new();
    walk_ts_node(tree.root_node(), source, class_names, &mut result);
    Some(result)
}

fn walk_ts_node(
    node: tree_sitter::Node,
    source: &[u8],
    class_names: &[&str],
    result: &mut HashMap<String, Vec<PrivateMember>>,
) {
    if node.kind() == "class_declaration" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(source) {
                if class_names.contains(&name) {
                    if let Some(body) = node.child_by_field_name("body") {
                        let members = collect_ts_private_members(body, source);
                        if !members.is_empty() {
                            result.insert(name.to_string(), members);
                        }
                    }
                }
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk_ts_node(child, source, class_names, result);
        }
    }
}

fn collect_ts_private_members(body: tree_sitter::Node, source: &[u8]) -> Vec<PrivateMember> {
    let mut members = Vec::new();

    for i in 0..body.child_count() {
        let child = match body.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "method_definition" => {
                if let Some(m) = ts_private_method(child, source) {
                    members.push(m);
                }
            }
            // Property declarations: `private pool: Pool;`
            "public_field_definition" => {
                if let Some(m) = ts_private_field(child, source) {
                    members.push(m);
                }
            }
            _ => {}
        }
    }

    members.sort_by_key(|m| m.start);
    members
}

/// Extract a private/protected method_definition. Returns None when public.
///
/// Handles two private-method syntaxes:
/// - TypeScript `private`/`protected` keyword: `accessibility_modifier` child present
/// - ECMAScript `#method`: name child kind is `private_property_identifier`
fn ts_private_method(node: tree_sitter::Node, source: &[u8]) -> Option<PrivateMember> {
    let name_node = node.child_by_field_name("name")?;
    let name_kind = name_node.kind();
    let name = name_node.utf8_text(source).ok()?.to_string();

    // Skip computed names like [Symbol.iterator]
    if name.starts_with('[') {
        return None;
    }

    // ECMAScript #method — private_property_identifier is the name
    if name_kind == "private_property_identifier" {
        return Some(PrivateMember {
            name,
            start: node.start_position().row + 1,
            end: node.end_position().row + 1,
            is_method: true,
        });
    }

    // TypeScript `private`/`protected` keyword
    let has_modifier = (0..node.child_count()).any(|i| {
        node.child(i as u32)
            .filter(|c| c.kind() == "accessibility_modifier")
            .and_then(|c| c.utf8_text(source).ok())
            .map(|t| t == "private" || t == "protected")
            .unwrap_or(false)
    });
    if !has_modifier {
        return None;
    }

    Some(PrivateMember {
        name,
        start: node.start_position().row + 1,
        end: node.end_position().row + 1,
        is_method: true,
    })
}

/// Extract a private/protected field declaration. Returns None when public.
///
/// In tree-sitter-typescript, all field declarations use `public_field_definition`
/// regardless of access modifier. Two private-field syntaxes are handled:
/// - TypeScript `private`/`protected` keyword: `accessibility_modifier` child present
/// - ECMAScript `#field`: name child kind is `private_property_identifier`
fn ts_private_field(node: tree_sitter::Node, source: &[u8]) -> Option<PrivateMember> {
    let name_node = node.child_by_field_name("name")?;
    let name_kind = name_node.kind();
    let name = name_node.utf8_text(source).ok()?.to_string();

    // Skip computed property names like [Symbol.hasInstance]
    if name.starts_with('[') {
        return None;
    }

    // ECMAScript #field — private_property_identifier is the name child
    if name_kind == "private_property_identifier" {
        return Some(PrivateMember {
            name,
            start: node.start_position().row + 1,
            end: node.end_position().row + 1,
            is_method: false,
        });
    }

    // TypeScript `private`/`protected` keyword
    let has_modifier = (0..node.child_count()).any(|i| {
        node.child(i as u32)
            .filter(|c| c.kind() == "accessibility_modifier")
            .and_then(|c| c.utf8_text(source).ok())
            .map(|t| t == "private" || t == "protected")
            .unwrap_or(false)
    });
    if !has_modifier {
        return None;
    }

    Some(PrivateMember {
        name,
        start: node.start_position().row + 1,
        end: node.end_position().row + 1,
        is_method: false,
    })
}

// ---------------------------------------------------------------------------
// Python extraction
// ---------------------------------------------------------------------------

fn extract_py_private(
    source: &[u8],
    class_names: &[&str],
) -> Option<HashMap<String, Vec<PrivateMember>>> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(source, None)?;

    let mut result: HashMap<String, Vec<PrivateMember>> = HashMap::new();
    walk_py_node(tree.root_node(), source, class_names, &mut result);
    Some(result)
}

fn walk_py_node(
    node: tree_sitter::Node,
    source: &[u8],
    class_names: &[&str],
    result: &mut HashMap<String, Vec<PrivateMember>>,
) {
    if node.kind() == "class_definition" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(source) {
                if class_names.contains(&name) {
                    if let Some(body) = node.child_by_field_name("body") {
                        let members = collect_py_private_members(body, source);
                        if !members.is_empty() {
                            result.insert(name.to_string(), members);
                        }
                    }
                }
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk_py_node(child, source, class_names, result);
        }
    }
}

fn collect_py_private_members(body: tree_sitter::Node, source: &[u8]) -> Vec<PrivateMember> {
    let mut members = Vec::new();

    for i in 0..body.child_count() {
        let child = match body.child(i as u32) {
            Some(c) => c,
            None => continue,
        };

        if child.kind() == "function_definition" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    if is_py_private(name) {
                        members.push(PrivateMember {
                            name: name.to_string(),
                            start: child.start_position().row + 1,
                            end: child.end_position().row + 1,
                            is_method: true,
                        });
                    }
                }
            }
        }
    }

    members.sort_by_key(|m| m.start);
    members
}

/// Python private convention: `_name` or `__name` (single/double prefix),
/// but NOT dunder methods (`__name__`) which are magic, not private.
fn is_py_private(name: &str) -> bool {
    if !name.starts_with('_') {
        return false;
    }
    // Exclude dunder methods (__init__, __repr__, etc.)
    if name.starts_with("__") && name.ends_with("__") {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_private_method_extracted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Foo {\n  public bar(): void {}\n  private baz(): void {}\n  protected qux(): void {}\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "foo.ts", &["Foo"]);
        let members = result.get("Foo").expect("Foo should have private members");

        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"baz"), "private baz missing: {:?}", names);
        assert!(names.contains(&"qux"), "protected qux missing: {:?}", names);
        assert!(
            !names.contains(&"bar"),
            "public bar should not appear: {:?}",
            names
        );
        assert!(members.iter().all(|m| m.is_method));
    }

    #[test]
    fn ts_private_field_extracted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Foo {\n  public name: string;\n  private pool: Pool;\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "foo.ts", &["Foo"]);
        let members = result.get("Foo").expect("Foo should have private fields");

        let field = members
            .iter()
            .find(|m| m.name == "pool")
            .expect("pool missing");
        assert!(!field.is_method, "field should have is_method=false");

        let public_field = members.iter().find(|m| m.name == "name");
        assert!(public_field.is_none(), "public field should not appear");
    }

    #[test]
    fn ts_public_only_class_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Foo {\n  public bar(): void {}\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "foo.ts", &["Foo"]);
        assert!(result.is_empty() || result.get("Foo").map(|v| v.is_empty()).unwrap_or(true));
    }

    #[test]
    fn ts_unknown_class_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Foo {\n  private baz(): void {}\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "foo.ts", &["Bar"]);
        assert!(!result.contains_key("Bar"));
    }

    #[test]
    fn py_private_method_extracted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "class Injector:\n    def load_instance(self):\n        pass\n\n    def _bind_token(self):\n        pass\n\n    def __init__(self):\n        pass\n";
        std::fs::write(tmp.path().join("injector.py"), src).unwrap();

        let result = extract_private_members(tmp.path(), "injector.py", &["Injector"]);
        let members = result
            .get("Injector")
            .expect("Injector should have private members");
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();

        assert!(
            names.contains(&"_bind_token"),
            "_bind_token missing: {:?}",
            names
        );
        assert!(
            !names.contains(&"load_instance"),
            "public load_instance should not appear"
        );
        assert!(
            !names.contains(&"__init__"),
            "dunder __init__ should not appear"
        );
    }

    #[test]
    fn find_private_method_range_returns_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Foo {\n  private baz(): void {\n    return;\n  }\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let range = find_private_method_range(tmp.path(), "foo.ts", "Foo", "baz");
        assert!(range.is_some(), "expected line range for baz");
        let (start, end) = range.unwrap();
        assert!(
            start >= 2 && end >= start,
            "lines [{}, {}] look wrong",
            start,
            end
        );
    }

    #[test]
    fn unsupported_extension_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "pub struct Foo {}").unwrap();
        let result = extract_private_members(tmp.path(), "foo.rs", &["Foo"]);
        assert!(result.is_empty());
    }

    // ALP-855: Regression tests for #field, public field, and TypeScript private keyword.

    /// A class mixing all three field varieties: public, TypeScript `private`, and `#field`.
    /// Only the two private varieties should appear; the public field must not.
    #[test]
    fn ts_hash_field_detected_and_public_excluded() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Counter {\n  public label: string;\n  private count: number;\n  #secret: string;\n}\n";
        std::fs::write(tmp.path().join("counter.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "counter.ts", &["Counter"]);
        let members = result
            .get("Counter")
            .expect("Counter should have private members");
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();

        assert!(
            names.contains(&"count"),
            "TypeScript private field 'count' missing: {:?}",
            names
        );
        assert!(
            names.contains(&"#secret"),
            "ECMAScript #field '#secret' missing: {:?}",
            names
        );
        assert!(
            !names.contains(&"label"),
            "public field 'label' should not appear: {:?}",
            names
        );
        // #secret must be is_method=false
        let secret = members.iter().find(|m| m.name == "#secret").unwrap();
        assert!(!secret.is_method, "#secret should be a field, not a method");
    }

    /// A class with `#method()` ECMAScript private method syntax.
    #[test]
    fn ts_hash_method_detected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Worker {\n  public run(): void {}\n  #setup(): void {}\n}\n";
        std::fs::write(tmp.path().join("worker.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "worker.ts", &["Worker"]);
        let members = result
            .get("Worker")
            .expect("Worker should have private members");
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();

        assert!(
            names.contains(&"#setup"),
            "ECMAScript #method '#setup' missing: {:?}",
            names
        );
        assert!(
            !names.contains(&"run"),
            "public method 'run' should not appear: {:?}",
            names
        );
        let setup = members.iter().find(|m| m.name == "#setup").unwrap();
        assert!(setup.is_method, "#setup should be is_method=true");
    }

    /// Public-only class using `#field` must NOT produce false positives for
    /// classes that have no private keyword fields — only `#field` ones.
    #[test]
    fn ts_class_with_only_hash_fields_no_keyword_private() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class Bag {\n  #items: string[] = [];\n  #size: number = 0;\n  public add(item: string): void {}\n}\n";
        std::fs::write(tmp.path().join("bag.ts"), src).unwrap();

        let result = extract_private_members(tmp.path(), "bag.ts", &["Bag"]);
        let members = result.get("Bag").expect("Bag should have private members");
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();

        assert!(names.contains(&"#items"), "#items missing: {:?}", names);
        assert!(names.contains(&"#size"), "#size missing: {:?}", names);
        assert!(
            !names.contains(&"add"),
            "public method 'add' should not appear: {:?}",
            names
        );
    }
}
