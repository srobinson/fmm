use std::path::{Component, Path};

pub(crate) fn relative_importer_starts_with_package_dir(
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
