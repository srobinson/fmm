use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn setup_multi_lang_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "Cargo.toml",
        r#"
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.dependencies]
core-alias = { package = "cm-core", path = "crates/core" }
"#,
    );

    write_file(
        root,
        "crates/core/Cargo.toml",
        r#"
[package]
name = "cm-core"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(root, "crates/core/src/lib.rs", "pub mod store;\n");
    write_file(
        root,
        "crates/core/src/store.rs",
        "pub struct CxStore;\n\nimpl CxStore {\n    pub fn new() -> Self {\n        Self\n    }\n}\n",
    );

    write_file(
        root,
        "crates/cli/Cargo.toml",
        r#"
[package]
name = "cm-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
core-alias.workspace = true
"#,
    );
    write_file(
        root,
        "crates/cli/src/main.rs",
        "use core_alias::store::CxStore;\nuse serde::Serialize;\n\nfn main() {\n    let _store = CxStore::new();\n}\n",
    );

    write_file(
        root,
        "crates/web/Cargo.toml",
        r#"
[package]
name = "cm-web"
version = "0.1.0"
edition = "2024"
"#,
    );
    write_file(root, "crates/web/src/main.rs", "fn main() {}\n");
    write_file(
        root,
        "crates/web/frontend/package.json",
        r#"{"name": "cm-web-ui"}"#,
    );
    write_file(
        root,
        "crates/web/frontend/src/api.ts",
        "export const api = { status: 'ok' };\n",
    );
    write_file(
        root,
        "crates/web/frontend/src/App.tsx",
        "import { api } from './api';\n\nexport function App() {\n  return api.status;\n}\n",
    );

    fmm::cli::generate(&[root.to_string_lossy().to_string()], false, false, true).unwrap();

    tmp
}

fn load_manifest(root: &Path) -> fmm_core::manifest::Manifest {
    use fmm_core::store::FmmStore;

    fmm_store::SqliteStore::open(root)
        .and_then(|store| store.load_manifest())
        .expect("manifest should load after fmm generate")
}

fn call_tool_text(server: &fmm::mcp::SqliteMcpServer, tool: &str, args: Value) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn assert_contains(text: &str, needle: &str) {
    assert!(text.contains(needle), "expected {needle:?} in:\n{text}");
}

fn assert_not_contains(text: &str, needle: &str) {
    assert!(
        !text.contains(needle),
        "did not expect {needle:?} in:\n{text}"
    );
}

#[test]
fn multi_lang_repo_resolves_rust_and_typescript_dependency_graphs() {
    let tmp = setup_multi_lang_repo();
    let root = tmp.path();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());

    let manifest = load_manifest(root);
    assert!(manifest.workspace_packages.contains_key("cm_core"));
    assert!(manifest.workspace_packages.contains_key("cm_cli"));
    assert!(manifest.workspace_packages.contains_key("cm_web"));
    assert!(!manifest.workspace_packages.contains_key("cm-web-ui"));
    assert_eq!(
        manifest.workspace_roots.len(),
        3,
        "Cargo workspace members should be discovered without treating nested package.json as a JS workspace"
    );

    let cli_graph = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "crates/cli/src/main.rs"}),
    );
    assert_contains(&cli_graph, "crates/core/src/store.rs");
    assert_contains(&cli_graph, "external: [serde]");
    assert_not_contains(&cli_graph, "external: [core_alias]");
    assert_not_contains(&cli_graph, "crates/web/frontend/src/api.ts");

    let rust_search = call_tool_text(
        &server,
        "fmm_search",
        json!({"depends_on": "crates/core/src/store.rs"}),
    );
    assert_contains(&rust_search, "crates/cli/src/main.rs");
    assert_not_contains(&rust_search, "crates/web/frontend/src/App.tsx");

    let app_graph = call_tool_text(
        &server,
        "fmm_dependency_graph",
        json!({"file": "crates/web/frontend/src/App.tsx"}),
    );
    assert_contains(&app_graph, "crates/web/frontend/src/api.ts");
    assert_not_contains(&app_graph, "crates/core/src/store.rs");
}
