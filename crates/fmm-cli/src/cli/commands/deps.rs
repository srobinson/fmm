use anyhow::Result;
use colored::Colorize;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct DepsJson {
    file: String,
    local_deps: Vec<String>,
    external: Vec<String>,
    downstream: Vec<String>,
}

pub fn deps(file: &str, depth: i32, filter: &str, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if depth != -1 && depth < 1 {
        anyhow::bail!("--depth must be >= 1 or -1 (full closure). Got {}.", depth);
    }

    // Build filter predicate — same heuristic as fmm_list_files filter.
    let config = fmm_core::config::Config::load_from_dir(&root).unwrap_or_default();
    let keep = |path: &str| -> bool {
        match filter {
            "source" => !config.is_test_file(path),
            "tests" => config.is_test_file(path),
            _ => true,
        }
    };

    if file.ends_with('/') || root.join(file).is_dir() {
        anyhow::bail!(
            "'{}' is a directory. Use {} to list files.",
            file,
            format!("fmm ls {}", file).bold()
        );
    }

    let entry = manifest.files.get(file).ok_or_else(|| {
        anyhow::anyhow!(
            "File '{}' not found in manifest. Run 'fmm generate' to index it.",
            file
        )
    })?;

    if json_output {
        if depth == 1 {
            let (local, external, downstream) =
                fmm_core::search::dependency_graph(&manifest, file, entry);
            let local: Vec<String> = local.into_iter().filter(|p| keep(p)).collect();
            let downstream: Vec<String> = downstream
                .into_iter()
                .filter(|p| keep(p.as_str()))
                .cloned()
                .collect();
            let json = DepsJson {
                file: file.to_string(),
                local_deps: local,
                external,
                downstream,
            };
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            let (upstream, external, downstream) =
                fmm_core::search::dependency_graph_transitive(&manifest, file, entry, depth);
            let upstream: Vec<(String, i32)> =
                upstream.into_iter().filter(|(p, _)| keep(p)).collect();
            let downstream: Vec<(String, i32)> =
                downstream.into_iter().filter(|(p, _)| keep(p)).collect();
            #[derive(serde::Serialize)]
            struct TransitiveEntry {
                file: String,
                depth: i32,
            }
            #[derive(serde::Serialize)]
            struct TransitiveDepsJson {
                file: String,
                upstream: Vec<TransitiveEntry>,
                external: Vec<String>,
                downstream: Vec<TransitiveEntry>,
            }
            let json = TransitiveDepsJson {
                file: file.to_string(),
                upstream: upstream
                    .iter()
                    .map(|(f, d)| TransitiveEntry {
                        file: f.clone(),
                        depth: *d,
                    })
                    .collect(),
                external,
                downstream: downstream
                    .iter()
                    .map(|(f, d)| TransitiveEntry {
                        file: f.clone(),
                        depth: *d,
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    } else if depth == 1 {
        let (local, external, downstream) =
            fmm_core::search::dependency_graph(&manifest, file, entry);
        let local: Vec<String> = local.into_iter().filter(|p| keep(p)).collect();
        let downstream: Vec<&String> = downstream
            .into_iter()
            .filter(|p| keep(p.as_str()))
            .collect();
        println!(
            "{}",
            fmm_core::format::format_dependency_graph(file, entry, &local, &external, &downstream)
        );
    } else {
        let (upstream, external, downstream) =
            fmm_core::search::dependency_graph_transitive(&manifest, file, entry, depth);
        let upstream: Vec<(String, i32)> = upstream.into_iter().filter(|(p, _)| keep(p)).collect();
        let downstream: Vec<(String, i32)> =
            downstream.into_iter().filter(|(p, _)| keep(p)).collect();
        println!(
            "{}",
            fmm_core::format::format_dependency_graph_transitive(
                file,
                entry,
                &upstream,
                &external,
                &downstream,
                depth
            )
        );
    }

    Ok(())
}
