use anyhow::Result;
use colored::Colorize;
use fmm_core::manifest::OutlineReExport;

use super::{load_manifest, missing_file_diagnostic, warn_no_sidecars};

#[derive(serde::Serialize)]
struct OutlineExportJson {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

#[derive(serde::Serialize)]
struct OutlineReExportJson {
    name: String,
    origin_file: String,
    origin_start: usize,
    origin_end: usize,
}

#[derive(serde::Serialize)]
struct OutlineJson {
    file: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    imports: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
    exports: Vec<OutlineExportJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reexports: Vec<OutlineReExportJson>,
    loc: usize,
}

pub fn outline(file: &str, include_private: bool, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if file.ends_with('/') || root.join(file).is_dir() {
        anyhow::bail!(
            "'{}' is a directory. Use {} to list files.",
            file,
            format!("fmm ls {}", file).bold()
        );
    }

    let entry = manifest
        .files
        .get(file)
        .ok_or_else(|| anyhow::anyhow!(missing_file_diagnostic(&root, file)))?;

    let reexports = manifest.reexports_in_file(file);
    let reexport_names: std::collections::HashSet<&str> =
        reexports.iter().map(|r| r.name.as_str()).collect();

    if json_output {
        let exports: Vec<OutlineExportJson> = entry
            .exports
            .iter()
            .enumerate()
            .filter(|(_, name)| !reexport_names.contains(name.as_str()))
            .map(|(i, name)| {
                let lines = entry
                    .export_lines
                    .as_ref()
                    .and_then(|el| el.get(i))
                    .filter(|l| l.start > 0)
                    .map(|l| [l.start, l.end]);
                OutlineExportJson {
                    name: name.clone(),
                    lines,
                }
            })
            .collect();
        let reexport_json: Vec<OutlineReExportJson> = reexports
            .iter()
            .map(|r: &OutlineReExport| OutlineReExportJson {
                name: r.name.clone(),
                origin_file: r.origin_file.clone(),
                origin_start: r.origin_start,
                origin_end: r.origin_end,
            })
            .collect();
        let json = OutlineJson {
            file: file.to_string(),
            imports: entry.imports.clone(),
            dependencies: entry.dependencies.clone(),
            exports,
            reexports: reexport_json,
            loc: entry.loc,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        let private_by_class = if include_private {
            let class_names: Vec<&str> = entry.exports.iter().map(|s| s.as_str()).collect();
            Some(
                fmm_core::manifest::private_members::extract_private_members(
                    &root,
                    file,
                    &class_names,
                ),
            )
        } else {
            None
        };
        let top_level_fns = if include_private {
            let export_names: Vec<&str> = entry.exports.iter().map(|s| s.as_str()).collect();
            Some(
                fmm_core::manifest::private_members::extract_top_level_functions(
                    &root,
                    file,
                    &export_names,
                ),
            )
        } else {
            None
        };
        println!(
            "{}",
            fmm_core::format::format_file_outline(
                file,
                entry,
                &reexports,
                private_by_class.as_ref(),
                top_level_fns.as_deref(),
            )
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_json_mixed_local_and_reexports() {
        let json = OutlineJson {
            file: "pkg/__init__.py".to_string(),
            imports: vec![],
            dependencies: vec![],
            exports: vec![OutlineExportJson {
                name: "main".to_string(),
                lines: Some([83, 90]),
            }],
            reexports: vec![OutlineReExportJson {
                name: "BindFailure".to_string(),
                origin_file: "pkg/runner.py".to_string(),
                origin_start: 12,
                origin_end: 30,
            }],
            loc: 649,
        };
        // Round-trip through a dynamic Value so we don't hard-code the
        // pretty-printer's indentation.
        let v: serde_json::Value = serde_json::to_value(&json).unwrap();
        assert_eq!(v["file"], "pkg/__init__.py");
        assert_eq!(v["loc"], 649);
        assert_eq!(v["exports"][0]["name"], "main");
        assert_eq!(v["exports"][0]["lines"][0], 83);
        assert_eq!(v["exports"][0]["lines"][1], 90);
        assert_eq!(v["reexports"][0]["name"], "BindFailure");
        assert_eq!(v["reexports"][0]["origin_file"], "pkg/runner.py");
        assert_eq!(v["reexports"][0]["origin_start"], 12);
        assert_eq!(v["reexports"][0]["origin_end"], 30);
    }

    #[test]
    fn outline_json_no_reexports_omits_field() {
        // `reexports: Vec::is_empty` should drop the field entirely so
        // pre-Phase-3 consumers see no new key when the file has no
        // surface re-exports.
        let json = OutlineJson {
            file: "src/mod.ts".to_string(),
            imports: vec![],
            dependencies: vec![],
            exports: vec![OutlineExportJson {
                name: "foo".to_string(),
                lines: Some([1, 10]),
            }],
            reexports: vec![],
            loc: 20,
        };
        let s = serde_json::to_string(&json).unwrap();
        assert!(
            !s.contains("reexports"),
            "empty reexports must be skipped; got: {}",
            s
        );
    }

    #[test]
    fn outline_json_aliased_import_appears_only_in_exports() {
        // `from .foo import bar as baz` — `baz` is a local alias. The
        // outline handler must put it in `exports`, not `reexports`.
        // (This simulates the output the CLI handler would produce after
        // partitioning via `reexports_in_file`.)
        let json = OutlineJson {
            file: "pkg/__init__.py".to_string(),
            imports: vec![],
            dependencies: vec![],
            exports: vec![OutlineExportJson {
                name: "baz".to_string(),
                lines: Some([2, 2]),
            }],
            reexports: vec![],
            loc: 5,
        };
        let s = serde_json::to_string(&json).unwrap();
        assert!(s.contains("\"baz\""));
        assert!(!s.contains("reexports"));
    }
}
