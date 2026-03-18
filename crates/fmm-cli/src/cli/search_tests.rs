use super::*;

fn matches_loc_filter(loc: usize, op: &str, value: usize) -> bool {
    match op {
        ">" => loc > value,
        "<" => loc < value,
        ">=" => loc >= value,
        "<=" => loc <= value,
        "=" => loc == value,
        _ => false,
    }
}
use crate::manifest::{ExportLines, ExportLocation, FileEntry, Manifest};

fn test_manifest() -> Manifest {
    let mut m = Manifest::new();

    m.files.insert(
        "src/store/index.ts".to_string(),
        FileEntry {
            exports: vec!["createStore".to_string(), "destroyStore".to_string()],
            export_lines: Some(vec![
                ExportLines { start: 12, end: 45 },
                ExportLines { start: 47, end: 60 },
            ]),
            methods: None,
            imports: vec!["redux".to_string()],
            dependencies: vec!["./types".to_string()],
            loc: 120,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
        },
    );
    m.files.insert(
        "src/store/provider.tsx".to_string(),
        FileEntry {
            exports: vec!["StoreProvider".to_string()],
            export_lines: Some(vec![ExportLines { start: 8, end: 22 }]),
            methods: None,
            imports: vec!["react".to_string(), "redux".to_string()],
            dependencies: vec!["./index".to_string()],
            loc: 45,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
        },
    );
    m.files.insert(
        "src/hooks/useStore.ts".to_string(),
        FileEntry {
            exports: vec!["useStore".to_string()],
            export_lines: Some(vec![ExportLines { start: 3, end: 15 }]),
            methods: None,
            imports: vec!["react".to_string()],
            dependencies: vec!["../store/index".to_string()],
            loc: 30,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
        },
    );
    m.files.insert(
        "src/auth/login.ts".to_string(),
        FileEntry {
            exports: vec!["login".to_string(), "logout".to_string()],
            export_lines: Some(vec![
                ExportLines { start: 5, end: 20 },
                ExportLines { start: 22, end: 35 },
            ]),
            methods: None,
            imports: vec!["crypto".to_string()],
            dependencies: vec!["./session".to_string()],
            loc: 80,
            modified: None,
            function_names: Vec::new(),
            ..Default::default()
        },
    );

    // Build export index and locations
    for (path, entry) in &m.files {
        for (i, export) in entry.exports.iter().enumerate() {
            m.export_index.insert(export.clone(), path.clone());
            let lines = entry
                .export_lines
                .as_ref()
                .and_then(|el| el.get(i))
                .cloned();
            m.export_locations.insert(
                export.clone(),
                ExportLocation {
                    file: path.clone(),
                    lines,
                },
            );
        }
    }

    m
}

#[test]
fn exact_export_match() {
    let m = test_manifest();
    let matches = crate::search::find_export_matches(&m, "createStore");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "createStore");
    assert_eq!(matches[0].file, "src/store/index.ts");
}

#[test]
fn fuzzy_export_match_substring() {
    let m = test_manifest();
    let matches = crate::search::find_export_matches(&m, "store");
    assert!(matches.len() >= 3);
    let names: Vec<&str> = matches.iter().map(|h| h.name.as_str()).collect();
    assert!(names.contains(&"createStore"));
    assert!(names.contains(&"StoreProvider"));
    assert!(names.contains(&"useStore"));
}

#[test]
fn fuzzy_export_match_case_insensitive() {
    let m = test_manifest();
    let matches = crate::search::find_export_matches(&m, "STORE");
    assert!(matches.len() >= 3);
    let names: Vec<&str> = matches.iter().map(|h| h.name.as_str()).collect();
    assert!(names.contains(&"createStore"));
    assert!(names.contains(&"useStore"));
}

#[test]
fn export_no_match() {
    let m = test_manifest();
    let matches = crate::search::find_export_matches(&m, "xyznothing");
    assert!(matches.is_empty());
}

#[test]
fn exact_match_ranked_first() {
    let m = test_manifest();
    let matches = crate::search::find_export_matches(&m, "createStore");
    assert_eq!(matches[0].name, "createStore");
    assert_eq!(matches.len(), 1);
}

#[test]
fn parse_loc_operators() {
    assert_eq!(parse_loc_expr(">500").unwrap(), (">".to_string(), 500));
    assert_eq!(parse_loc_expr("<100").unwrap(), ("<".to_string(), 100));
    assert_eq!(parse_loc_expr(">=50").unwrap(), (">=".to_string(), 50));
    assert_eq!(parse_loc_expr("<=1000").unwrap(), ("<=".to_string(), 1000));
    assert_eq!(parse_loc_expr("=200").unwrap(), ("=".to_string(), 200));
    assert_eq!(parse_loc_expr("200").unwrap(), ("=".to_string(), 200));
}

#[test]
fn loc_filter_matches() {
    assert!(matches_loc_filter(600, ">", 500));
    assert!(!matches_loc_filter(500, ">", 500));
    assert!(matches_loc_filter(50, "<", 100));
    assert!(matches_loc_filter(100, ">=", 100));
    assert!(matches_loc_filter(200, "=", 200));
}

#[test]
fn bare_search_produces_grouped_text() {
    let m = test_manifest();
    let result = crate::search::bare_search(&m, "store", None);
    let text = crate::format::format_bare_search(&result, false);
    assert!(text.contains("EXPORTS"));
    assert!(text.contains("createStore"));
}

#[test]
fn filter_search_produces_per_file_text() {
    let m = test_manifest();
    let filters = crate::search::SearchFilters {
        export: None,
        imports: Some("redux".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };
    let results = crate::search::filter_search(&m, &filters);
    let text = crate::format::format_filter_search(&results, false);
    assert!(text.contains("redux"));
    assert!(text.contains("imports:"));
}

// --- Named import call-site tests ---

fn named_imports_manifest() -> Manifest {
    use std::collections::HashMap;

    let mut m = Manifest::new();

    // Two files import createServerFn from @tanstack/react-start
    let mut ni1: HashMap<String, Vec<String>> = HashMap::new();
    ni1.insert(
        "@tanstack/react-start".to_string(),
        vec!["createServerFn".to_string(), "createFileRoute".to_string()],
    );
    m.files.insert(
        "src/routes/foo.ts".to_string(),
        FileEntry {
            exports: vec![],
            imports: vec!["redux".to_string()],
            dependencies: vec![],
            loc: 50,
            named_imports: ni1,
            ..Default::default()
        },
    );

    let mut ni2: HashMap<String, Vec<String>> = HashMap::new();
    ni2.insert(
        "@tanstack/react-start".to_string(),
        vec!["createServerFn".to_string()],
    );
    m.files.insert(
        "src/routes/bar.ts".to_string(),
        FileEntry {
            exports: vec![],
            imports: vec!["axios".to_string()],
            dependencies: vec![],
            loc: 30,
            named_imports: ni2,
            ..Default::default()
        },
    );

    // One file imports a different symbol (no match for createServerFn)
    let mut ni3: HashMap<String, Vec<String>> = HashMap::new();
    ni3.insert(
        "@tanstack/react-start".to_string(),
        vec!["createFileRoute".to_string()],
    );
    m.files.insert(
        "src/routes/baz.ts".to_string(),
        FileEntry {
            exports: vec![],
            imports: vec!["redux".to_string()],
            dependencies: vec![],
            loc: 20,
            named_imports: ni3,
            ..Default::default()
        },
    );

    m
}

#[test]
fn named_import_exact_match() {
    let m = named_imports_manifest();
    let result = crate::search::bare_search(&m, "createServerFn", None);
    assert!(
        !result.named_import_hits.is_empty(),
        "should find named import hits for createServerFn"
    );
    let hit = result
        .named_import_hits
        .iter()
        .find(|h| h.symbol == "createServerFn")
        .expect("hit for createServerFn");
    assert_eq!(hit.source, "@tanstack/react-start");
    assert!(hit.files.contains(&"src/routes/foo.ts".to_string()));
    assert!(hit.files.contains(&"src/routes/bar.ts".to_string()));
    assert!(!hit.files.contains(&"src/routes/baz.ts".to_string()));
}

#[test]
fn named_import_fuzzy_match() {
    let m = named_imports_manifest();
    // "server" should match "createServerFn" case-insensitively
    let result = crate::search::bare_search(&m, "server", None);
    let hit = result
        .named_import_hits
        .iter()
        .find(|h| h.symbol == "createServerFn");
    assert!(
        hit.is_some(),
        "fuzzy match on 'server' should hit createServerFn"
    );
    // "Server" (uppercase) should also work
    let result2 = crate::search::bare_search(&m, "Server", None);
    let hit2 = result2
        .named_import_hits
        .iter()
        .find(|h| h.symbol == "createServerFn");
    assert!(hit2.is_some(), "case-insensitive fuzzy match should work");
}

#[test]
fn named_import_combined_mode_intersection() {
    let m = named_imports_manifest();
    // filter_search: imports "redux" matches foo.ts and baz.ts (not bar.ts)
    let filters = crate::search::SearchFilters {
        export: None,
        imports: Some("redux".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };
    let filter_results = crate::search::filter_search(&m, &filters);
    let filter_files: std::collections::HashSet<&str> =
        filter_results.iter().map(|r| r.file.as_str()).collect();
    let mut result = crate::search::bare_search(&m, "createServerFn", None);
    result.named_import_hits.iter_mut().for_each(|h| {
        h.files.retain(|f| filter_files.contains(f.as_str()));
    });
    result.named_import_hits.retain(|h| !h.files.is_empty());

    let hit = result
        .named_import_hits
        .iter()
        .find(|h| h.symbol == "createServerFn")
        .expect("hit survives intersection");
    // foo.ts is in both sets; bar.ts is not in filter set
    assert!(
        hit.files.contains(&"src/routes/foo.ts".to_string()),
        "foo.ts imports redux and createServerFn"
    );
    assert!(
        !hit.files.contains(&"src/routes/bar.ts".to_string()),
        "bar.ts does not import redux"
    );
}

#[test]
fn named_import_section_in_formatted_output() {
    let m = named_imports_manifest();
    let result = crate::search::bare_search(&m, "createServerFn", None);
    let text = crate::format::format_bare_search(&result, false);
    assert!(
        text.contains("NAMED IMPORTS"),
        "output should have NAMED IMPORTS section"
    );
    assert!(text.contains("createServerFn"));
    assert!(text.contains("@tanstack/react-start"));
}
