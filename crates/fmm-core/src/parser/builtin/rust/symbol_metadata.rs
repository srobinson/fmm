use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};
use tree_sitter::Node;

pub(super) fn rust_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    declaration_kind: DeclarationKind,
) -> ExportEntry {
    let mut entry = ExportEntry::new(
        name,
        node.start_position().row + 1,
        node.end_position().row + 1,
    );
    annotate_entry(&mut entry, node, source_bytes, declaration_kind);
    entry
}

pub(super) fn rust_method_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
) -> ExportEntry {
    let mut entry = ExportEntry::method(
        name,
        node.start_position().row + 1,
        node.end_position().row + 1,
        parent_class,
    );
    annotate_entry(&mut entry, node, source_bytes, DeclarationKind::Method);
    entry
}

pub(super) fn annotate_entry(
    entry: &mut ExportEntry,
    node: Node,
    source_bytes: &[u8],
    declaration_kind: DeclarationKind,
) {
    entry.signature = Some(signature_text(node, source_bytes));
    entry.visibility = Some(visibility_for(node, source_bytes));
    entry.declaration_kind = Some(declaration_kind);
}

pub(super) fn declaration_kind(node: Node, source_bytes: &[u8]) -> Option<DeclarationKind> {
    match node.kind() {
        "function_item" if has_test_attr(node, source_bytes) => Some(DeclarationKind::Test),
        "function_item" => Some(DeclarationKind::Fn),
        "field_declaration" => Some(DeclarationKind::Field),
        "const_item" | "static_item" => Some(DeclarationKind::Const),
        "struct_item" => Some(DeclarationKind::Struct),
        "trait_item" => Some(DeclarationKind::Trait),
        "impl_item" => Some(DeclarationKind::Impl),
        "enum_item" => Some(DeclarationKind::Enum),
        "enum_variant" => Some(DeclarationKind::Variant),
        "mod_item" => Some(DeclarationKind::Module),
        "macro_definition" => Some(DeclarationKind::Macro),
        "type_item" => Some(DeclarationKind::Type),
        _ => None,
    }
}

pub(super) fn visibility_for(node: Node, source_bytes: &[u8]) -> SymbolVisibility {
    match visibility_text(node, source_bytes).as_deref() {
        None => SymbolVisibility::Private,
        Some("pub") if enclosing_modules_are_public(node, source_bytes) => SymbolVisibility::Public,
        Some("pub") => SymbolVisibility::NonExported,
        Some(_) => SymbolVisibility::Crate,
    }
}

pub(super) fn signature_text(node: Node, source_bytes: &[u8]) -> String {
    let end_byte = signature_end_byte(node).unwrap_or_else(|| node.end_byte());
    let text = std::str::from_utf8(&source_bytes[node.start_byte()..end_byte]).unwrap_or("");
    text.trim()
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim()
        .to_string()
}

pub(super) fn normalize_macro_name(name: &str) -> &str {
    name.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
}

fn visibility_text(node: Node, source_bytes: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .and_then(|child| child.utf8_text(source_bytes).ok())
        .map(str::to_string)
}

fn signature_end_byte(node: Node) -> Option<usize> {
    node.child_by_field_name("body")
        .map(|body| body.start_byte())
        .or_else(|| {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|child| is_rust_body_node(child.kind()))
                .map(|child| child.start_byte())
        })
}

fn is_rust_body_node(kind: &str) -> bool {
    matches!(
        kind,
        "declaration_list"
            | "field_declaration_list"
            | "ordered_field_declaration_list"
            | "enum_variant_list"
            | "trait_item_list"
    )
}

fn enclosing_modules_are_public(node: Node, source_bytes: &[u8]) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "mod_item"
            && visibility_text(parent, source_bytes).as_deref() != Some("pub")
        {
            return false;
        }
        current = parent.parent();
    }
    true
}

fn has_test_attr(node: Node, source_bytes: &[u8]) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let mut previous = node.prev_sibling();
    while let Some(sibling) = previous {
        if sibling.parent() != Some(parent) {
            break;
        }
        match sibling.kind() {
            "attribute_item" if attr_item_has_name(sibling, source_bytes, "test") => return true,
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        previous = sibling.prev_sibling();
    }
    false
}

fn attr_item_has_name(attr_item: Node, source_bytes: &[u8], name: &str) -> bool {
    let mut cursor = attr_item.walk();
    for child in attr_item.children(&mut cursor) {
        if child.kind() != "attribute" {
            continue;
        }
        let mut attr_cursor = child.walk();
        for attr_child in child.children(&mut attr_cursor) {
            if attr_child.kind() == "identifier" {
                return attr_child
                    .utf8_text(source_bytes)
                    .map(|text| text == name)
                    .unwrap_or(false);
            }
            if attr_child.is_named() {
                break;
            }
        }
    }
    false
}
