//! Shared YAML-building helpers used by multiple formatter sub-modules.

use crate::formatter::yaml_escape;
use crate::manifest::ExportLines;

/// Append an inline YAML list: `key: [item1, item2]`.
/// No-ops when `items` is empty.
pub(crate) fn push_inline_list(lines: &mut Vec<String>, key: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let escaped: Vec<String> = items.iter().map(|s| yaml_escape(s)).collect();
    lines.push(format!("{}: [{}]", key, escaped.join(", ")));
}

/// Append an `exports:` block with optional line-range annotations.
/// No-ops when `exports` is empty.
pub(crate) fn push_exports_map(
    lines: &mut Vec<String>,
    exports: &[String],
    export_lines: Option<&Vec<ExportLines>>,
) {
    if exports.is_empty() {
        return;
    }
    lines.push("exports:".to_string());
    for (i, name) in exports.iter().enumerate() {
        if let Some(el) = export_lines.and_then(|els| els.get(i)) {
            if el.start > 0 {
                lines.push(format!(
                    "  {}: [{}, {}]",
                    yaml_escape(name),
                    el.start,
                    el.end
                ));
                continue;
            }
        }
        lines.push(format!("  {}", yaml_escape(name)));
    }
}
