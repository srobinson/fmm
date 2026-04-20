use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::ImportResolver;
use super::workspace::{WorkspaceDiscoverer, WorkspaceInfo};

const DENO_CONFIG_FILES: [&str; 2] = ["deno.json", "deno.jsonc"];
const DENO_SOURCE_EXTENSIONS: [&str; 7] = ["ts", "tsx", "js", "jsx", "mjs", "cjs", "json"];

/// Deno workspace discoverer. Reads root `deno.json` or `deno.jsonc`
/// `workspace` members and maps member `name` fields to package roots.
pub struct DenoWorkspaceDiscoverer;

/// Deno import resolver for local workspace and import-map targets.
#[derive(Debug, Clone, Default)]
pub struct DenoImportResolver {
    configs: Vec<DenoConfig>,
    packages: Vec<DenoPackage>,
    source_roots: Vec<DenoSourceRoot>,
}

#[derive(Debug, Clone)]
struct DenoConfig {
    root: PathBuf,
    imports: Vec<ImportMapEntry>,
    scopes: Vec<ImportMapScope>,
}

#[derive(Debug, Clone)]
struct ImportMapEntry {
    key: String,
    target: String,
}

#[derive(Debug, Clone)]
struct ImportMapScope {
    root: Option<PathBuf>,
    key: String,
    imports: Vec<ImportMapEntry>,
}

#[derive(Debug, Clone)]
struct DenoPackage {
    name: String,
    root: PathBuf,
    exports: Option<Value>,
}

#[derive(Debug, Clone)]
struct DenoSourceRoot {
    root: PathBuf,
    is_deno: bool,
}

impl WorkspaceDiscoverer for DenoWorkspaceDiscoverer {
    fn detect(&self, repo_root: &Path) -> bool {
        deno_config_path(repo_root).is_some()
    }

    fn discover(&self, repo_root: &Path) -> WorkspaceInfo {
        let Some(root_value) = read_deno_config_value(repo_root) else {
            return WorkspaceInfo::default();
        };

        let mut roots = vec![repo_root.to_path_buf()];
        for member in deno_workspace_members(&root_value) {
            let root = repo_root.join(member);
            if root.is_dir() && deno_config_path(&root).is_some() {
                roots.push(root);
            }
        }
        roots.sort();
        roots.dedup();

        let mut packages = HashMap::new();
        for root in &roots {
            if let Some(name) = read_deno_package_name(root) {
                packages.insert(name, root.to_path_buf());
            }
        }

        WorkspaceInfo { packages, roots }
    }
}

impl DenoImportResolver {
    pub fn new(workspace_packages: &HashMap<String, PathBuf>, workspace_roots: &[PathBuf]) -> Self {
        let mut all_roots: Vec<PathBuf> = workspace_roots.to_vec();
        all_roots.extend(workspace_packages.values().cloned());
        all_roots.sort();
        all_roots.dedup();

        let mut source_roots: Vec<DenoSourceRoot> = all_roots
            .iter()
            .map(|root| DenoSourceRoot {
                root: root.clone(),
                is_deno: deno_config_path(root).is_some(),
            })
            .collect();
        source_roots.sort_by(|a, b| {
            path_depth(&b.root)
                .cmp(&path_depth(&a.root))
                .then_with(|| a.root.cmp(&b.root))
        });

        let deno_roots: Vec<PathBuf> = source_roots
            .iter()
            .filter(|root| root.is_deno)
            .map(|root| root.root.clone())
            .collect();

        let mut configs: Vec<DenoConfig> = deno_roots
            .iter()
            .filter_map(|root| read_deno_config(root))
            .collect();
        configs.sort_by(|a, b| {
            path_depth(&b.root)
                .cmp(&path_depth(&a.root))
                .then_with(|| a.root.cmp(&b.root))
        });

        let mut packages: Vec<DenoPackage> = deno_roots
            .iter()
            .filter_map(|root| {
                let value = read_deno_config_value(root)?;
                let name = deno_config_name(&value)?;
                Some(DenoPackage {
                    name,
                    root: root.clone(),
                    exports: value.get("exports").cloned(),
                })
            })
            .collect();
        packages.sort_by(|a, b| {
            b.name
                .len()
                .cmp(&a.name.len())
                .then_with(|| a.name.cmp(&b.name))
        });
        packages.dedup_by(|a, b| a.name == b.name && a.root == b.root);

        Self {
            configs,
            packages,
            source_roots,
        }
    }

    pub fn is_deno_source(&self, importer: &Path) -> bool {
        self.source_roots
            .iter()
            .find(|source_root| importer.starts_with(&source_root.root))
            .is_some_and(|source_root| source_root.is_deno)
    }

    pub fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        if !self.is_deno_source(importer) || is_external_deno_specifier(specifier) {
            return None;
        }

        let configs = self.applicable_configs(importer);
        for config in &configs {
            for scope in config.scopes.iter().filter(|scope| scope.applies(importer)) {
                if let Some(resolved) = resolve_import_map(specifier, &config.root, &scope.imports)
                {
                    return Some(resolved);
                }
            }
        }

        for config in &configs {
            if let Some(resolved) = resolve_import_map(specifier, &config.root, &config.imports) {
                return Some(resolved);
            }
        }

        if is_relative_specifier(specifier) {
            return importer
                .parent()
                .and_then(|base| resolve_local_path(base.join(specifier)));
        }

        self.resolve_workspace_package(specifier)
    }

    fn applicable_configs(&self, importer: &Path) -> Vec<&DenoConfig> {
        self.configs
            .iter()
            .filter(|config| importer.starts_with(&config.root))
            .collect()
    }

    fn resolve_workspace_package(&self, specifier: &str) -> Option<PathBuf> {
        let package = self
            .packages
            .iter()
            .find(|package| package_name_matches(&package.name, specifier))?;
        let subpath = specifier
            .strip_prefix(&package.name)
            .unwrap_or("")
            .strip_prefix('/');

        if let Some(export_target) = package_export_target(package, subpath) {
            return resolve_config_relative_path(&package.root, &export_target)
                .and_then(resolve_local_path);
        }

        match subpath {
            Some(rest) if !rest.is_empty() => resolve_local_path(package.root.join(rest)),
            _ => resolve_package_entrypoint(&package.root),
        }
    }
}

impl ImportResolver for DenoImportResolver {
    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        DenoImportResolver::resolve(self, importer, specifier)
    }
}

impl ImportMapScope {
    fn applies(&self, importer: &Path) -> bool {
        self.root
            .as_ref()
            .is_some_and(|root| importer.starts_with(root))
            || importer.to_string_lossy().starts_with(&self.key)
    }
}

fn deno_config_path(dir: &Path) -> Option<PathBuf> {
    DENO_CONFIG_FILES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn read_deno_config(dir: &Path) -> Option<DenoConfig> {
    let value = read_deno_config_value(dir)?;
    let imports = import_map_entries(value.get("imports"));
    let scopes = import_map_scopes(dir, value.get("scopes"));
    Some(DenoConfig {
        root: dir.to_path_buf(),
        imports,
        scopes,
    })
}

fn read_deno_config_value(dir: &Path) -> Option<Value> {
    let path = deno_config_path(dir)?;
    let content = std::fs::read_to_string(path).ok()?;
    parse_jsonc(&content).ok()
}

fn read_deno_package_name(dir: &Path) -> Option<String> {
    read_deno_config_value(dir).and_then(|value| deno_config_name(&value))
}

fn deno_config_name(value: &Value) -> Option<String> {
    value.get("name")?.as_str().map(String::from)
}

fn deno_workspace_members(value: &Value) -> Vec<String> {
    value
        .get("workspace")
        .and_then(|workspace| workspace.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn import_map_entries(value: Option<&Value>) -> Vec<ImportMapEntry> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut entries: Vec<ImportMapEntry> = object
        .iter()
        .filter_map(|(key, target)| {
            Some(ImportMapEntry {
                key: key.clone(),
                target: target.as_str()?.to_string(),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.key
            .len()
            .cmp(&a.key.len())
            .then_with(|| a.key.cmp(&b.key))
    });
    entries
}

fn import_map_scopes(config_root: &Path, value: Option<&Value>) -> Vec<ImportMapScope> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut scopes: Vec<ImportMapScope> = object
        .iter()
        .map(|(key, imports)| {
            let root = resolve_config_relative_path(config_root, key);
            ImportMapScope {
                root,
                key: key.clone(),
                imports: import_map_entries(Some(imports)),
            }
        })
        .collect();
    scopes.sort_by(|a, b| {
        b.key
            .len()
            .cmp(&a.key.len())
            .then_with(|| a.key.cmp(&b.key))
    });
    scopes
}

fn resolve_import_map(
    specifier: &str,
    config_root: &Path,
    entries: &[ImportMapEntry],
) -> Option<PathBuf> {
    let (entry, remainder) = entries.iter().find_map(|entry| {
        if specifier == entry.key {
            Some((entry, ""))
        } else if entry.key.ends_with('/') {
            specifier
                .strip_prefix(&entry.key)
                .map(|remainder| (entry, remainder))
        } else {
            None
        }
    })?;

    if is_external_deno_specifier(&entry.target) {
        return None;
    }

    let mut target = resolve_config_relative_path(config_root, &entry.target)?;
    if !remainder.is_empty() {
        target = target.join(remainder);
    }
    resolve_local_path(target)
}

fn resolve_config_relative_path(config_root: &Path, specifier: &str) -> Option<PathBuf> {
    if is_external_deno_specifier(specifier) {
        return None;
    }
    let path = Path::new(specifier);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_root.join(path)
    })
}

fn resolve_package_entrypoint(root: &Path) -> Option<PathBuf> {
    [
        "mod.ts",
        "mod.tsx",
        "mod.js",
        "index.ts",
        "index.tsx",
        "index.js",
    ]
    .iter()
    .find_map(|candidate| resolve_local_path(root.join(candidate)))
}

fn package_export_target(package: &DenoPackage, subpath: Option<&str>) -> Option<String> {
    let exports = package.exports.as_ref()?;
    match (exports, subpath) {
        (Value::String(target), None) => Some(target.clone()),
        (Value::Object(map), None) => map.get(".").and_then(Value::as_str).map(String::from),
        (Value::Object(map), Some(rest)) => map
            .get(&format!("./{rest}"))
            .and_then(Value::as_str)
            .map(String::from),
        _ => None,
    }
}

fn resolve_local_path(path: PathBuf) -> Option<PathBuf> {
    let mut candidates = vec![path.clone()];
    if path.extension().is_none() {
        candidates.extend(
            DENO_SOURCE_EXTENSIONS
                .iter()
                .map(|ext| path.with_extension(ext)),
        );
    }
    candidates.extend(
        [
            "mod.ts",
            "mod.tsx",
            "mod.js",
            "index.ts",
            "index.tsx",
            "index.js",
        ]
        .iter()
        .map(|entrypoint| path.join(entrypoint)),
    );

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn package_name_matches(package_name: &str, specifier: &str) -> bool {
    specifier == package_name
        || specifier
            .strip_prefix(package_name)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn is_relative_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../")
}

fn is_external_deno_specifier(specifier: &str) -> bool {
    specifier.starts_with("http://")
        || specifier.starts_with("https://")
        || specifier.starts_with("jsr:")
        || specifier.starts_with("npm:")
        || specifier.starts_with("node:")
        || specifier.starts_with("data:")
        || specifier.starts_with("blob:")
        || specifier.starts_with("file:")
}

fn path_depth(path: &Path) -> usize {
    path.components().count()
}

fn parse_jsonc(input: &str) -> Result<Value, serde_json::Error> {
    let without_comments = strip_jsonc_comments(input);
    let without_trailing_commas = strip_trailing_json_commas(&without_comments);
    serde_json::from_str(&without_trailing_commas)
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                output.push(ch);
            }
            continue;
        }
        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            } else if ch == '\n' {
                output.push('\n');
            }
            continue;
        }
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line_comment = true;
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
        } else {
            output.push(ch);
        }
    }

    output
}

fn strip_trailing_json_commas(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == ',' {
            let mut lookahead = chars.clone();
            while lookahead.peek().is_some_and(|next| next.is_whitespace()) {
                lookahead.next();
            }
            if matches!(lookahead.peek(), Some(']' | '}')) {
                continue;
            }
        }

        output.push(ch);
    }

    output
}

#[cfg(test)]
#[path = "deno_tests.rs"]
mod tests;
