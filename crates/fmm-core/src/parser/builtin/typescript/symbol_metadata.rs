use crate::parser::builtin::symbol_metadata as shared_metadata;
use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};
use tree_sitter::Node;

pub(super) fn ts_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
) -> ExportEntry {
    shared_metadata::export_entry(
        name,
        node,
        source_bytes,
        visibility,
        declaration_kind,
        signature_end_byte,
    )
}

pub(super) fn ts_method_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
) -> ExportEntry {
    shared_metadata::method_entry(
        name,
        node,
        source_bytes,
        parent_class,
        visibility_for_member(node, source_bytes),
        DeclarationKind::Method,
        signature_end_byte,
    )
}

pub(super) fn ts_field_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
) -> ExportEntry {
    shared_metadata::method_entry(
        name,
        node,
        source_bytes,
        parent_class,
        visibility_for_member(node, source_bytes),
        DeclarationKind::Field,
        signature_end_byte,
    )
}

pub(super) fn visibility_for_member(node: Node, source_bytes: &[u8]) -> SymbolVisibility {
    if node
        .child_by_field_name("name")
        .is_some_and(|name| name.kind() == "private_property_identifier")
    {
        return SymbolVisibility::Private;
    }

    match accessibility_text(node, source_bytes).as_deref() {
        Some("private") => SymbolVisibility::Private,
        Some("protected") => SymbolVisibility::Protected,
        _ => SymbolVisibility::Public,
    }
}

pub(super) fn apply_outline_metadata(
    entry: &mut ExportEntry,
    node: Node,
    source_bytes: &[u8],
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
) {
    shared_metadata::apply_outline_metadata(
        entry,
        node,
        source_bytes,
        visibility,
        declaration_kind,
        signature_end_byte,
    );
}

pub(super) fn collect_test_blocks(node: Node, source_bytes: &[u8], entries: &mut Vec<ExportEntry>) {
    if node.kind() == "call_expression"
        && let Some(name) = test_call_name(node, source_bytes)
    {
        entries.push(ts_entry(
            name,
            node,
            source_bytes,
            SymbolVisibility::NonExported,
            DeclarationKind::Test,
        ));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_test_blocks(child, source_bytes, entries);
    }
}

fn accessibility_text(node: Node, source_bytes: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "accessibility_modifier")
        .and_then(|child| child.utf8_text(source_bytes).ok())
        .map(str::to_string)
}

fn signature_end_byte(node: Node) -> Option<usize> {
    match node.kind() {
        "function_declaration"
        | "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "method_definition"
        | "internal_module"
        | "module" => node
            .child_by_field_name("body")
            .map(|body| body.start_byte()),
        "export_statement" => export_statement_signature_end_byte(node),
        "call_expression" => call_signature_end_byte(node),
        _ => None,
    }
}

fn export_statement_signature_end_byte(node: Node) -> Option<usize> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find_map(signature_end_byte)
}

fn call_signature_end_byte(node: Node) -> Option<usize> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "arguments")
        .map(|arguments| arguments.start_byte())
}

fn test_call_name(node: Node, source_bytes: &[u8]) -> Option<String> {
    let function = node.child_by_field_name("function")?;
    let name = function.utf8_text(source_bytes).ok()?;
    if !matches!(name, "describe" | "it" | "test") {
        return None;
    }

    let label = test_call_label(node, source_bytes);
    Some(match label {
        Some(label) => format!("{name} {label}"),
        None => name.to_string(),
    })
}

fn test_call_label(node: Node, source_bytes: &[u8]) -> Option<String> {
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    arguments
        .children(&mut cursor)
        .find(|child| child.kind() == "string")
        .and_then(|child| child.utf8_text(source_bytes).ok())
        .map(|text| text.trim_matches(['"', '\'', '`']).to_string())
        .filter(|text| !text.is_empty())
}
