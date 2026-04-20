use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ImportResolver;
use super::rust_path::relative_importer_starts_with_package_dir;

#[derive(Debug, Clone)]
struct RustCrate {
    name: String,
    package_dir: PathBuf,
    src_dir: PathBuf,
    lib_root: PathBuf,
}

/// Rust workspace import resolver.
///
/// Resolves Rust `use` paths to module files using Cargo workspace metadata:
/// cross-crate paths go through the importing member's path dependencies, while
/// `crate::`, `self::`, and `super::` paths resolve relative to the importing
/// crate/module.
#[derive(Debug, Clone, Default)]
pub struct RustImportResolver {
    crates_by_name: HashMap<String, RustCrate>,
    crates_by_dir: Vec<RustCrate>,
    dependency_aliases_by_member: HashMap<PathBuf, HashMap<String, String>>,
}

impl RustImportResolver {
    pub fn new(workspace_packages: &HashMap<String, PathBuf>) -> Self {
        let mut crates_by_name = HashMap::new();
        let mut crates_by_dir = Vec::new();

        for dir in workspace_packages.values() {
            if let Some(krate) = read_rust_crate(dir) {
                crates_by_name.insert(krate.name.clone(), krate.clone());
                crates_by_dir.push(krate);
            }
        }

        crates_by_dir.sort_by(|a, b| {
            b.package_dir
                .components()
                .count()
                .cmp(&a.package_dir.components().count())
                .then_with(|| b.package_dir.cmp(&a.package_dir))
        });

        let mut resolver = Self {
            crates_by_name,
            crates_by_dir,
            dependency_aliases_by_member: HashMap::new(),
        };
        resolver.build_dependency_aliases();
        resolver
    }

    pub fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        if let Some(rest) = specifier.strip_prefix("crate::") {
            let importer_crate = self.crate_for_importer(importer)?;
            return self.resolve_from_crate_root(importer_crate, split_rust_path(rest));
        }

        if specifier == "crate" {
            return self
                .crate_for_importer(importer)
                .map(|krate| krate.lib_root.clone());
        }

        if let Some(rest) = specifier.strip_prefix("super::") {
            let base = parent_module_base(importer)?;
            if let Some(importer_crate) = self.crate_for_importer(importer)
                && base == importer_crate.src_dir
            {
                return self.resolve_from_crate_root(importer_crate, split_rust_path(rest));
            }
            return resolve_from_module_base(&base, split_rust_path(rest));
        }

        if specifier == "super" {
            let base = parent_module_base(importer)?;
            if let Some(importer_crate) = self.crate_for_importer(importer)
                && base == importer_crate.src_dir
            {
                return existing_file(&importer_crate.lib_root);
            }
            return resolve_module_file(&base);
        }

        if let Some(rest) = specifier.strip_prefix("self::") {
            let base = current_module_base(importer)?;
            return resolve_from_module_base(&base, split_rust_path(rest));
        }

        if specifier.starts_with("./") || specifier.starts_with("../") {
            return resolve_relative_dep(importer, specifier);
        }

        self.resolve_cross_crate(importer, specifier)
    }

    fn build_dependency_aliases(&mut self) {
        let crates_by_dir: HashMap<PathBuf, String> = self
            .crates_by_dir
            .iter()
            .map(|krate| (krate.package_dir.clone(), krate.name.clone()))
            .collect();

        for member in &self.crates_by_dir {
            let Some(manifest) = read_cargo_manifest(&member.package_dir) else {
                continue;
            };
            let workspace_root = find_workspace_root(&member.package_dir);
            let workspace_manifest = workspace_root
                .as_ref()
                .and_then(|root| read_cargo_manifest(root));
            let workspace_manifest_ref = workspace_manifest.as_ref();

            let mut aliases = HashMap::new();
            for (dep_key, dep_value) in dependency_entries(&manifest) {
                let workspace_dep = workspace_dependency_entry(workspace_manifest_ref, dep_key);
                let Some(target_dir) = dependency_path(
                    &member.package_dir,
                    workspace_root.as_deref(),
                    workspace_dep,
                    dep_value,
                ) else {
                    continue;
                };

                let target_dir = normalize_existing_dir(target_dir);
                let target_name = crates_by_dir
                    .get(&target_dir)
                    .cloned()
                    .or_else(|| read_rust_crate(&target_dir).map(|krate| krate.name));

                if let Some(target_name) = target_name {
                    let alias = dependency_alias(dep_key, dep_value, workspace_dep, &target_name);
                    aliases.insert(alias, target_name);
                }
            }

            if !aliases.is_empty() {
                self.dependency_aliases_by_member
                    .insert(member.package_dir.clone(), aliases);
            }
        }
    }

    fn crate_for_importer(&self, importer: &Path) -> Option<&RustCrate> {
        self.crates_by_dir.iter().find(|krate| {
            importer.starts_with(&krate.package_dir)
                || relative_importer_starts_with_package_dir(importer, &krate.package_dir)
        })
    }

    pub(crate) fn workspace_dependency_names_for_importer(&self, importer: &Path) -> Vec<String> {
        let Some(krate) = self.crate_for_importer(importer) else {
            return Vec::new();
        };
        let mut names: Vec<String> = self
            .dependency_aliases_by_member
            .get(&krate.package_dir)
            .map(|aliases| aliases.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names.dedup();
        names
    }

    fn resolve_cross_crate(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        let (crate_name, rest) = split_crate_specifier(specifier)?;
        let importer_crate = self.crate_for_importer(importer)?;
        let resolved_crate_name = self
            .dependency_aliases_by_member
            .get(&importer_crate.package_dir)
            .and_then(|aliases| aliases.get(crate_name))
            .cloned()
            .or_else(|| (importer_crate.name == crate_name).then(|| importer_crate.name.clone()))?;

        let target_crate = self.crates_by_name.get(&resolved_crate_name)?;
        self.resolve_from_crate_root(target_crate, split_rust_path(rest))
    }

    fn resolve_from_crate_root(&self, krate: &RustCrate, segments: Vec<&str>) -> Option<PathBuf> {
        resolve_from_base_dir(&krate.src_dir, &segments, Some(&krate.lib_root))
    }
}

impl ImportResolver for RustImportResolver {
    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        RustImportResolver::resolve(self, importer, specifier)
    }
}

fn read_cargo_manifest(dir: &Path) -> Option<toml::Value> {
    let content = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    toml::from_str(&content).ok()
}

fn read_rust_crate(dir: &Path) -> Option<RustCrate> {
    let manifest = read_cargo_manifest(dir)?;
    let package_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())?;
    let name = manifest
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(|name| name.as_str())
        .map(normalize_crate_name)
        .unwrap_or_else(|| normalize_crate_name(package_name));
    let configured_lib_path = manifest
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(|path| path.as_str())
        .map(PathBuf::from);
    let lib_root = configured_lib_path
        .map(|path| dir.join(path))
        .or_else(|| existing_file(&dir.join("src/lib.rs")))
        .or_else(|| existing_file(&dir.join("src/main.rs")))
        .unwrap_or_else(|| dir.join("src/lib.rs"));
    let src_dir = lib_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dir.join("src"));

    Some(RustCrate {
        name,
        package_dir: dir.to_path_buf(),
        src_dir,
        lib_root,
    })
}

fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

fn dependency_entries(manifest: &toml::Value) -> Vec<(&str, &toml::Value)> {
    let mut entries = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = manifest.get(section).and_then(|value| value.as_table()) {
            entries.extend(table.iter().map(|(name, value)| (name.as_str(), value)));
        }
    }
    entries
}

fn dependency_path(
    member_dir: &Path,
    workspace_root: Option<&Path>,
    workspace_dep_value: Option<&toml::Value>,
    dep_value: &toml::Value,
) -> Option<PathBuf> {
    if let Some(path) = dep_value
        .as_table()
        .and_then(|table| table.get("path"))
        .and_then(|value| value.as_str())
    {
        return Some(member_dir.join(path));
    }

    let workspace = dep_value
        .as_table()
        .and_then(|table| table.get("workspace"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !workspace {
        return None;
    }

    let workspace_root = workspace_root?;
    let workspace_dep = workspace_dep_value?;
    let path = workspace_dep
        .as_table()
        .and_then(|table| table.get("path"))
        .and_then(|value| value.as_str())?;

    Some(workspace_root.join(path))
}

fn workspace_dependency_entry<'a>(
    workspace_manifest: Option<&'a toml::Value>,
    dep_key: &str,
) -> Option<&'a toml::Value> {
    workspace_manifest?
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get(dep_key))
}

fn dependency_alias(
    dep_key: &str,
    dep_value: &toml::Value,
    workspace_dep_value: Option<&toml::Value>,
    target_name: &str,
) -> String {
    let is_renamed =
        has_package_override(dep_value) || workspace_dep_value.is_some_and(has_package_override);

    if is_renamed {
        normalize_crate_name(dep_key)
    } else {
        target_name.to_string()
    }
}

fn has_package_override(value: &toml::Value) -> bool {
    value
        .as_table()
        .and_then(|table| table.get("package"))
        .and_then(|value| value.as_str())
        .is_some()
}

fn find_workspace_root(member_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(member_dir);
    while let Some(dir) = current {
        if read_cargo_manifest(dir)
            .and_then(|manifest| manifest.get("workspace").cloned())
            .and_then(|workspace| workspace.as_table().cloned())
            .is_some()
        {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn normalize_existing_dir(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn split_crate_specifier(specifier: &str) -> Option<(&str, &str)> {
    if specifier.is_empty() || specifier.starts_with("::") {
        return None;
    }
    specifier
        .split_once("::")
        .map_or(Some((specifier, "")), |(name, rest)| Some((name, rest)))
}

fn split_rust_path(path: &str) -> Vec<&str> {
    if path.is_empty() {
        Vec::new()
    } else {
        path.split("::")
            .filter(|segment| !segment.is_empty())
            .collect()
    }
}

fn resolve_from_base_dir(
    base_dir: &Path,
    segments: &[&str],
    empty_path_file: Option<&PathBuf>,
) -> Option<PathBuf> {
    if segments.is_empty() {
        return empty_path_file.and_then(|path| existing_file(path));
    }

    if let Some(path) = resolve_module_path(base_dir, segments) {
        return Some(path);
    }

    if last_segment_looks_like_symbol(segments) {
        return resolve_from_base_dir(base_dir, &segments[..segments.len() - 1], empty_path_file);
    }

    None
}

fn resolve_from_module_base(base: &Path, segments: Vec<&str>) -> Option<PathBuf> {
    if segments.is_empty() {
        return resolve_module_file(base);
    }

    if let Some(path) = resolve_module_path(base, &segments) {
        return Some(path);
    }

    if last_segment_looks_like_symbol(&segments) {
        return resolve_from_module_base(base, segments[..segments.len() - 1].to_vec());
    }

    None
}

fn resolve_module_path(base_dir: &Path, segments: &[&str]) -> Option<PathBuf> {
    let mut path = base_dir.to_path_buf();
    for segment in segments {
        path.push(segment);
    }
    resolve_module_file(&path)
}

fn resolve_module_file(base: &Path) -> Option<PathBuf> {
    let file_candidate = base.with_extension("rs");
    if file_candidate.exists() {
        return Some(file_candidate);
    }

    let mod_candidate = base.join("mod.rs");
    if mod_candidate.exists() {
        return Some(mod_candidate);
    }

    None
}

fn existing_file(path: &Path) -> Option<PathBuf> {
    path.exists().then(|| path.to_path_buf())
}

fn last_segment_looks_like_symbol(segments: &[&str]) -> bool {
    segments
        .last()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}

fn current_module_base(importer: &Path) -> Option<PathBuf> {
    let file_name = importer.file_name()?.to_str()?;
    let parent = importer.parent()?;
    if matches!(file_name, "mod.rs" | "lib.rs" | "main.rs") {
        Some(parent.to_path_buf())
    } else {
        Some(parent.join(importer.file_stem()?))
    }
}

fn parent_module_base(importer: &Path) -> Option<PathBuf> {
    current_module_base(importer)?
        .parent()
        .map(Path::to_path_buf)
}

fn resolve_relative_dep(importer: &Path, specifier: &str) -> Option<PathBuf> {
    let mut path = importer.parent()?.to_path_buf();
    for segment in specifier.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                path.pop();
            }
            part => path.push(part),
        }
    }
    resolve_module_file(&path)
}

#[cfg(test)]
#[path = "rust_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rust_review_tests.rs"]
mod review_tests;
