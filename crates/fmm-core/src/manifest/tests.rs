use super::*;
use crate::manifest::GlossaryMode;
use crate::parser::{ExportEntry, Metadata};

fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
    ExportEntry::new(name.to_string(), start, end)
}

#[test]
fn export_all_tracks_all_definitions_including_duplicates() {
    let mut manifest = Manifest::new();

    let meta_a = Metadata {
        exports: vec![entry("Config", 1, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };
    let meta_b = Metadata {
        exports: vec![entry("Config", 5, 15)],
        imports: vec![],
        dependencies: vec![],
        loc: 30,
        ..Default::default()
    };

    manifest.add_file("src/config/types.rs", meta_a);
    manifest.add_file("src/config/defaults.rs", meta_b);

    // export_index last-writer-wins
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"src/config/defaults.rs".to_string())
    );
    // export_all has both
    let all = manifest.export_all.get("Config").unwrap();
    assert_eq!(all.len(), 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"src/config/types.rs"));
    assert!(files.contains(&"src/config/defaults.rs"));
}

#[test]
fn find_dependents_uses_dep_matches() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/config.ts",
        Metadata {
            exports: vec![entry("Config", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/app.ts",
        Metadata {
            exports: vec![entry("App", 1, 10)],
            imports: vec![],
            dependencies: vec!["./config".to_string()],
            loc: 20,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/other.ts",
        Metadata {
            exports: vec![entry("Other", 1, 5)],
            imports: vec![],
            dependencies: vec!["./utils".to_string()],
            loc: 5,
            ..Default::default()
        },
    );

    let deps = manifest.find_dependents("src/config.ts");
    assert_eq!(deps, vec!["src/app.ts"]);
}

#[test]
fn build_glossary_empty_pattern_returns_all() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/a.ts",
        Metadata {
            exports: vec![entry("Alpha", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    let entries = manifest.build_glossary("", GlossaryMode::All);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
}

#[test]
fn test_manifest_add_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("validateUser", 5, 20), entry("createSession", 22, 45)],
        imports: vec!["crypto".to_string()],
        dependencies: vec!["./database".to_string()],
        loc: 234,
        ..Default::default()
    };

    manifest.add_file("src/auth.ts", metadata);

    assert!(manifest.has_file("src/auth.ts"));
    assert_eq!(
        manifest.export_index.get("validateUser"),
        Some(&"src/auth.ts".to_string())
    );
    assert_eq!(
        manifest.export_index.get("createSession"),
        Some(&"src/auth.ts".to_string())
    );
    let loc = manifest.export_locations.get("validateUser").unwrap();
    assert_eq!(loc.lines.as_ref().unwrap().start, 5);
    assert_eq!(loc.lines.as_ref().unwrap().end, 20);
}

#[test]
fn test_add_file_skips_method_entries_in_export_index() {
    let mut manifest = Manifest::new();
    let metadata = Metadata {
        exports: vec![
            ExportEntry::new("MyClass".to_string(), 1, 50),
            ExportEntry::method("run".to_string(), 5, 20, "MyClass".to_string()),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    manifest.add_file("src/thing.ts", metadata);

    // Class is in export_index
    assert!(manifest.export_index.contains_key("MyClass"));
    // Method is NOT in export_index
    assert!(!manifest.export_index.contains_key("run"));
    assert!(!manifest.export_index.contains_key("MyClass.run"));
}

#[test]
fn test_method_index_populated_by_add_file() {
    let mut manifest = Manifest::new();
    let metadata = Metadata {
        exports: vec![
            ExportEntry::new("NestFactoryStatic".to_string(), 43, 381),
            ExportEntry::method(
                "create".to_string(),
                55,
                89,
                "NestFactoryStatic".to_string(),
            ),
            ExportEntry::method(
                "createApplicationContext".to_string(),
                132,
                158,
                "NestFactoryStatic".to_string(),
            ),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 400,
        ..Default::default()
    };
    manifest.add_file("src/factory.ts", metadata);

    let loc = manifest
        .method_index
        .get("NestFactoryStatic.createApplicationContext")
        .unwrap();
    assert_eq!(loc.file, "src/factory.ts");
    assert_eq!(loc.lines.as_ref().unwrap().start, 132);
    assert_eq!(loc.lines.as_ref().unwrap().end, 158);

    let create = manifest
        .method_index
        .get("NestFactoryStatic.create")
        .unwrap();
    assert_eq!(create.lines.as_ref().unwrap().start, 55);

    // Class itself is still in export_index
    assert!(manifest.export_index.contains_key("NestFactoryStatic"));
}

#[test]
fn test_manifest_validate_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("test", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata.clone());
    assert!(manifest.validate_file("file.ts", &metadata));

    let different = Metadata {
        exports: vec![entry("different", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    assert!(!manifest.validate_file("file.ts", &different));
}

#[test]
fn validate_file_ignores_method_entries() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![
            entry("MyClass", 1, 20),
            ExportEntry::method("doThing".to_string(), 5, 10, "MyClass".to_string()),
        ],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };

    manifest.add_file("svc.ts", metadata.clone());
    // Must return true — methods are not top-level exports and should not affect
    // the comparison. Prior to the fix this would return false because method
    // names leaked into current_names.
    assert!(manifest.validate_file("svc.ts", &metadata));
}

#[test]
fn test_manifest_remove_file() {
    let mut manifest = Manifest::new();

    let metadata = Metadata {
        exports: vec![entry("toRemove", 1, 5)],
        imports: vec![],
        dependencies: vec![],
        loc: 10,
        ..Default::default()
    };

    manifest.add_file("remove.ts", metadata);
    assert!(manifest.has_file("remove.ts"));
    assert!(manifest.export_index.contains_key("toRemove"));

    manifest.remove_file("remove.ts");
    assert!(!manifest.has_file("remove.ts"));
    assert!(!manifest.export_index.contains_key("toRemove"));
    assert!(!manifest.export_locations.contains_key("toRemove"));
}

#[test]
fn cross_file_collision_shadows_old_entry() {
    let mut manifest = Manifest::new();

    let meta_a = Metadata {
        exports: vec![entry("Config", 1, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 20,
        ..Default::default()
    };
    let meta_b = Metadata {
        exports: vec![entry("Config", 5, 15)],
        imports: vec![],
        dependencies: vec![],
        loc: 30,
        ..Default::default()
    };

    manifest.add_file("src/config/types.rs", meta_a);
    manifest.add_file("src/config/defaults.rs", meta_b);

    // Last writer wins
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"src/config/defaults.rs".to_string())
    );
}

#[test]
fn ts_over_js_priority_no_shadow() {
    let mut manifest = Manifest::new();

    let meta_ts = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    let meta_js = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    manifest.add_file("src/app.ts", meta_ts);
    manifest.add_file("src/app.js", meta_js);

    // .ts should win — .js doesn't overwrite
    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
}

#[test]
fn js_then_ts_order_ts_still_wins() {
    let mut manifest = Manifest::new();

    let meta_js = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };
    let meta_ts = Metadata {
        exports: vec![entry("App", 1, 50)],
        imports: vec![],
        dependencies: vec![],
        loc: 50,
        ..Default::default()
    };

    // JS added first, then TS — TS should still win
    manifest.add_file("src/app.js", meta_js);
    manifest.add_file("src/app.ts", meta_ts);

    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
}

#[test]
fn test_manifest_update_file_cleans_old_exports() {
    let mut manifest = Manifest::new();

    let metadata1 = Metadata {
        exports: vec![entry("foo", 1, 5), entry("bar", 7, 10)],
        imports: vec![],
        dependencies: vec![],
        loc: 10,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata1);

    let metadata2 = Metadata {
        exports: vec![entry("foo", 1, 5), entry("baz", 7, 12)],
        imports: vec![],
        dependencies: vec![],
        loc: 15,
        ..Default::default()
    };

    manifest.add_file("file.ts", metadata2);

    assert_eq!(
        manifest.export_index.get("foo"),
        Some(&"file.ts".to_string())
    );
    assert_eq!(
        manifest.export_index.get("baz"),
        Some(&"file.ts".to_string())
    );
    assert!(!manifest.export_index.contains_key("bar"));
    assert_eq!(manifest.file_count(), 1);
}

#[test]
fn export_all_remove_file_cleans_up() {
    let mut manifest = Manifest::new();

    manifest.add_file(
        "src/a.ts",
        Metadata {
            exports: vec![entry("Foo", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/b.ts",
        Metadata {
            exports: vec![entry("Foo", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );

    assert_eq!(manifest.export_all.get("Foo").unwrap().len(), 2);

    manifest.remove_file("src/a.ts");
    let remaining = manifest.export_all.get("Foo").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].file, "src/b.ts");
}

#[test]
fn python_reexport_does_not_claim_export_index_slot() {
    // `foo.py` defines `bar` locally; `__init__.py` re-exports it via
    // `from .foo import bar` + `__all__ = ["bar"]`. After add_file, the
    // export_index must still point at foo.py (the true definition).
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            imports: vec![],
            dependencies: vec![],
            loc: 3,
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("bar", 2, 2)], // points at import line (Phase 1)
            imports: vec![],
            dependencies: vec!["./foo".to_string()],
            loc: 3,
            named_imports: named,
            ..Default::default()
        },
    );

    // export_index still points at the original definition
    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/foo.py".to_string()),
        "re-export must not shadow the original definition"
    );
    // export_all tracks both files so glossary/discovery still works
    let all = manifest.export_all.get("bar").unwrap();
    assert_eq!(all.len(), 2);
    let files: Vec<&str> = all.iter().map(|l| l.file.as_str()).collect();
    assert!(files.contains(&"pkg/foo.py"));
    assert!(files.contains(&"pkg/__init__.py"));
}

#[test]
fn python_aliased_reexport_treated_as_local_bind() {
    // `extract_named_imports` stores the ORIGINAL name for `A as B`, so
    // `from .foo import bar as baz` stores `bar`, not `baz`. The alias
    // `baz` is a local bind unique to this file — treat it as a local
    // define for shadow detection.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("baz", 2, 2)],
            named_imports: named,
            ..Default::default()
        },
    );

    // baz: unique name, treated as local — this file owns export_index["baz"]
    assert_eq!(
        manifest.export_index.get("baz"),
        Some(&"pkg/__init__.py".to_string())
    );
    // bar: original file still owns it (re-export was via aliased form)
    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/foo.py".to_string())
    );
}

#[test]
fn python_true_name_collision_still_warns_and_shadows() {
    // Two files both DEFINE `bar` locally (no named_imports). This is a real
    // collision — last-writer-wins behavior must be preserved.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("bar", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("bar", 1, 5)],
            ..Default::default()
        },
    );

    // Last writer wins (no TS/JS priority for .py)
    assert_eq!(
        manifest.export_index.get("bar"),
        Some(&"pkg/b.py".to_string())
    );
    // export_all has both
    assert_eq!(manifest.export_all.get("bar").unwrap().len(), 2);
}

#[test]
fn export_all_remove_last_entry_cleans_key() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/only.ts",
        Metadata {
            exports: vec![entry("Unique", 1, 5)],
            imports: vec![],
            dependencies: vec![],
            loc: 10,
            ..Default::default()
        },
    );
    manifest.remove_file("src/only.ts");
    assert!(!manifest.export_all.contains_key("Unique"));
}

// --- reexports_in_file: Phase 3 outline re-export separation ---

#[test]
fn reexports_in_file_resolves_to_origin() {
    // `foo.py` defines `bar` at 1..3 locally. `__init__.py` re-exports it.
    // `reexports_in_file` for `__init__.py` must resolve the origin to foo.py:[1, 3].
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("bar", 2, 2)], // points at import line
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "bar");
    assert_eq!(rx[0].origin_file, "pkg/foo.py");
    assert_eq!(rx[0].origin_start, 1);
    assert_eq!(rx[0].origin_end, 3);
}

#[test]
fn reexports_in_file_mixed_local_and_reexports() {
    // __init__.py defines `main` locally and re-exports `bar` from .foo.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 10, 20)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("main", 5, 15), entry("bar", 3, 3)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    // Only `bar` is a re-export; `main` stays in the caller's local-defs list.
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "bar");
    assert_eq!(rx[0].origin_file, "pkg/foo.py");
    assert_eq!(rx[0].origin_start, 10);
}

#[test]
fn reexports_in_file_ignores_aliased_imports() {
    // `from .foo import bar as baz` stores `bar` in named_imports; the
    // file's exports contain `baz` (the alias). `baz` is a local bind
    // and must NOT appear in re-exports.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/foo.py",
        Metadata {
            exports: vec![entry("bar", 1, 3)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".foo".to_string(), vec!["bar".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("baz", 2, 2)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert!(
        rx.is_empty(),
        "aliased import should not be treated as a re-export; got: {:?}",
        rx
    );
}

#[test]
fn reexports_in_file_only_local_defs_returns_empty() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/mod.py",
        Metadata {
            exports: vec![entry("foo", 1, 10), entry("bar", 12, 20)],
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/mod.py");
    assert!(rx.is_empty());
}

#[test]
fn reexports_in_file_falls_back_to_import_line_when_origin_missing() {
    // `__init__.py` re-exports `sys_exit` from `sys` (a stdlib module that's
    // NOT in the workspace index). Origin lookup fails → fall back to the
    // re-exporter's own import-line range.
    let mut manifest = Manifest::new();
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert("sys".to_string(), vec!["sys_exit".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("sys_exit", 4, 4)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    assert_eq!(rx.len(), 1);
    assert_eq!(rx[0].name, "sys_exit");
    assert_eq!(
        rx[0].origin_file, "pkg/__init__.py",
        "fallback should point at the re-exporter itself"
    );
    assert_eq!(rx[0].origin_start, 4, "fallback uses the import line");
    assert_eq!(rx[0].origin_end, 4);
}

#[test]
fn reexports_in_file_sorted_alphabetically() {
    // Multiple re-exports should be returned in alphabetical order so
    // downstream formatting is stable regardless of HashMap iteration order.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("zeta", 1, 2)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("alpha", 1, 2)],
            ..Default::default()
        },
    );

    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    named.insert(".a".to_string(), vec!["zeta".to_string()]);
    named.insert(".b".to_string(), vec!["alpha".to_string()]);
    manifest.add_file(
        "pkg/__init__.py",
        Metadata {
            exports: vec![entry("zeta", 2, 2), entry("alpha", 3, 3)],
            named_imports: named,
            ..Default::default()
        },
    );

    let rx = manifest.reexports_in_file("pkg/__init__.py");
    let names: Vec<&str> = rx.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "zeta"]);
}

#[test]
fn reexports_in_file_unknown_file_returns_empty() {
    let manifest = Manifest::new();
    let rx = manifest.reexports_in_file("does/not/exist.py");
    assert!(rx.is_empty());
}

// --- Phase 4: cross-language shadow suppression ---
//
// Warnings are emitted via `eprintln!` as a side effect, so these tests assert
// on the observable behavior (who wins `export_index`, what's in `export_all`)
// rather than on captured stderr. The no-warn contract is enforced by the
// code path taken (different family -> silent branch). The manicure fixture
// integration check is the end-to-end signal that the warnings are gone.

#[test]
fn same_language_python_collision_warns_and_last_wins() {
    // Two Python files both define `UsageStats` — a true same-family shadow.
    // Last writer wins; per the refactored branch this is the non-JS
    // same-family warning path.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "pkg/a.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "pkg/b.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("UsageStats"),
        Some(&"pkg/b.py".to_string()),
        "last writer must win for same-family collisions"
    );
    assert_eq!(manifest.export_all.get("UsageStats").unwrap().len(), 2);
}

#[test]
fn cross_language_python_ts_collision_no_warn() {
    // Python dataclass + TypeScript interface mirror for an API contract.
    // Cross-family -> silent branch. Last writer still wins in export_index,
    // but no shadow warning should fire.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "api/a.py",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "web/b.ts",
        Metadata {
            exports: vec![entry("UsageStats", 1, 5)],
            ..Default::default()
        },
    );
    // Last writer wins — cross-language is intentional insert, no warn.
    assert_eq!(
        manifest.export_index.get("UsageStats"),
        Some(&"web/b.ts".to_string())
    );
    // Both definitions tracked in export_all for glossary.
    assert_eq!(manifest.export_all.get("UsageStats").unwrap().len(), 2);
    // Sanity: lang_family disagrees for these two paths — that's the predicate
    // the refactored branch keys on.
    assert_ne!(
        crate::manifest::lang_family("api/a.py"),
        crate::manifest::lang_family("web/b.ts")
    );
}

#[test]
fn cross_language_rust_ts_collision_no_warn() {
    // Rust struct + TypeScript interface mirror — same idea, different langs.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "crates/core/src/a.rs",
        Metadata {
            exports: vec![entry("Config", 1, 10)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "web/b.ts",
        Metadata {
            exports: vec![entry("Config", 1, 10)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("Config"),
        Some(&"web/b.ts".to_string())
    );
    assert_eq!(manifest.export_all.get("Config").unwrap().len(), 2);
    assert_ne!(
        crate::manifest::lang_family("crates/core/src/a.rs"),
        crate::manifest::lang_family("web/b.ts")
    );
}

#[test]
fn ts_js_priority_unchanged_within_js_family() {
    // Regression guard: the JS-family TS > JS sub-logic must survive the
    // lang_family refactor. `.ts` still wins when added after `.js`.
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/app.js",
        Metadata {
            exports: vec![entry("App", 1, 50)],
            ..Default::default()
        },
    );
    manifest.add_file(
        "src/app.ts",
        Metadata {
            exports: vec![entry("App", 1, 50)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string())
    );
    // And `.js` added after `.ts` must not overwrite.
    manifest.add_file(
        "src/app.js",
        Metadata {
            exports: vec![entry("App", 1, 50)],
            ..Default::default()
        },
    );
    assert_eq!(
        manifest.export_index.get("App"),
        Some(&"src/app.ts".to_string()),
        ".js must not overwrite .ts within the JS family"
    );
}
