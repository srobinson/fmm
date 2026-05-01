use std::mem::size_of;

use fmm_core::identity::{EdgeKind, FileId, Fingerprint, PARSER_CACHE_VERSION, normalize_relative};
use tempfile::TempDir;

#[test]
fn file_id_is_four_bytes() {
    assert_eq!(size_of::<FileId>(), 4);
}

#[test]
fn normalizes_existing_file_to_slash_separated_relative_path() {
    let tmp = TempDir::new().unwrap();
    let source_dir = tmp.path().join("src").join("nested");
    std::fs::create_dir_all(&source_dir).unwrap();
    let file = source_dir.join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let relative = normalize_relative(tmp.path(), &file).unwrap();

    assert_eq!(relative.as_str(), "src/nested/main.rs");
}

#[test]
fn rejects_paths_outside_root() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let file = outside.path().join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let result = normalize_relative(root.path(), &file);

    assert!(result.is_err());
}

#[test]
fn fingerprint_carries_cache_identity_fields() {
    let fingerprint = Fingerprint {
        source_mtime: "2026-04-30T15:30:00Z".to_string(),
        source_size: 128,
        content_hash: "xxh3:abc123".to_string(),
        parser_cache_version: PARSER_CACHE_VERSION,
    };

    assert_eq!(fingerprint.source_mtime, "2026-04-30T15:30:00Z");
    assert_eq!(fingerprint.source_size, 128);
    assert_eq!(fingerprint.content_hash, "xxh3:abc123");
    assert_eq!(fingerprint.parser_cache_version, PARSER_CACHE_VERSION);
}

#[test]
fn edge_kind_distinguishes_runtime_from_type_only_edges() {
    assert_ne!(EdgeKind::Runtime, EdgeKind::TypeOnly);
}
