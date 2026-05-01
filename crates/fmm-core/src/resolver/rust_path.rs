use std::path::{Component, Path};

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
