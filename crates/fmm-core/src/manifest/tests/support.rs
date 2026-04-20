use crate::parser::ExportEntry;

pub(super) fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
    ExportEntry::new(name.to_string(), start, end)
}
