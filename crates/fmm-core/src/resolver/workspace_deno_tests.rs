use super::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn make_dir(base: &Path, rel: &str) {
    fs::create_dir_all(base.join(rel)).unwrap();
}

fn write_file(base: &Path, rel: &str, content: &str) {
    let p = base.join(rel);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

fn write_deno_project(base: &Path, rel: &str, name: &str) {
    write_file(
        base,
        &format!("{rel}/deno.json"),
        &format!(r#"{{"name":"{name}"}}"#),
    );
}

#[test]
fn deno_discoverer_detects_deno_json_and_jsonc() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "deno.json", r#"{"workspace":[]}"#);
    assert!(DenoWorkspaceDiscoverer.detect(tmp.path()));

    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "deno.jsonc", r#"{"workspace":[]}"#);
    assert!(DenoWorkspaceDiscoverer.detect(tmp.path()));
}

#[test]
fn deno_workspace_literal_paths_discover_members() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./packages/add","./packages/subtract"]}"#,
    );
    write_deno_project(tmp.path(), "packages/add", "@acme/add");
    write_deno_project(tmp.path(), "packages/subtract", "@acme/subtract");

    let info = discover(tmp.path());

    assert!(info.roots.contains(&tmp.path().to_path_buf()));
    assert!(info.roots.contains(&tmp.path().join("packages/add")));
    assert!(info.roots.contains(&tmp.path().join("packages/subtract")));
    assert_eq!(
        info.packages.get("@acme/add").unwrap(),
        &tmp.path().join("packages/add")
    );
    assert_eq!(
        info.packages.get("@acme/subtract").unwrap(),
        &tmp.path().join("packages/subtract")
    );
}

#[test]
fn deno_jsonc_workspace_accepts_comments_and_trailing_commas() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.jsonc",
        r#"{
          // Deno uses literal member paths.
          "workspace": [
            "./packages/add",
          ],
        }"#,
    );
    write_file(
        tmp.path(),
        "packages/add/deno.jsonc",
        r#"{
          "name": "@acme/add",
        }"#,
    );

    let info = discover(tmp.path());

    assert!(info.roots.contains(&tmp.path().join("packages/add")));
    assert_eq!(
        info.packages.get("@acme/add").unwrap(),
        &tmp.path().join("packages/add")
    );
}

#[test]
fn default_discover_merges_js_and_deno_workspaces() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    make_dir(tmp.path(), "packages/web");
    write_file(tmp.path(), "packages/web/package.json", r#"{"name":"web"}"#);
    write_file(tmp.path(), "deno.json", r#"{"workspace":["./deno/add"]}"#);
    write_deno_project(tmp.path(), "deno/add", "@acme/add");

    let info = discover(tmp.path());

    assert!(info.roots.contains(&tmp.path().join("packages/web")));
    assert!(info.roots.contains(&tmp.path().to_path_buf()));
    assert!(info.roots.contains(&tmp.path().join("deno/add")));
    assert!(info.packages.contains_key("web"));
    assert!(info.packages.contains_key("@acme/add"));
}
