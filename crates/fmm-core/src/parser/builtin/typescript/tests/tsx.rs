use super::support::{parse_tsx, parser, tsx_parser};
use crate::parser::Parser;

#[test]
fn tsx_jsx_parsed_with_tsx_grammar() {
    let source = r#"
export function Button({ label }: { label: string }) {
return <button>{label}</button>;
}
"#;
    let result = parse_tsx(source);
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Button".to_string())
    );
}

#[test]
fn tsx_jsx_arrow_component() {
    let source = r#"
export const Card = ({ title }: { title: string }) => (
<div className="card">
    <h2>{title}</h2>
</div>
);
"#;
    let result = parse_tsx(source);
    assert!(result.metadata.export_names().contains(&"Card".to_string()));
}

#[test]
fn ts_parser_language_id_and_extensions() {
    let parser = parser();
    assert_eq!(Parser::language_id(&parser), "typescript");
    assert_eq!(Parser::extensions(&parser), &["ts", "js"]);
}

#[test]
fn tsx_parser_language_id_and_extensions() {
    let parser = tsx_parser();
    assert_eq!(Parser::language_id(&parser), "tsx");
    assert_eq!(Parser::extensions(&parser), &["tsx", "jsx"]);
}
