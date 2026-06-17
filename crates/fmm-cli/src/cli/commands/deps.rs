use anyhow::Result;
use clap::Args;
use colored::Colorize;

use super::{load_manifest, missing_file_diagnostic, warn_no_sidecars};

#[derive(Args)]
pub struct DepsCommandArgs {
    /// Source file path (relative to project root, as indexed by fmm)
    #[arg(value_name = "FILE")]
    pub file: String,

    /// Traversal depth (1 = direct deps only, -1 = full closure)
    #[arg(long, default_value = "1")]
    pub depth: i32,

    /// Filter upstream/downstream by file type: all (default), source (exclude tests), tests (only tests)
    #[arg(long, default_value = "all", value_parser = ["all", "source", "tests"])]
    pub filter: String,

    /// Show reverse dependents only
    #[arg(long)]
    pub reverse: bool,

    /// Return the full transitive closure, equivalent to --depth -1
    #[arg(long)]
    pub transitive: bool,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct DepsJson {
    file: String,
    local_deps: Vec<String>,
    external: Vec<String>,
    downstream: Vec<String>,
}

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

#[derive(serde::Serialize)]
struct ReverseDepsJson {
    file: String,
    depth: i32,
    reverse_deps_count: usize,
    reverse_deps: Vec<TransitiveEntry>,
}

pub fn deps(
    file: &str,
    depth: i32,
    filter: &str,
    reverse: bool,
    transitive: bool,
    json_output: bool,
) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    let depth = effective_depth(depth, transitive)?;

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

    let entry = manifest
        .files
        .get(file)
        .ok_or_else(|| anyhow::anyhow!(missing_file_diagnostic(&root, file)))?;

    if reverse {
        return print_reverse_dependency_graph(&manifest, file, depth, &keep, json_output);
    }

    print_standard_dependency_graph(&manifest, file, entry, depth, &keep, json_output)
}

fn effective_depth(depth: i32, transitive: bool) -> Result<i32> {
    let depth = if transitive { -1 } else { depth };
    if depth != -1 && depth < 1 {
        anyhow::bail!("--depth must be >= 1 or -1 (full closure). Got {}.", depth);
    }
    Ok(depth)
}

fn print_reverse_dependency_graph<F>(
    manifest: &fmm_core::manifest::Manifest,
    file: &str,
    depth: i32,
    keep: &F,
    json_output: bool,
) -> Result<()>
where
    F: Fn(&str) -> bool,
{
    let reverse_deps: Vec<(String, i32)> =
        fmm_core::search::reverse_dependency_closure(manifest, file, depth)
            .into_iter()
            .filter(|(p, _)| keep(p))
            .collect();

    if json_output {
        let json = ReverseDepsJson {
            file: file.to_string(),
            depth,
            reverse_deps_count: reverse_deps.len(),
            reverse_deps: entries_to_json(&reverse_deps),
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "{}",
            fmm_core::format::format_reverse_dependency_graph(file, &reverse_deps, depth)
        );
    }

    Ok(())
}

fn print_standard_dependency_graph<F>(
    manifest: &fmm_core::manifest::Manifest,
    file: &str,
    entry: &fmm_core::manifest::FileEntry,
    depth: i32,
    keep: &F,
    json_output: bool,
) -> Result<()>
where
    F: Fn(&str) -> bool,
{
    if json_output {
        print_standard_dependency_graph_json(manifest, file, entry, depth, keep)?;
    } else if depth == 1 {
        let (local, external, downstream) =
            fmm_core::search::dependency_graph(manifest, file, entry);
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
            fmm_core::search::dependency_graph_transitive(manifest, file, entry, depth);
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

fn print_standard_dependency_graph_json<F>(
    manifest: &fmm_core::manifest::Manifest,
    file: &str,
    entry: &fmm_core::manifest::FileEntry,
    depth: i32,
    keep: &F,
) -> Result<()>
where
    F: Fn(&str) -> bool,
{
    if depth == 1 {
        let (local, external, downstream) =
            fmm_core::search::dependency_graph(manifest, file, entry);
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
            fmm_core::search::dependency_graph_transitive(manifest, file, entry, depth);
        let upstream: Vec<(String, i32)> = upstream.into_iter().filter(|(p, _)| keep(p)).collect();
        let downstream: Vec<(String, i32)> =
            downstream.into_iter().filter(|(p, _)| keep(p)).collect();
        let json = TransitiveDepsJson {
            file: file.to_string(),
            upstream: entries_to_json(&upstream),
            external,
            downstream: entries_to_json(&downstream),
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    }

    Ok(())
}

fn entries_to_json(entries: &[(String, i32)]) -> Vec<TransitiveEntry> {
    entries
        .iter()
        .map(|(file, depth)| TransitiveEntry {
            file: file.clone(),
            depth: *depth,
        })
        .collect()
}
