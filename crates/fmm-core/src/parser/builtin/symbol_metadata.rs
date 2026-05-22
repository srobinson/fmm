use crate::parser::{DeclarationKind, ExportEntry, SymbolVisibility};
use tree_sitter::Node;

#[derive(Clone, Copy)]
pub(super) struct SignatureSource<'tree> {
    range_node: Node<'tree>,
    signature_node: Node<'tree>,
}

impl<'tree> SignatureSource<'tree> {
    pub(super) fn new(node: Node<'tree>) -> Self {
        Self {
            range_node: node,
            signature_node: node,
        }
    }

    pub(super) fn with_signature_node(
        range_node: Node<'tree>,
        signature_node: Node<'tree>,
    ) -> Self {
        Self {
            range_node,
            signature_node,
        }
    }
}

pub(super) fn export_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) -> ExportEntry {
    export_entry_from_source(
        name,
        SignatureSource::new(node),
        source_bytes,
        visibility,
        declaration_kind,
        signature_end_byte,
    )
}

pub(super) fn export_entry_from_source(
    name: String,
    source: SignatureSource,
    source_bytes: &[u8],
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) -> ExportEntry {
    let mut entry = ExportEntry::new(
        name,
        source.range_node.start_position().row + 1,
        source.range_node.end_position().row + 1,
    );
    apply_outline_metadata(
        &mut entry,
        source.signature_node,
        source_bytes,
        visibility,
        declaration_kind,
        signature_end_byte,
    );
    entry
}

pub(super) fn method_entry(
    name: String,
    node: Node,
    source_bytes: &[u8],
    parent_class: String,
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) -> ExportEntry {
    method_entry_from_source(
        name,
        SignatureSource::new(node),
        source_bytes,
        parent_class,
        visibility,
        declaration_kind,
        signature_end_byte,
    )
}

pub(super) fn method_entry_from_source(
    name: String,
    source: SignatureSource,
    source_bytes: &[u8],
    parent_class: String,
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) -> ExportEntry {
    let mut entry = ExportEntry::method(
        name,
        source.range_node.start_position().row + 1,
        source.range_node.end_position().row + 1,
        parent_class,
    );
    apply_outline_metadata(
        &mut entry,
        source.signature_node,
        source_bytes,
        visibility,
        declaration_kind,
        signature_end_byte,
    );
    entry
}

pub(super) fn apply_outline_metadata(
    entry: &mut ExportEntry,
    node: Node,
    source_bytes: &[u8],
    visibility: SymbolVisibility,
    declaration_kind: DeclarationKind,
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) {
    entry.signature = Some(signature_text(node, source_bytes, signature_end_byte));
    entry.visibility = Some(visibility);
    entry.declaration_kind = Some(declaration_kind);
}

pub(super) fn signature_text(
    node: Node,
    source_bytes: &[u8],
    signature_end_byte: impl Fn(Node) -> Option<usize>,
) -> String {
    let end_byte = signature_end_byte(node).unwrap_or_else(|| node.end_byte());
    let text = std::str::from_utf8(&source_bytes[node.start_byte()..end_byte]).unwrap_or("");
    text.trim()
        .trim_end_matches(':')
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim()
        .to_string()
}

pub(super) fn top_level_ancestor(node: Node) -> Node {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.parent().is_none() {
            return current;
        }
        current = parent;
    }
    current
}
