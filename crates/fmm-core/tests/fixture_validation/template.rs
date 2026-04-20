/// Verifies that the language template remains a structurally complete contributor
/// reference without compiling it.
#[test]
fn template_rs_is_structurally_complete() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/parser/builtin/template.rs"
    ));

    assert!(
        src.contains("pub struct TemplateParser"),
        "template.rs must define TemplateParser"
    );
    assert!(
        src.contains("pub fn new()"),
        "template.rs must have a new() constructor"
    );
    assert!(
        src.contains("impl Parser for TemplateParser"),
        "template.rs must impl the Parser trait"
    );
    assert!(
        src.contains("fn language_id"),
        "template.rs must implement language_id()"
    );
    assert!(
        src.contains("fn extensions"),
        "template.rs must implement extensions()"
    );
    assert!(
        src.contains("fn parse("),
        "template.rs must implement parse()"
    );
    assert!(
        src.contains("fn extract_exports"),
        "template.rs must have an extract_exports helper"
    );
    assert!(
        src.contains("fn extract_imports"),
        "template.rs must have an extract_imports helper"
    );
}
