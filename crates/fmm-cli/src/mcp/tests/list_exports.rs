use super::support::{assert_error, test_server, tool_text};
use fmm_core::manifest::Manifest;
use fmm_core::parser::{ExportEntry, Metadata};
use serde_json::json;

fn method_manifest() -> Manifest {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![
                ExportEntry::new("NestFactoryStatic".to_string(), 1, 200),
                ExportEntry::method(
                    "create".to_string(),
                    79,
                    113,
                    "NestFactoryStatic".to_string(),
                ),
                ExportEntry::method(
                    "createApplicationContext".to_string(),
                    166,
                    195,
                    "NestFactoryStatic".to_string(),
                ),
            ],
            loc: 200,
            ..Default::default()
        },
    );
    manifest
}

fn export_series_manifest(count: usize) -> Manifest {
    let mut manifest = Manifest::new();
    for i in 0..count {
        manifest.add_file(
            &format!("src/file{i}.ts"),
            Metadata {
                exports: vec![ExportEntry::new(format!("export{i}"), 1, 5)],
                loc: 10,
                ..Default::default()
            },
        );
    }
    manifest
}

fn regex_exports_manifest() -> Manifest {
    let mut manifest = Manifest::new();
    for (file, export) in [
        ("src/a.ts", "AppModule"),
        ("src/b.ts", "AuthService"),
        ("src/c.ts", "handleLogin"),
        ("src/d.ts", "handleLogout"),
        ("src/e.ts", "createUser"),
        ("src/f.ts", "UserController"),
        ("src/g.ts", "getProfile"),
    ] {
        manifest.add_file(
            file,
            Metadata {
                exports: vec![ExportEntry::new(export.to_string(), 1, 10)],
                loc: 50,
                ..Default::default()
            },
        );
    }
    manifest
}

#[test]
fn list_exports_pattern_includes_method_index_matches() {
    let server = test_server(method_manifest());
    let text = tool_text(&server, "fmm_list_exports", json!({"pattern": "create"}));

    assert!(!text.starts_with("ERROR:"), "expected success, got: {text}");
    assert!(
        text.contains("NestFactoryStatic.create"),
        "should contain dotted method match, got: {text}",
    );
    assert!(
        text.contains("NestFactoryStatic.createApplicationContext"),
        "should contain second dotted method, got: {text}",
    );
}

#[test]
fn list_exports_pattern_directory_filter_applies_to_methods() {
    let mut manifest = Manifest::new();
    manifest.add_file(
        "src/factory.ts",
        Metadata {
            exports: vec![ExportEntry::method(
                "create".to_string(),
                79,
                113,
                "MyFactory".to_string(),
            )],
            loc: 113,
            ..Default::default()
        },
    );
    manifest.add_file(
        "lib/other.ts",
        Metadata {
            exports: vec![ExportEntry::method(
                "create".to_string(),
                1,
                5,
                "OtherFactory".to_string(),
            )],
            loc: 5,
            ..Default::default()
        },
    );
    let server = test_server(manifest);

    let text = tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "create", "directory": "src/"}),
    );

    assert!(text.contains("MyFactory.create"), "got: {text}");
    assert!(!text.contains("OtherFactory.create"), "got: {text}");
}

#[test]
fn list_exports_truncation_notice_follows_limit() {
    let server = test_server(export_series_manifest(5));
    let text = tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "export", "limit": 2}),
    );

    assert!(
        text.contains("showing:") || text.contains("# showing:"),
        "truncation notice must appear when limit is below total; got:\n{text}",
    );
    assert!(
        text.contains("of 5"),
        "notice should state total; got:\n{text}"
    );
    assert!(
        text.contains("offset="),
        "notice should hint at offset pagination; got:\n{text}",
    );

    let server = test_server(export_series_manifest(3));
    let text = tool_text(
        &server,
        "fmm_list_exports",
        json!({"pattern": "export", "limit": 10}),
    );
    assert!(
        !text.contains("showing:") && !text.contains("# showing:"),
        "no truncation notice when all results fit; got:\n{text}",
    );
}

#[test]
fn list_exports_regex_patterns_match_expected_symbols() {
    let server = test_server(regex_exports_manifest());

    let prefix = tool_text(&server, "fmm_list_exports", json!({"pattern": "^handle"}));
    assert!(prefix.contains("handleLogin"), "got:\n{prefix}");
    assert!(prefix.contains("handleLogout"), "got:\n{prefix}");
    assert!(!prefix.contains("createUser"), "got:\n{prefix}");
    assert!(!prefix.contains("AppModule"), "got:\n{prefix}");

    let suffix = tool_text(&server, "fmm_list_exports", json!({"pattern": "Service$"}));
    assert!(suffix.contains("AuthService"), "got:\n{suffix}");
    assert!(!suffix.contains("handleLogin"), "got:\n{suffix}");

    let pascal = tool_text(&server, "fmm_list_exports", json!({"pattern": "^[A-Z]"}));
    assert!(pascal.contains("AppModule"), "got:\n{pascal}");
    assert!(pascal.contains("AuthService"), "got:\n{pascal}");
    assert!(pascal.contains("UserController"), "got:\n{pascal}");
    assert!(!pascal.contains("handleLogin"), "got:\n{pascal}");
    assert!(!pascal.contains("createUser"), "got:\n{pascal}");

    let plain = tool_text(&server, "fmm_list_exports", json!({"pattern": "module"}));
    assert!(
        plain.contains("AppModule"),
        "plain patterns must remain case insensitive; got:\n{plain}",
    );
}

#[test]
fn list_exports_invalid_regex_returns_error() {
    let server = test_server(regex_exports_manifest());
    let text = tool_text(&server, "fmm_list_exports", json!({"pattern": "[invalid"}));

    assert_error(&text);
    assert!(
        text.contains("Invalid pattern"),
        "error must say Invalid pattern; got:\n{text}",
    );
}
