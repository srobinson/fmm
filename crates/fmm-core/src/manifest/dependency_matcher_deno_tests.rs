use super::*;

fn write_file(base: &Path, rel: &str, content: &str) -> std::path::PathBuf {
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn build_reverse_deps_resolves_deno_workspace_package_import() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./app","./shared"]}"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    write_file(tmp.path(), "shared/deno.json", r#"{"name":"shared"}"#);
    let importer = write_file(
        tmp.path(),
        "app/main.ts",
        "import { value } from 'shared';\nconsole.log(value);\n",
    );
    let target = write_file(tmp.path(), "shared/mod.ts", "export const value = 1;\n");

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_roots.extend([
        tmp.path().to_path_buf(),
        tmp.path().join("app"),
        tmp.path().join("shared"),
    ]);
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("app"));
    manifest
        .workspace_packages
        .insert("shared".into(), tmp.path().join("shared"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "Deno workspace package import should resolve through the Deno resolver, got: {:?}",
        importers
    );
}

#[test]
fn build_reverse_deps_resolves_deno_import_map_and_ignores_remote_imports() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "deno.json",
        r#"{"workspace":["./app"],"imports":{"@/":"./app/src/","remote":"https://deno.land/std/mod.ts"}}"#,
    );
    write_file(tmp.path(), "app/deno.json", r#"{"name":"app"}"#);
    let importer = write_file(
        tmp.path(),
        "app/src/main.ts",
        "import { local } from '@/local.ts';\nimport 'https://deno.land/std/assert/mod.ts';\nimport 'jsr:@std/assert';\nimport 'remote';\nconsole.log(local);\n",
    );
    let target = write_file(tmp.path(), "app/src/local.ts", "export const local = 1;\n");

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest
        .workspace_roots
        .extend([tmp.path().to_path_buf(), tmp.path().join("app")]);
    manifest
        .workspace_packages
        .insert("app".into(), tmp.path().join("app"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec![
                "@/local.ts".to_string(),
                "https://deno.land/std/assert/mod.ts".to_string(),
                "jsr:@std/assert".to_string(),
                "remote".to_string(),
            ],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert_eq!(importers, vec![importer_key]);
    assert_eq!(reverse_deps.len(), 1);
}

#[test]
fn build_reverse_deps_keeps_js_workspace_edges_under_deno_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_file(tmp.path(), "deno.json", r#"{"workspace":["./deno/app"]}"#);
    write_file(tmp.path(), "deno/app/deno.json", r#"{"name":"deno-app"}"#);
    write_file(
        tmp.path(),
        "package.json",
        r#"{"workspaces":["packages/*"]}"#,
    );
    write_file(tmp.path(), "packages/web/package.json", r#"{"name":"web"}"#);
    write_file(
        tmp.path(),
        "packages/shared/package.json",
        r#"{"name":"shared-js","main":"index.ts"}"#,
    );
    let importer = write_file(
        tmp.path(),
        "packages/web/index.ts",
        "import { value } from 'shared-js';\nconsole.log(value);\n",
    );
    let target = write_file(
        tmp.path(),
        "packages/shared/index.ts",
        "export const value = 1;\n",
    );

    let importer_key = importer.to_string_lossy().into_owned();
    let target_key = target.to_string_lossy().into_owned();
    let mut manifest = Manifest::new();
    manifest.workspace_roots.extend([
        tmp.path().to_path_buf(),
        tmp.path().join("deno/app"),
        tmp.path().join("packages/web"),
        tmp.path().join("packages/shared"),
    ]);
    manifest
        .workspace_packages
        .insert("deno-app".into(), tmp.path().join("deno/app"));
    manifest
        .workspace_packages
        .insert("web".into(), tmp.path().join("packages/web"));
    manifest
        .workspace_packages
        .insert("shared-js".into(), tmp.path().join("packages/shared"));
    manifest
        .files
        .insert(target_key.clone(), crate::manifest::FileEntry::default());
    manifest.files.insert(
        importer_key.clone(),
        crate::manifest::FileEntry {
            imports: vec!["shared-js".to_string()],
            ..Default::default()
        },
    );

    let reverse_deps = build_reverse_deps(&manifest);
    let importers = reverse_deps.get(&target_key).cloned().unwrap_or_default();

    assert!(
        importers.contains(&importer_key),
        "JS workspace import should still use JS resolution, got: {:?}",
        importers
    );
}
