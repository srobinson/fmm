use fmm_core::identity::Fingerprint;
use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;
use std::collections::HashMap;
use std::path::Path;

use crate::cli::sidecar::staleness::{self, StalenessDecision};

pub(crate) fn outline_freshness(root: &Path, file: &str) -> Option<String> {
    let store = SqliteStore::open(root).ok()?;
    let fingerprints = store.load_fingerprints().ok()?;
    let abs_file = root.join(file);
    let decision = staleness::decide_file(&abs_file, root, &fingerprints, false).ok()?;
    freshness_annotation(file, &fingerprints, decision)
}

fn freshness_annotation(
    file: &str,
    fingerprints: &HashMap<String, Fingerprint>,
    decision: StalenessDecision,
) -> Option<String> {
    match decision {
        StalenessDecision::UpToDate | StalenessDecision::RefreshFingerprint(_) => None,
        StalenessDecision::Reparse(_) => {
            if fingerprints.contains_key(file) {
                Some(format!("{file} is stale; run fmm generate"))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> Fingerprint {
        Fingerprint {
            source_mtime: "2026-05-22T00:00:00+00:00".to_string(),
            source_size: 10,
            content_hash: "fnv1a64:test".to_string(),
            parser_cache_version: 1,
        }
    }

    fn indexed_fingerprints() -> HashMap<String, Fingerprint> {
        HashMap::from([("src/mod.ts".to_string(), fingerprint())])
    }

    #[test]
    fn up_to_date_has_no_freshness_annotation() {
        let annotation = freshness_annotation(
            "src/mod.ts",
            &indexed_fingerprints(),
            StalenessDecision::UpToDate,
        );

        assert_eq!(annotation, None);
    }

    #[test]
    fn fingerprint_refresh_has_no_freshness_annotation() {
        let annotation = freshness_annotation(
            "src/mod.ts",
            &indexed_fingerprints(),
            StalenessDecision::RefreshFingerprint(fingerprint()),
        );

        assert_eq!(annotation, None);
    }

    #[test]
    fn indexed_reparse_has_freshness_annotation() {
        let annotation = freshness_annotation(
            "src/mod.ts",
            &indexed_fingerprints(),
            StalenessDecision::Reparse(Some(fingerprint())),
        );

        assert_eq!(
            annotation,
            Some("src/mod.ts is stale; run fmm generate".to_string())
        );
    }

    #[test]
    fn unindexed_reparse_has_no_freshness_annotation() {
        let annotation = freshness_annotation(
            "src/missing.ts",
            &indexed_fingerprints(),
            StalenessDecision::Reparse(None),
        );

        assert_eq!(annotation, None);
    }
}
