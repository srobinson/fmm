use super::*;
use std::fs;
use tempfile::TempDir;

fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, content).unwrap();
    path
}

fn packages(tmp: &TempDir) -> HashMap<String, PathBuf> {
    HashMap::from([
        ("app".to_string(), tmp.path().join("app")),
        ("shared".to_string(), tmp.path().join("shared")),
    ])
}

fn roots(tmp: &TempDir) -> Vec<PathBuf> {
    vec![
        tmp.path().to_path_buf(),
        tmp.path().join("app"),
        tmp.path().join("shared"),
    ]
}

fn write_workspace(tmp: &TempDir) {
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./app","./shared"],"imports":{"@/":"./app/src/"}}"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    write_file(
        tmp.path(),
        "shared/deno.json",
        r#"{"name":"shared","exports":"./mod.ts"}"#,
    );
    write_file(tmp.path(), "app/src/main.ts", "");
    write_file(tmp.path(), "app/src/util.ts", "");
    write_file(tmp.path(), "shared/mod.ts", "");
}

#[test]
fn jsonc_parser_accepts_comments_trailing_commas_and_urls() {
    let value = parse_jsonc(
        r#"
        {
          // comment
          "imports": {
            "std": "https://deno.land/std/mod.ts",
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(
        value["imports"]["std"].as_str(),
        Some("https://deno.land/std/mod.ts")
    );
}

#[test]
fn relative_import_resolves_inside_deno_root() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = DenoImportResolver::new(&packages(&tmp), &roots(&tmp));
    let importer = tmp.path().join("app/src/main.ts");

    assert_eq!(
        resolver.resolve(&importer, "./util.ts"),
        Some(tmp.path().join("app/src/util.ts"))
    );
}

#[test]
fn workspace_package_name_resolves_to_export() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = DenoImportResolver::new(&packages(&tmp), &roots(&tmp));
    let importer = tmp.path().join("app/src/main.ts");

    assert_eq!(
        resolver.resolve(&importer, "shared"),
        Some(tmp.path().join("shared/mod.ts"))
    );
}

#[test]
fn import_map_prefix_resolves_local_target() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = DenoImportResolver::new(&packages(&tmp), &roots(&tmp));
    let importer = tmp.path().join("app/src/main.ts");

    assert_eq!(
        resolver.resolve(&importer, "@/util.ts"),
        Some(tmp.path().join("app/src/util.ts"))
    );
}

#[test]
fn scope_import_overrides_top_level_import() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{
          "workspace":["./app","./shared","./patched"],
          "imports":{"shared":"./shared/mod.ts"},
          "scopes":{"./app/":{"shared":"./patched/mod.ts"}}
        }"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    write_file(tmp.path(), "shared/deno.json", r#"{"name":"shared"}"#);
    write_file(tmp.path(), "patched/deno.json", r#"{"name":"patched"}"#);
    write_file(tmp.path(), "app/main.ts", "");
    write_file(tmp.path(), "shared/mod.ts", "");
    write_file(tmp.path(), "patched/mod.ts", "");

    let packages = HashMap::from([
        ("app".to_string(), tmp.path().join("app")),
        ("shared".to_string(), tmp.path().join("shared")),
        ("patched".to_string(), tmp.path().join("patched")),
    ]);
    let roots = vec![
        tmp.path().to_path_buf(),
        tmp.path().join("app"),
        tmp.path().join("shared"),
        tmp.path().join("patched"),
    ];
    let resolver = DenoImportResolver::new(&packages, &roots);

    assert_eq!(
        resolver.resolve(&tmp.path().join("app/main.ts"), "shared"),
        Some(tmp.path().join("patched/mod.ts"))
    );
}

#[test]
fn url_jsr_and_npm_imports_stay_unresolved() {
    let tmp = TempDir::new().unwrap();
    write_workspace(&tmp);
    let resolver = DenoImportResolver::new(&packages(&tmp), &roots(&tmp));
    let importer = tmp.path().join("app/src/main.ts");

    assert_eq!(
        resolver.resolve(&importer, "https://deno.land/std/assert/mod.ts"),
        None
    );
    assert_eq!(resolver.resolve(&importer, "jsr:@std/assert"), None);
    assert_eq!(resolver.resolve(&importer, "npm:chalk"), None);
}
