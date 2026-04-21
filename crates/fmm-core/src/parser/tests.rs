use super::*;
use std::collections::HashSet;

#[test]
fn registry_has_builtin_parsers() {
    let registry = ParserRegistry::with_builtins();
    assert!(registry.has_parser("ts"));
    assert!(registry.has_parser("tsx"));
    assert!(registry.has_parser("js"));
    assert!(registry.has_parser("jsx"));
    assert!(registry.has_parser("py"));
    assert!(registry.has_parser("rs"));
    assert!(registry.has_parser("go"));
    assert!(registry.has_parser("java"));
    assert!(registry.has_parser("cpp"));
    assert!(registry.has_parser("hpp"));
    assert!(registry.has_parser("cs"));
    assert!(registry.has_parser("rb"));
    assert!(registry.has_parser("php"));
    assert!(registry.has_parser("c"));
    assert!(registry.has_parser("h"));
    assert!(registry.has_parser("zig"));
    assert!(registry.has_parser("lua"));
    assert!(registry.has_parser("scala"));
    assert!(registry.has_parser("sc"));
    assert!(registry.has_parser("swift"));
    assert!(registry.has_parser("kt"));
    assert!(registry.has_parser("kts"));
    assert!(registry.has_parser("dart"));
    assert!(registry.has_parser("ex"));
    assert!(registry.has_parser("exs"));
}

#[test]
fn language_id_for_avoids_parser_construction() {
    let registry = ParserRegistry::with_builtins();
    assert_eq!(registry.language_id_for("rs"), Some("rust"));
    assert_eq!(registry.language_id_for("ts"), Some("typescript"));
    assert_eq!(registry.language_id_for("js"), Some("typescript"));
    assert_eq!(registry.language_id_for("tsx"), Some("tsx"));
    assert_eq!(registry.language_id_for("jsx"), Some("tsx"));
    assert_eq!(registry.language_id_for("py"), Some("python"));
    assert_eq!(registry.language_id_for("go"), Some("go"));
    assert_eq!(registry.language_id_for("java"), Some("java"));
    assert_eq!(registry.language_id_for("cpp"), Some("cpp"));
    assert_eq!(registry.language_id_for("cs"), Some("csharp"));
    assert_eq!(registry.language_id_for("rb"), Some("ruby"));
    assert_eq!(registry.language_id_for("php"), Some("php"));
    assert_eq!(registry.language_id_for("c"), Some("c"));
    assert_eq!(registry.language_id_for("h"), Some("c"));
    assert_eq!(registry.language_id_for("zig"), Some("zig"));
    assert_eq!(registry.language_id_for("lua"), Some("lua"));
    assert_eq!(registry.language_id_for("scala"), Some("scala"));
    assert_eq!(registry.language_id_for("sc"), Some("scala"));
    assert_eq!(registry.language_id_for("swift"), Some("swift"));
    assert_eq!(registry.language_id_for("kt"), Some("kotlin"));
    assert_eq!(registry.language_id_for("kts"), Some("kotlin"));
    assert_eq!(registry.language_id_for("dart"), Some("dart"));
    assert_eq!(registry.language_id_for("ex"), Some("elixir"));
    assert_eq!(registry.language_id_for("exs"), Some("elixir"));
}

#[test]
fn registry_returns_error_for_unknown_extension() {
    let registry = ParserRegistry::with_builtins();
    assert!(registry.get_parser("unknown_ext").is_err());
}

#[test]
fn registry_creates_working_typescript_parser() {
    let registry = ParserRegistry::with_builtins();
    let mut parser = registry.get_parser("ts").unwrap();
    let result = parser.parse("export function hello() {}").unwrap();
    assert_eq!(result.metadata.export_names(), vec!["hello"]);
}

#[test]
fn registry_creates_working_python_parser() {
    let registry = ParserRegistry::with_builtins();
    let mut parser = registry.get_parser("py").unwrap();
    let result = parser
        .parse("def hello():\n    pass\n\ndef world():\n    pass")
        .unwrap();
    let names = result.metadata.export_names();
    assert!(names.contains(&"hello".to_string()));
    assert!(names.contains(&"world".to_string()));
}

#[test]
fn registry_creates_working_rust_parser() {
    let registry = ParserRegistry::with_builtins();
    let mut parser = registry.get_parser("rs").unwrap();
    let result = parser.parse("pub fn hello() {}").unwrap();
    assert_eq!(result.metadata.export_names(), vec!["hello"]);
}

#[test]
fn registry_lists_extensions() {
    let registry = ParserRegistry::with_builtins();
    let exts = registry.extensions();
    assert!(exts.contains(&"ts"));
    assert!(exts.contains(&"py"));
    assert!(exts.contains(&"rs"));
}

#[test]
fn registry_custom_register() {
    let mut registry = ParserRegistry::new();
    registry.register(&["custom"], || {
        Ok(Box::new(builtin::typescript::TypeScriptParser::new()?))
    });
    assert!(registry.has_parser("custom"));
    assert!(!registry.has_parser("ts"));
}

#[test]
fn default_registry_has_builtins() {
    let registry = ParserRegistry::default();
    assert!(registry.has_parser("ts"));
    assert!(registry.has_parser("py"));
    assert!(registry.has_parser("rs"));
}

#[test]
fn registry_source_extensions_covers_all_builtins() {
    let registry = ParserRegistry::with_builtins();
    let exts = registry.source_extensions();
    // Spot-check a selection across all language families.
    for ext in [
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "cs", "rb", "php", "c", "h",
        "zig", "lua", "scala", "swift", "kt", "dart", "ex",
    ] {
        assert!(
            exts.contains(&ext.to_string()),
            "source_extensions missing: .{ext}"
        );
    }
}

#[test]
fn registry_is_reexport_file_detects_hubs() {
    let registry = ParserRegistry::with_builtins();
    assert!(
        registry.is_reexport_file("__init__.py"),
        "__init__.py should be reexport hub"
    );
    assert!(
        registry.is_reexport_file("mod.rs"),
        "mod.rs should be reexport hub"
    );
    assert!(
        registry.is_reexport_file("index.ts"),
        "index.ts should be reexport hub"
    );
    assert!(
        registry.is_reexport_file("index.js"),
        "index.js should be reexport hub"
    );
    assert!(
        !registry.is_reexport_file("main.rs"),
        "main.rs should NOT be reexport hub"
    );
    assert!(
        !registry.is_reexport_file("lib.py"),
        "lib.py should NOT be reexport hub"
    );
}

#[test]
fn registry_is_language_test_file_detects_patterns() {
    let registry = ParserRegistry::with_builtins();
    assert!(
        registry.is_language_test_file("src/foo_test.go"),
        "_test.go suffix"
    );
    assert!(
        registry.is_language_test_file("src/foo_test.rs"),
        "_test.rs suffix"
    );
    assert!(
        registry.is_language_test_file("src/test_foo.py"),
        "test_ prefix in .py"
    );
    assert!(
        !registry.is_language_test_file("src/main.rs"),
        "main.rs not a test"
    );
    assert!(
        !registry.is_language_test_file("src/server.go"),
        "server.go not a test"
    );
}

#[test]
fn registry_has_correct_descriptor_count() {
    let registry = ParserRegistry::with_builtins();
    // 17 builtin parsers: 16 regular + 1 TSX variant = 18 descriptors
    // (TypeScript = TS + TSX = 2 descriptors)
    assert!(
        registry.descriptors().len() >= 17,
        "expected at least 17 descriptors, got {}",
        registry.descriptors().len()
    );
}

/// Guard: every parser factory extension must have a corresponding descriptor.
///
/// If this test fails, a parser was registered via `register()` without a
/// matching `register_descriptor_ref()` call. Use the `register_language!`
/// macro to ensure both are always paired.
#[test]
fn all_parsers_have_descriptors() {
    let registry = ParserRegistry::with_builtins();
    let descriptor_exts: HashSet<&str> = registry
        .descriptors()
        .iter()
        .flat_map(|d| d.extensions.iter().copied())
        .collect();

    for ext in registry.extensions() {
        assert!(
            descriptor_exts.contains(ext),
            "Extension '{ext}' has a parser factory but no descriptor. \
                 Use register_language!() to register both together."
        );
    }
}

/// Guard: descriptor extensions and parser factory extensions must be identical.
///
/// Catches stale descriptors that reference extensions removed from factory
/// registration, or typos in descriptor extension lists.
#[test]
fn descriptor_extensions_match_parser_extensions() {
    let registry = ParserRegistry::with_builtins();
    let factory_exts: HashSet<&str> = registry.extensions().into_iter().collect();
    let descriptor_exts: HashSet<&str> = registry
        .descriptors()
        .iter()
        .flat_map(|d| d.extensions.iter().copied())
        .collect();

    for ext in &descriptor_exts {
        assert!(
            factory_exts.contains(ext),
            "Descriptor declares extension '{ext}' but no parser factory is registered for it."
        );
    }

    for ext in &factory_exts {
        assert!(
            descriptor_exts.contains(ext),
            "Factory registered extension '{ext}' but no descriptor declares it."
        );
    }

    assert_eq!(
        factory_exts, descriptor_exts,
        "Factory extensions and descriptor extensions must be identical sets."
    );
}

// NOTE: Cross-module guard test `default_languages_matches_registry` lives in
// fmm-cli's config/mod.rs tests. It requires Config which is not in fmm-core.
