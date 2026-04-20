use fmm_core::parser::{ParseResult, Parser};

pub fn parse_fixture<P: Parser>(mut parser: P, source: &str) -> ParseResult {
    parser.parse(source).unwrap()
}

pub fn assert_exports_sorted(result: &ParseResult) {
    let lines: Vec<usize> = result
        .metadata
        .exports
        .iter()
        .map(|entry| entry.start_line)
        .collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);
}
