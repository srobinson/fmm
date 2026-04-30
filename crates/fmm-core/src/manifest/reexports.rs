use std::collections::HashSet;

use super::Manifest;

/// A re-export surfaced from another module, resolved to its origin definition.
///
/// Produced by [`Manifest::reexports_in_file`] and rendered by
/// `format_file_outline` into a separate `re-exports:` section so agents can
/// distinguish surface re-exports from local definitions at a glance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineReExport {
    pub name: String,
    pub origin_file: String,
    pub origin_start: usize,
    pub origin_end: usize,
}

impl Manifest {
    /// Return the re-exports surfaced by `file`, each resolved to its origin
    /// definition. A re-export is an exported name whose string also appears
    /// as a value in the file's `named_imports` map (i.e. imported by name
    /// from another module and re-surfaced in this file's public API).
    ///
    /// Aliased imports like `from X import A as B` are NOT re-exports:
    /// `named_imports` stores the original name `A`, while the file exports
    /// the local alias `B`. The name lookup therefore treats `B` as a local
    /// definition, matching the Phase 2 shadow-silencing logic.
    ///
    /// Origin resolution:
    /// 1. `export_locations[name]` with a valid (non-self, lines.start > 0)
    ///    entry — first choice.
    /// 2. Fallback to `(file, import_line, import_line)` using the
    ///    re-exporter's own `export_lines[i]` when the origin is not in the
    ///    index (e.g. imported from a third-party package outside the
    ///    workspace). The entry is still actionable — agents can jump to
    ///    the import line to see where it comes from.
    ///
    /// Results are sorted alphabetically by name for stable output.
    pub fn reexports_in_file(&self, file: &str) -> Vec<OutlineReExport> {
        let Some(entry) = self.files.get(file) else {
            return Vec::new();
        };

        let imported_names: HashSet<&str> = entry
            .named_imports
            .values()
            .flat_map(|v| v.iter().map(String::as_str))
            .collect();

        let mut out = Vec::with_capacity(entry.exports.len());
        for (i, name) in entry.exports.iter().enumerate() {
            if !imported_names.contains(name.as_str()) {
                continue;
            }

            let origin = self
                .export_locations
                .get(name)
                .filter(|loc| loc.file != file)
                .and_then(|loc| {
                    let lines = loc.lines.as_ref()?;
                    if lines.start == 0 {
                        return None;
                    }
                    Some((loc.file.clone(), lines.start, lines.end))
                });

            let (origin_file, origin_start, origin_end) = match origin {
                Some(r) => r,
                None => {
                    let (s, e) = entry
                        .export_lines
                        .as_ref()
                        .and_then(|els| els.get(i))
                        .filter(|el| el.start > 0)
                        .map(|el| (el.start, el.end))
                        .unwrap_or((0, 0));
                    (file.to_string(), s, e)
                }
            };

            out.push(OutlineReExport {
                name: name.clone(),
                origin_file,
                origin_start,
                origin_end,
            });
        }

        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}
