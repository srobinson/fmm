use crate::manifest::Manifest;
use crate::parser::{ExportEntry, Metadata};

pub(super) fn manifest_with(files: Vec<(&str, Vec<&str>)>) -> Manifest {
    let mut manifest = Manifest::new();
    for (path, deps) in files {
        manifest.add_file(
            path,
            Metadata {
                exports: vec![ExportEntry::new(path.to_string(), 1, 1)],
                imports: vec![],
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                loc: 10,
                ..Default::default()
            },
        );
    }
    manifest.rebuild_reverse_deps();
    manifest
}

pub(super) fn manifest_with_graph_only(files: Vec<(&str, Vec<&str>)>) -> Manifest {
    let mut manifest = Manifest::new();
    for (path, deps) in files {
        manifest.add_file(
            path,
            Metadata {
                exports: vec![ExportEntry::new(path.to_string(), 1, 1)],
                imports: vec![],
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                loc: 10,
                ..Default::default()
            },
        );
    }
    manifest
}

pub(super) fn manifest_with_imports(files: Vec<(&str, Vec<&str>, Vec<&str>)>) -> Manifest {
    let mut manifest = Manifest::new();
    for (path, deps, imports) in files {
        manifest.add_file(
            path,
            Metadata {
                exports: vec![ExportEntry::new(path.to_string(), 1, 1)],
                imports: imports.iter().map(|s| s.to_string()).collect(),
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                loc: 10,
                ..Default::default()
            },
        );
    }
    manifest.rebuild_reverse_deps();
    manifest
}
