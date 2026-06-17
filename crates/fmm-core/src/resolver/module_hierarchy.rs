use std::path::{Path, PathBuf};

use crate::convention::builtin_convention_registry;

pub(crate) fn is_direct_module_hierarchy_relative(a: &str, b: &str) -> bool {
    a != b && (is_module_parent(a, b) || is_module_parent(b, a))
}

fn is_module_parent(parent: &str, child: &str) -> bool {
    let Some(parent_base) = child_base(Path::new(parent)) else {
        return false;
    };
    let Some(child_parent) =
        module_path(Path::new(child)).and_then(|path| path.parent().map(Path::to_path_buf))
    else {
        return false;
    };

    parent_base == child_parent
}

fn child_base(path: &Path) -> Option<PathBuf> {
    if is_module_hub(path) {
        path.parent().map(Path::to_path_buf)
    } else {
        module_path(path)
    }
}

fn module_path(path: &Path) -> Option<PathBuf> {
    if !is_source_file(path) {
        return None;
    }

    if is_module_hub(path) {
        path.parent().map(Path::to_path_buf)
    } else {
        Some(path.parent()?.join(path.file_stem()?.to_str()?))
    }
}

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            builtin_convention_registry()
                .source_extensions()
                .contains(ext)
        })
}

fn is_module_hub(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            builtin_convention_registry().is_reexport_file(name)
                || matches!(name, "lib.rs" | "main.rs")
        })
}
