use std::collections::HashSet;

use crate::manifest::Manifest;

use super::path::strip_source_ext;

pub fn match_path_dependency(
    dep: &str,
    target_file: &str,
    dependent_file: &str,
    known_extensions: &HashSet<String>,
) -> bool {
    // Resolve the dependency path relative to the dependent file's directory.
    let dep_dir = dependent_file
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");

    let mut parts: Vec<&str> = if dep_dir.is_empty() {
        Vec::new()
    } else {
        dep_dir.split('/').collect()
    };

    let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
    for segment in dep_clean.split('/') {
        if segment == ".." {
            parts.pop();
        } else if segment != "." {
            parts.push(segment);
        }
    }

    let resolved = parts.join("/");

    // Strip extension from the target file path. For the resolved dep, only
    // strip if the suffix is a known source file extension. NestJS-style
    // compound names like `runtime.exception` use `.exception` as part of
    // the filename.
    let resolved_stem = strip_source_ext(&resolved, known_extensions);
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(target_file);

    if resolved_stem == target_stem {
        return true;
    }

    if let Some(package_stem) = target_stem.strip_suffix("/__init__")
        && resolved_stem == package_stem
    {
        return true;
    }

    if let Some(module_path_str) = dep.strip_prefix("crate::") {
        let module_path = module_path_str.replace("::", "/");
        return target_stem.ends_with(&module_path);
    }

    if dep.contains('/') && !dep.starts_with('.') {
        let segments: Vec<&str> = dep.split('/').collect();
        for start in 1..segments.len() {
            let suffix = segments[start..].join("/");
            if target_stem.ends_with(&suffix) {
                return true;
            }
        }
    }

    false
}

fn resolve_python_relative_path(dep: &str, source_file: &str) -> Option<String> {
    debug_assert!(dep.starts_with('.') && !dep.starts_with("./"));
    let dots = dep.chars().take_while(|&c| c == '.').count();
    let module_name = &dep[dots..];

    let source_dir = source_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = if source_dir.is_empty() {
        vec![]
    } else {
        source_dir.split('/').collect()
    };

    // Single dot means current package; each additional dot means one level up.
    for _ in 1..dots {
        parts.pop()?;
    }

    if module_name.is_empty() {
        return None;
    }

    for part in module_name.split('.') {
        parts.push(part);
    }

    Some(parts.join("/"))
}

pub fn match_python_relative_dependency(
    dep: &str,
    target_file: &str,
    dependent_file: &str,
) -> bool {
    if !dep.starts_with('.') || dep.starts_with("./") || dep.starts_with("../") {
        return false;
    }
    if let Some(resolved) = resolve_python_relative_path(dep, dependent_file) {
        let target_stem = target_file
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(target_file);
        resolved == target_stem
    } else {
        false
    }
}

pub fn match_python_dotted_dependency(dep: &str, target_file: &str) -> bool {
    if dep.starts_with('.') || dep.contains('/') || dep.contains("::") || !dep.contains('.') {
        return false;
    }
    let path_stem = dep.replace('.', "/");
    let target_stem = target_file
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(target_file);
    let effective = target_stem.strip_suffix("/__init__").unwrap_or(target_stem);
    effective == path_stem.as_str() || effective.ends_with(&format!("/{}", path_stem))
}

/// Attempt to resolve a dependency string to a file path present in the manifest.
///
/// Handles Python-style relative imports (`._run`, `..config`), JS/TS-style relative
/// paths (`./utils`, `../config`), Go module paths, and Rust `crate::` paths.
///
/// `known_extensions` is the set of recognised source file extensions without
/// the leading dot, typically from `ParserRegistry::source_extensions()`.
pub fn try_resolve_local_dep(
    dep: &str,
    source_file: &str,
    manifest: &Manifest,
    known_extensions: &HashSet<String>,
) -> Option<String> {
    if dep.starts_with('.') && !dep.starts_with("./") && !dep.starts_with("../") {
        let resolved_stem = resolve_python_relative_path(dep, source_file)?;
        for candidate in [
            format!("{}.py", resolved_stem),
            format!("{}/__init__.py", resolved_stem),
            resolved_stem.clone(),
        ] {
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return None;
    }

    if dep.starts_with("./") || dep.starts_with("../") {
        if let Some(found) = manifest
            .files
            .keys()
            .find(|path| match_path_dependency(dep, path, source_file, known_extensions))
        {
            return Some(found.clone());
        }

        let dep_dir = source_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let mut parts: Vec<&str> = if dep_dir.is_empty() {
            Vec::new()
        } else {
            dep_dir.split('/').collect()
        };
        let dep_clean = dep.strip_prefix("./").unwrap_or(dep);
        for segment in dep_clean.split('/') {
            match segment {
                ".." => {
                    parts.pop();
                }
                "." => {}
                s => parts.push(s),
            }
        }
        let resolved = parts.join("/");
        for index_name in &["index.ts", "index.tsx", "index.js", "index.jsx"] {
            let candidate = format!("{}/{}", resolved, index_name);
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return None;
    }

    if dep.contains('/') || dep.contains("::") {
        return manifest
            .files
            .keys()
            .find(|path| match_path_dependency(dep, path, source_file, known_extensions))
            .cloned();
    }

    if dep.contains('.') {
        let path_stem = dep.replace('.', "/");
        for candidate in [
            format!("{}.py", path_stem),
            format!("{}/__init__.py", path_stem),
            path_stem.clone(),
        ] {
            if manifest.files.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        return manifest
            .files
            .keys()
            .find(|path| {
                let stem = path.rsplit_once('.').map(|(s, _)| s).unwrap_or(path);
                let effective = stem.strip_suffix("/__init__").unwrap_or(stem);
                effective == path_stem.as_str() || effective.ends_with(&format!("/{}", path_stem))
            })
            .cloned();
    }

    None
}
