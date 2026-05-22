use fmm_core::store::FmmStore;
use fmm_store::SqliteStore;
use std::path::Path;

use crate::cli::sidecar::staleness::{self, StalenessDecision};

pub(crate) fn outline_freshness(root: &Path, file: &str) -> Option<String> {
    let store = SqliteStore::open(root).ok()?;
    let fingerprints = store.load_fingerprints().ok()?;
    let abs_file = root.join(file);
    match staleness::decide_file(&abs_file, root, &fingerprints, false).ok()? {
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
