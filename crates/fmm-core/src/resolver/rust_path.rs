use std::path::{Component, Path, PathBuf};

pub(super) fn relative_importer_starts_with_package_dir(
    importer: &Path,
    package_dir: &Path,
) -> bool {
    if importer.is_absolute() {
        return false;
    }

    let importer_components = normal_components(importer);
    let package_components = normal_components(package_dir);
    let max_len = importer_components.len().min(package_components.len());
    if max_len == 0 {
        return false;
    }

    let min_len = if max_len >= 2 { 2 } else { 1 };
    (min_len..=max_len).rev().any(|len| {
        package_components[package_components.len() - len..] == importer_components[..len]
    })
}

pub(crate) fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect()
}

pub(crate) fn rust_module_name_from_path(target: &str) -> Option<&str> {
    let path = Path::new(target);
    let stem = path.file_stem()?.to_str()?;
    if stem == "mod" {
        path.parent()?.file_name()?.to_str()
    } else {
        Some(stem)
    }
}

pub(crate) fn is_direct_rust_mod_relative(a: &str, b: &str) -> bool {
    a != b && (is_rust_mod_parent(a, b) || is_rust_mod_parent(b, a))
}

fn is_rust_mod_parent(parent: &str, child: &str) -> bool {
    let Some(parent_base) = rust_child_module_base(Path::new(parent)) else {
        return false;
    };
    let Some(child_parent) =
        rust_module_path(Path::new(child)).and_then(|path| path.parent().map(Path::to_path_buf))
    else {
        return false;
    };

    parent_base == child_parent
}

fn rust_child_module_base(path: &Path) -> Option<PathBuf> {
    if !is_rust_file(path) {
        return None;
    }

    let file_name = path.file_name()?.to_str()?;
    match file_name {
        "lib.rs" | "main.rs" | "mod.rs" => path.parent().map(Path::to_path_buf),
        _ => {
            let stem = path.file_stem()?.to_str()?;
            Some(path.parent()?.join(stem))
        }
    }
}

fn rust_module_path(path: &Path) -> Option<PathBuf> {
    if !is_rust_file(path) {
        return None;
    }

    if path.file_name()?.to_str()? == "mod.rs" {
        path.parent().map(Path::to_path_buf)
    } else {
        let stem = path.file_stem()?.to_str()?;
        Some(path.parent()?.join(stem))
    }
}

fn is_rust_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("rs")
}

pub(crate) fn last_rust_path_segment_looks_like_symbol(segments: &[&str]) -> bool {
    segments
        .last()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}

pub(crate) fn rust_module_name_from_specifier(specifier: &str) -> Option<&str> {
    let mut segments: Vec<&str> = specifier
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return None;
    }

    if last_rust_path_segment_looks_like_symbol(&segments) {
        segments.pop();
    }

    segments.last().copied()
}
