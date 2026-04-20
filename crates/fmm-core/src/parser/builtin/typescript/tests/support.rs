use std::collections::HashMap;

use super::super::TypeScriptParser;
use crate::parser::{ParseResult, Parser};

pub(super) fn parse(source: &str) -> ParseResult {
    let mut parser = TypeScriptParser::new().unwrap();
    parser.parse(source).unwrap()
}

pub(super) fn parse_tsx(source: &str) -> ParseResult {
    let mut parser = TypeScriptParser::new_tsx().unwrap();
    parser.parse(source).unwrap()
}

pub(super) fn parse_with_aliases(
    source: &str,
    aliases: HashMap<String, Vec<String>>,
) -> ParseResult {
    let mut parser = TypeScriptParser::new().unwrap();
    parser.parse_with_aliases(source, &aliases).unwrap()
}

pub(super) fn parser() -> TypeScriptParser {
    TypeScriptParser::new().unwrap()
}

pub(super) fn tsx_parser() -> TypeScriptParser {
    TypeScriptParser::new_tsx().unwrap()
}
