//! Filesystem utility functions.
//!
//! Extracted from db/writer.rs because these are I/O operations,
//! not storage concerns.

use chrono::Utc;
use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

/// Returns the file's last-modified time as an RFC3339 string, or `None`
/// if the metadata cannot be read.
///
/// Includes nanoseconds when the OS provides sub-second precision (APFS, Linux
/// ext4) so that same-second modifications are correctly detected by the
/// staleness check.
pub fn file_mtime_rfc3339(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    metadata_mtime_rfc3339(&meta)
}

pub fn metadata_mtime_rfc3339(meta: &Metadata) -> Option<String> {
    let mtime = meta.modified().ok()?;
    let duration = mtime.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let dt = chrono::DateTime::<Utc>::from_timestamp(
        duration.as_secs() as i64,
        duration.subsec_nanos(),
    )?;
    Some(dt.to_rfc3339())
}
