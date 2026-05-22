use crate::parser::builtin::symbol_metadata as shared_metadata;
use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};
use std::collections::HashSet;
use tree_sitter::Node;

pub(super) fn python_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    explicit_public_names: &HashSet<String>,
) -> Option<ExportEntry> {
    let kind = declaration_kind(node, source_bytes)?;
    let signature_node = signature_node(node);
    Some(shared_metadata::export_entry_from_source(
        name.clone(),
        shared_metadata::SignatureSource::with_signature_node(node, signature_node),
        source_bytes,
        visibility_for_name(&name, explicit_public_names),
        kind,
        signature_end_byte,
    ))
}

pub(super) fn python_method_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
) -> ExportEntry {
    let signature_node = signature_node(node);
    shared_metadata::method_entry_from_source(
        name.clone(),
        shared_metadata::SignatureSource::with_signature_node(node, signature_node),
        source_bytes,
        parent_class,
        visibility_for_member_name(&name),
        DeclarationKind::Method,
        signature_end_byte,
    )
}

pub(super) fn python_field_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
) -> ExportEntry {
    let signature_node = signature_node(node);
    shared_metadata::method_entry_from_source(
        name.clone(),
        shared_metadata::SignatureSource::with_signature_node(node, signature_node),
        source_bytes,
        parent_class,
        visibility_for_member_name(&name),
        DeclarationKind::Field,
        signature_end_byte,
    )
}

fn declaration_kind(node: Node, source_bytes: &[u8]) -> Option<DeclarationKind> {
    match node.kind() {
        "function_definition" | "decorated_definition" if is_python_test(node, source_bytes) => {
            Some(DeclarationKind::Test)
        }
        "function_definition" | "decorated_definition"
            if function_node(node).is_some_and(|func| is_python_test(func, source_bytes)) =>
        {
            Some(DeclarationKind::Test)
        }
        "function_definition" | "decorated_definition"
            if function_node(node).is_some() || node.kind() == "function_definition" =>
        {
            Some(DeclarationKind::Fn)
        }
        "class_definition" | "decorated_definition" if class_node(node).is_some() => {
            class_kind(class_node(node).unwrap_or(node), source_bytes)
        }
        "class_definition" => class_kind(node, source_bytes),
        "expression_statement" | "assignment" if is_type_alias(node, source_bytes) => {
            Some(DeclarationKind::Type)
        }
        "expression_statement" | "assignment" if is_const_assignment(node, source_bytes) => {
            Some(DeclarationKind::Const)
        }
        _ => None,
    }
}

fn class_kind(node: Node, source_bytes: &[u8]) -> Option<DeclarationKind> {
    if class_bases_text(node, source_bytes).is_some_and(|text| {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|part| matches!(part, "Protocol" | "ABC"))
    }) {
        Some(DeclarationKind::Trait)
    } else {
        Some(DeclarationKind::Struct)
    }
}

fn visibility_for_name(name: &str, explicit_public_names: &HashSet<String>) -> SymbolVisibility {
    if explicit_public_names.contains(name) {
        SymbolVisibility::Public
    } else if name.starts_with('_') {
        SymbolVisibility::Private
    } else {
        SymbolVisibility::Public
    }
}

fn visibility_for_member_name(name: &str) -> SymbolVisibility {
    if name.starts_with('_') && name != "__init__" {
        SymbolVisibility::Private
    } else {
        SymbolVisibility::Public
    }
}

fn signature_node(node: Node) -> Node {
    function_node(node)
        .or_else(|| class_node(node))
        .unwrap_or(node)
}

fn signature_end_byte(node: Node) -> Option<usize> {
    node.child_by_field_name("body")
        .map(|body| body.start_byte())
}

fn function_node(node: Node) -> Option<Node> {
    if node.kind() == "function_definition" {
        return Some(node);
    }
    child_of_kind(node, "function_definition")
}

fn class_node(node: Node) -> Option<Node> {
    if node.kind() == "class_definition" {
        return Some(node);
    }
    child_of_kind(node, "class_definition")
}

fn child_of_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn class_bases_text(node: Node, source_bytes: &[u8]) -> Option<String> {
    node.child_by_field_name("superclasses")
        .or_else(|| node.child_by_field_name("argument_list"))
        .and_then(|child| child.utf8_text(source_bytes).ok())
        .map(str::to_string)
}

fn is_python_test(node: Node, source_bytes: &[u8]) -> bool {
    let name = function_node(node)
        .and_then(|func| func.child_by_field_name("name"))
        .and_then(|name| name.utf8_text(source_bytes).ok());
    if name.is_some_and(|name| name.starts_with("test_") || name.ends_with("_test")) {
        return true;
    }
    has_decorator(node, source_bytes, "pytest.fixture")
        || has_decorator(node, source_bytes, "fixture")
}

fn has_decorator(node: Node, source_bytes: &[u8], needle: &str) -> bool {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.kind() == "decorated_definition" {
            let mut cursor = candidate.walk();
            return candidate.children(&mut cursor).any(|child| {
                child.kind() == "decorator"
                    && child
                        .utf8_text(source_bytes)
                        .map(|text| text.contains(needle))
                        .unwrap_or(false)
            });
        }
        current = candidate.parent();
    }
    false
}

fn is_type_alias(node: Node, source_bytes: &[u8]) -> bool {
    assignment_text(node, source_bytes).is_some_and(|text| {
        text.contains("TypeAlias")
            || text.trim_start().starts_with("type ")
            || text.contains(": TypeAlias")
    })
}

fn is_const_assignment(node: Node, source_bytes: &[u8]) -> bool {
    let Some(text) = assignment_text(node, source_bytes) else {
        return false;
    };
    let name = assignment_name(node, source_bytes).unwrap_or_default();
    is_upper_snake(&name) || text.contains("Final[") || text.contains(": Final")
}

pub(super) fn assignment_name(node: Node, source_bytes: &[u8]) -> Option<String> {
    let assignment = if node.kind() == "assignment" {
        node
    } else {
        child_of_kind(node, "assignment")?
    };
    assignment
        .child_by_field_name("left")
        .and_then(|left| left.utf8_text(source_bytes).ok())
        .map(str::to_string)
}

fn assignment_text(node: Node, source_bytes: &[u8]) -> Option<String> {
    node.utf8_text(source_bytes).ok().map(str::to_string)
}

fn is_upper_snake(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
}
