use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

use fmm_core::config::Config;

/// Returns `(kept_files, skipped_count)`.
///
/// Files exceeding `config.max_lines` are excluded from `kept_files` and
/// counted in `skipped_count`. The caller is responsible for reporting skips.
pub(crate) fn collect_files(path: &str, config: &Config) -> Result<(Vec<PathBuf>, usize)> {
    let path = Path::new(path);

    if path.is_file() {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if config.max_lines > 0 && !file_within_line_limit(&canonical, config.max_lines) {
            return Ok((vec![], 1));
        }
        return Ok((vec![canonical], 0));
    }

    // Pre-compile exclude glob patterns (relative to the walk root).
    let exclude_patterns: Vec<glob::Pattern> = config
        .exclude
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let candidates: Vec<PathBuf> = WalkBuilder::new(path)
        .standard_filters(true)
        .add_custom_ignore_filename(".fmmignore")
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| {
            if let Some(ext) = entry.path().extension() {
                config.is_supported_language(ext.to_str().unwrap_or(""))
            } else {
                false
            }
        })
        .filter(|entry| {
            if exclude_patterns.is_empty() {
                return true;
            }
            // Normalize the entry path: strip leading "./" so patterns like
            // "vendor/**" match both "vendor/foo.js" and "./vendor/foo.js".
            let raw = entry.path().to_string_lossy();
            let rel = raw.strip_prefix("./").unwrap_or(&raw);
            !exclude_patterns.iter().any(|pat| {
                pat.matches_with(
                    rel,
                    glob::MatchOptions {
                        require_literal_separator: false,
                        ..Default::default()
                    },
                )
            })
        })
        .map(|entry| {
            entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf())
        })
        .collect();

    if config.max_lines == 0 {
        return Ok((candidates, 0));
    }

    let (files, skipped): (Vec<PathBuf>, Vec<PathBuf>) = candidates
        .into_iter()
        .partition(|canonical| file_within_line_limit(canonical, config.max_lines));
    let skip_count = skipped.len();
    Ok((files, skip_count))
}

/// Returns true if the file has at most `max_lines` lines.
///
/// Uses a byte-size lower bound to avoid reading most files: any file with
/// fewer bytes than `max_lines` cannot possibly have more than `max_lines`
/// lines (every line needs at least one byte for the newline character).
/// Files that exceed this threshold are read as raw bytes and their newlines
/// counted -- no UTF-8 decoding required.
fn file_within_line_limit(path: &Path, max_lines: usize) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return true; // cannot stat: include and let the parser decide
    };
    // Fast path: if file_bytes <= max_lines, it cannot have more than max_lines
    // lines regardless of content.
    if meta.len() <= max_lines as u64 {
        return true;
    }
    // Slow path: count newlines by reading raw bytes (no UTF-8 overhead).
    let Ok(bytes) = std::fs::read(path) else {
        return true; // cannot read: include and let the parser decide
    };
    let line_count = bytecount_newlines(&bytes);
    line_count <= max_lines
}

/// Count lines in a byte slice by counting `\n` characters.
///
/// Each `\n` terminates exactly one line. A file with content but no trailing
/// newline has one more line than it has newlines (the unterminated final line).
///
/// Examples: `""` -> 0, `"hello"` -> 1, `"a\nb"` -> 2, `"a\nb\n"` -> 2.
#[inline]
fn bytecount_newlines(bytes: &[u8]) -> usize {
    let newlines = bytes.iter().filter(|&&b| b == b'\n').count();
    if bytes.is_empty() {
        0
    } else if bytes.last() == Some(&b'\n') {
        // Every newline terminates exactly one line.
        newlines
    } else {
        // Final line has no terminating newline -- add 1.
        newlines + 1
    }
}

pub(crate) fn collect_files_multi(
    paths: &[String],
    config: &Config,
) -> Result<(Vec<PathBuf>, usize)> {
    let mut all_files = Vec::new();
    let mut total_skipped = 0usize;
    for path in paths {
        let (files, skipped) = collect_files(path, config)?;
        all_files.extend(files);
        total_skipped += skipped;
    }
    all_files.sort();
    all_files.dedup();
    Ok((all_files, total_skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn collect_files_returns_canonical_paths() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("app.ts"), "export const a = 1;").unwrap();
        std::fs::write(src.join("util.ts"), "export const b = 2;").unwrap();

        let config = Config::default();
        let (files, _) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();

        assert!(!files.is_empty());
        for file in &files {
            assert!(file.is_absolute(), "path should be absolute: {:?}", file);
        }
    }

    #[test]
    fn collect_files_single_file_is_canonical() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("index.ts");
        std::fs::write(&file_path, "export function main() {}").unwrap();

        let config = Config::default();
        let (files, _) = collect_files(file_path.to_str().unwrap(), &config).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].is_absolute());
        assert_eq!(files[0], file_path.canonicalize().unwrap());
    }

    // bytecount_newlines unit tests

    #[test]
    fn bytecount_newlines_empty() {
        assert_eq!(bytecount_newlines(b""), 0);
    }

    #[test]
    fn bytecount_newlines_single_line_no_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello"), 1);
    }

    #[test]
    fn bytecount_newlines_single_line_with_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello\n"), 1);
    }

    #[test]
    fn bytecount_newlines_two_lines_with_trailing_newline() {
        assert_eq!(bytecount_newlines(b"hello\nworld\n"), 2);
    }

    #[test]
    fn bytecount_newlines_two_lines_no_trailing_newline() {
        // This was the previously failing case: 1 newline -> max(1,1) = 1, wrong.
        assert_eq!(bytecount_newlines(b"hello\nworld"), 2);
    }

    #[test]
    fn bytecount_newlines_only_newlines() {
        assert_eq!(bytecount_newlines(b"\n\n\n"), 3);
    }

    // collect_files respects max_lines

    #[test]
    fn collect_files_excludes_file_exceeding_max_lines() {
        let tmp = TempDir::new().unwrap();
        let big: String = (0..5).map(|i| format!("line{}\n", i)).collect();
        std::fs::write(tmp.path().join("big.ts"), &big).unwrap();
        std::fs::write(tmp.path().join("small.ts"), "export const x = 1;\n").unwrap();

        let config = Config {
            max_lines: 3,
            ..Default::default()
        }; // big.ts has 5 lines, small.ts has 1

        let (files, skipped) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();
        assert_eq!(files.len(), 1, "only small.ts should be collected");
        assert_eq!(skipped, 1, "big.ts should be counted as skipped");
        assert!(files[0].to_string_lossy().contains("small.ts"));
    }

    #[test]
    fn collect_files_max_lines_zero_disables_limit() {
        let tmp = TempDir::new().unwrap();
        let big: String = (0..200).map(|i| format!("line{}\n", i)).collect();
        std::fs::write(tmp.path().join("big.ts"), &big).unwrap();

        let config = Config {
            max_lines: 0,
            ..Default::default()
        }; // 0 disables the limit

        let (files, skipped) = collect_files(tmp.path().to_str().unwrap(), &config).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(skipped, 0);
    }
}
