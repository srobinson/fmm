//! Tests for private member and top-level function extraction.

#[cfg(test)]
mod tests {
    use crate::manifest::private_members::{
        extract_private_members, extract_top_level_functions, find_private_method_range,
        find_top_level_function_range,
    };

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

    // ---------------------------------------------------------------------------
    // ALP-910: TopLevelFunction extraction tests
    // ---------------------------------------------------------------------------

    #[test]
    fn ts_non_exported_function_declaration() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export function exportedFn() {}\nfunction helperFn() {}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_top_level_functions(tmp.path(), "foo.ts", &["exportedFn"]);
        let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"helperFn"), "helperFn missing: {:?}", names);
        assert!(
            !names.contains(&"exportedFn"),
            "exportedFn should be excluded: {:?}",
            names
        );
    }

    #[test]
    fn ts_non_exported_arrow_function() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export const pub = () => {};\nconst helper = () => {};\nconst data = 42;\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_top_level_functions(tmp.path(), "foo.ts", &["pub"]);
        let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"helper"), "helper missing: {:?}", names);
        assert!(
            !names.contains(&"pub"),
            "exported pub should be excluded: {:?}",
            names
        );
        assert!(
            !names.contains(&"data"),
            "non-function data should not appear: {:?}",
            names
        );
    }

    #[test]
    fn ts_non_exported_class_declaration() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "export class PublicClass {}\nclass InternalClass {}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let result = extract_top_level_functions(tmp.path(), "foo.ts", &["PublicClass"]);
        let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"InternalClass"),
            "InternalClass missing: {:?}",
            names
        );
        assert!(
            !names.contains(&"PublicClass"),
            "PublicClass should be excluded: {:?}",
            names
        );
    }

    #[test]
    fn ts_find_top_level_function_range_returns_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "function helperFn() {\n  return 42;\n}\n";
        std::fs::write(tmp.path().join("foo.ts"), src).unwrap();

        let range = find_top_level_function_range(tmp.path(), "foo.ts", "helperFn");
        assert!(range.is_some(), "expected range for helperFn");
        let (start, end) = range.unwrap();
        assert_eq!(start, 1, "start should be 1");
        assert_eq!(end, 3, "end should be 3");
    }

    #[test]
    fn py_non_exported_function() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "def public_fn():\n    pass\n\ndef _private_fn():\n    pass\n";
        std::fs::write(tmp.path().join("mod.py"), src).unwrap();

        // Simulate fmm exporting only public_fn
        let result = extract_top_level_functions(tmp.path(), "mod.py", &["public_fn"]);
        let names: Vec<&str> = result.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"_private_fn"),
            "_private_fn missing: {:?}",
            names
        );
        assert!(
            !names.contains(&"public_fn"),
            "public_fn should be excluded: {:?}",
            names
        );
    }

    #[test]
    fn top_level_unsupported_extension_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "fn helper() {}").unwrap();
        let result = extract_top_level_functions(tmp.path(), "foo.rs", &[]);
        assert!(result.is_empty());
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
