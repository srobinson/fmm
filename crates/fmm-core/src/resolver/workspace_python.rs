use std::collections::HashMap;
use std::path::Path;

use super::{WorkspaceDiscoverer, WorkspaceEcosystem, WorkspaceInfo, debug_log, is_excluded};

/// Python workspace discoverer. Reads root `pyproject.toml`
/// `[tool.uv.workspace]` members and maps distribution names to import names.
pub struct PythonWorkspaceDiscoverer;

impl WorkspaceDiscoverer for PythonWorkspaceDiscoverer {
    fn ecosystem(&self) -> WorkspaceEcosystem {
        WorkspaceEcosystem::Python
    }

    fn detect(&self, repo_root: &Path) -> bool {
        read_pyproject_manifest(repo_root).is_some_and(|manifest| has_uv_workspace(&manifest))
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let Some(manifest) = read_pyproject_manifest(repo_root) else {
            return WorkspaceInfo::default();
        };
        if !has_uv_workspace(&manifest) {
            return WorkspaceInfo::default();
        }

        let excludes = uv_workspace_string_list(&manifest, "exclude");
        let mut roots = vec![repo_root.to_path_buf()];

        for pattern in uv_workspace_string_list(&manifest, "members") {
            let abs_pattern = repo_root.join(&pattern);
            let pattern_str = abs_pattern.to_string_lossy();
            match glob::glob(&pattern_str) {
                Ok(entries) => {
                    for entry in entries.filter_map(Result::ok) {
                        if !entry.is_dir() {
                            continue;
                        }
                        if is_excluded(&entry, repo_root, &excludes) {
                            continue;
                        }
                        if !entry.join("pyproject.toml").is_file() {
                            debug_log(&format!(
                                "uv workspace: matched member '{}' has no pyproject.toml",
                                entry.display()
                            ));
                            continue;
                        }
                        roots.push(entry);
                    }
                }
                Err(e) => {
                    debug_log(&format!("uv workspace: bad glob '{}': {}", pattern, e));
                }
            }
        }

        roots.sort();
        roots.dedup();

        let mut packages = HashMap::new();
        for root in &roots {
            if let Some(name) = read_python_import_name(root) {
                packages.insert(name, root.to_path_buf());
            }
        }

        WorkspaceInfo::new(packages, roots)
    }
}

fn read_pyproject_manifest(dir: &Path) -> Option<toml::Value> {
    let content = std::fs::read_to_string(dir.join("pyproject.toml")).ok()?;
    toml::from_str(&content).ok()
}

fn has_uv_workspace(manifest: &toml::Value) -> bool {
    manifest
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("workspace"))
        .and_then(|workspace| workspace.as_table())
        .is_some()
}

fn uv_workspace_string_list(manifest: &toml::Value, key: &str) -> Vec<String> {
    manifest
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("workspace"))
        .and_then(|workspace| workspace.get(key))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn read_python_import_name(dir: &Path) -> Option<String> {
    let manifest = read_pyproject_manifest(dir)?;
    let project_name = manifest
        .get("project")
        .and_then(|project| project.get("name"))
        .and_then(|name| name.as_str())?;
    let import_name = normalize_python_import_name(project_name)?;

    (dir.join("src").join(&import_name).is_dir() || dir.join(&import_name).is_dir())
        .then_some(import_name)
}

fn normalize_python_import_name(distribution_name: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut in_separator_run = false;

    for ch in distribution_name.to_ascii_lowercase().chars() {
        if matches!(ch, '-' | '_' | '.') {
            if !normalized.is_empty() && !in_separator_run {
                normalized.push('_');
            }
            in_separator_run = true;
        } else {
            normalized.push(ch);
            in_separator_run = false;
        }
    }

    while normalized.ends_with('_') {
        normalized.pop();
    }

    (!normalized.is_empty()).then_some(normalized)
}
