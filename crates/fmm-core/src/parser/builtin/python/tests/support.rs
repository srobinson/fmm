use super::super::PythonParser;
use crate::parser::{ExportEntry, ParseResult, Parser};

pub(super) fn parse(source: &str) -> ParseResult {
    let mut parser = PythonParser::new().unwrap();
    parser.parse(source).unwrap()
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
