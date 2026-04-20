use std::path::Path;

use super::super::RustParser;
use crate::parser::{ExportEntry, ParseResult, Parser};

pub(super) fn parse(source: &str) -> ParseResult {
    let mut parser = RustParser::new().unwrap();
    parser.parse(source).unwrap()
}

pub(super) fn parse_file(source: &str, path: &str) -> ParseResult {
    let mut parser = RustParser::new().unwrap();
    parser.parse_file(source, Path::new(path)).unwrap()
}

pub(super) fn get_method<'a>(
    exports: &'a [ExportEntry],
    class: &str,
    method: &str,
) -> Option<&'a ExportEntry> {
    exports
        .iter()
        .find(|e| e.parent_class.as_deref() == Some(class) && e.name == method)
}
