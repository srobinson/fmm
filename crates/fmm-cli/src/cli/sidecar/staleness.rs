use anyhow::Result;
use fmm_core::identity::{Fingerprint, PARSER_CACHE_VERSION};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;

use crate::fs_utils;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StalenessDecision {
    UpToDate,
    RefreshFingerprint(Fingerprint),
    Reparse(Option<Fingerprint>),
}

pub(crate) struct StaleFile<'a> {
    pub(crate) path: &'a PathBuf,
    pub(crate) fingerprint: Option<Fingerprint>,
}

pub(crate) struct FingerprintRefresh {
    pub(crate) rel_path: String,
    pub(crate) fingerprint: Fingerprint,
}

pub(crate) struct StalenessScan<'a> {
    pub(crate) dirty_files: Vec<StaleFile<'a>>,
    pub(crate) fingerprint_refreshes: Vec<FingerprintRefresh>,
    pub(crate) removed_paths: Vec<String>,
    pub(crate) elapsed: Duration,
}

pub(crate) fn dry_run_dirty_files<'a>(
    files: &'a [PathBuf],
    root: &Path,
    force: bool,
) -> Vec<&'a PathBuf> {
    match SqliteStore::open(root) {
        Ok(store) => {
            let cached = if !force {
                store.load_fingerprints().unwrap_or_default()
            } else {
                HashMap::new()
            };
            files
                .iter()
                .filter(|file| {
                    matches!(
                        decide_file(file, root, &cached, force),
                        Ok(StalenessDecision::Reparse(_))
                    )
                })
                .collect()
        }
        _ => files.iter().collect(),
    }
}

pub(crate) fn stale_files<'a>(
    files: &'a [PathBuf],
    root: &Path,
    store: &SqliteStore,
    force: bool,
) -> Result<StalenessScan<'a>> {
    let start = Instant::now();
    let cached: HashMap<String, Fingerprint> = if !force {
        store.load_fingerprints()?
    } else {
        HashMap::new()
    };
    let current_paths: HashSet<String> = files.iter().map(|file| rel_path(file, root)).collect();
    let removed_paths = cached
        .keys()
        .filter(|path| !current_paths.contains(*path))
        .cloned()
        .collect();

    let decisions: Vec<_> = files
        .par_iter()
        .map(|file| {
            let decision = decide_file(file, root, &cached, force);
            (file, decision)
        })
        .collect();

    let mut dirty_files = Vec::new();
    let mut refreshes = Vec::new();
    for (file, decision) in decisions {
        match decision? {
            StalenessDecision::UpToDate => {}
            StalenessDecision::RefreshFingerprint(fingerprint) => {
                refreshes.push(FingerprintRefresh {
                    rel_path: rel_path(file, root),
                    fingerprint,
                });
            }
            StalenessDecision::Reparse(fingerprint) => {
                dirty_files.push(StaleFile {
                    path: file,
                    fingerprint,
                });
            }
        }
    }

    Ok(StalenessScan {
        dirty_files,
        fingerprint_refreshes: refreshes,
        removed_paths,
        elapsed: start.elapsed(),
    })
}

pub(crate) fn decide_file(
    file: &Path,
    root: &Path,
    cached: &HashMap<String, Fingerprint>,
    force: bool,
) -> Result<StalenessDecision> {
    if force {
        return Ok(StalenessDecision::Reparse(source_fingerprint(file).ok()));
    }

    let rel = rel_path(file, root);
    let Some(stored) = cached.get(&rel) else {
        return Ok(StalenessDecision::Reparse(source_fingerprint(file).ok()));
    };
    let Ok(metadata) = std::fs::metadata(file) else {
        return Ok(StalenessDecision::Reparse(None));
    };
    let Some(source_mtime) = fs_utils::metadata_mtime_rfc3339(&metadata) else {
        return Ok(StalenessDecision::Reparse(source_fingerprint(file).ok()));
    };
    let source_size = metadata.len();

    if stored.source_mtime == source_mtime
        && stored.source_size == source_size
        && stored.parser_cache_version == PARSER_CACHE_VERSION
    {
        return Ok(StalenessDecision::UpToDate);
    }

    let Ok(current) = source_fingerprint(file) else {
        return Ok(StalenessDecision::Reparse(None));
    };
    if current.content_hash == stored.content_hash
        && current.parser_cache_version == stored.parser_cache_version
    {
        return Ok(StalenessDecision::RefreshFingerprint(current));
    }

    Ok(StalenessDecision::Reparse(Some(current)))
}

pub(crate) fn source_fingerprint(path: &Path) -> Result<Fingerprint> {
    let metadata = std::fs::metadata(path)?;
    let source_mtime = fs_utils::metadata_mtime_rfc3339(&metadata)
        .ok_or_else(|| anyhow::anyhow!("failed to read source mtime for {}", path.display()))?;
    let bytes = std::fs::read(path)?;

    Ok(Fingerprint {
        source_mtime,
        source_size: metadata.len(),
        content_hash: content_hash(&bytes),
        parser_cache_version: PARSER_CACHE_VERSION,
    })
}

fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Fnv1a64::default();
    hasher.write(bytes);
    format!("fnv1a64:{:016x}", hasher.finish())
}

struct Fnv1a64(u64);

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

fn rel_path(file: &Path, root: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmm_core::identity::{Fingerprint, PARSER_CACHE_VERSION};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn fingerprint_for(path: &Path) -> Fingerprint {
        source_fingerprint(path).unwrap()
    }

    fn write_source(root: &Path, rel: &str, source: &str) -> PathBuf {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn metadata_match_uses_fast_path_even_when_cached_hash_differs() {
        let tmp = TempDir::new().unwrap();
        let file = write_source(tmp.path(), "src/app.ts", "export const value = 1;\n");
        let mut cached = HashMap::new();
        let mut fingerprint = fingerprint_for(&file);
        fingerprint.content_hash = "fnv1a64:not-read-on-fast-path".to_string();
        cached.insert("src/app.ts".to_string(), fingerprint);

        let decision = decide_file(&file, tmp.path(), &cached, false).unwrap();

        assert!(matches!(decision, StalenessDecision::UpToDate));
    }

    #[test]
    fn changed_mtime_with_identical_content_refreshes_fingerprint_without_parse() {
        let tmp = TempDir::new().unwrap();
        let file = write_source(tmp.path(), "src/app.ts", "export const value = 1;\n");
        let mut old = fingerprint_for(&file);
        old.source_mtime = "2000-01-01T00:00:00+00:00".to_string();
        let mut cached = HashMap::new();
        cached.insert("src/app.ts".to_string(), old);

        let decision = decide_file(&file, tmp.path(), &cached, false).unwrap();

        match decision {
            StalenessDecision::RefreshFingerprint(fingerprint) => {
                assert_eq!(fingerprint.source_size, 24);
                assert_eq!(fingerprint.parser_cache_version, PARSER_CACHE_VERSION);
            }
            other => panic!("expected fingerprint refresh, got {other:?}"),
        }
    }

    #[test]
    fn changed_content_reparses() {
        let tmp = TempDir::new().unwrap();
        let file = write_source(tmp.path(), "src/app.ts", "export const value = 1;\n");
        let mut cached = HashMap::new();
        let mut old = fingerprint_for(&file);
        old.source_size = 1;
        old.content_hash = "fnv1a64:old-content".to_string();
        cached.insert("src/app.ts".to_string(), old);

        let decision = decide_file(&file, tmp.path(), &cached, false).unwrap();

        assert!(matches!(decision, StalenessDecision::Reparse(_)));
    }

    #[test]
    fn missing_fingerprint_row_reparses() {
        let tmp = TempDir::new().unwrap();
        let file = write_source(tmp.path(), "src/app.ts", "export const value = 1;\n");

        let decision = decide_file(&file, tmp.path(), &HashMap::new(), false).unwrap();

        assert!(matches!(decision, StalenessDecision::Reparse(_)));
    }
}
