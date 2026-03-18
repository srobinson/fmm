use super::RustParser;
use crate::parser::ExportEntry;
use crate::parser::builtin::query_helpers::extract_field_text;
use tree_sitter::Node;

impl RustParser {
    /// Extract `#[macro_export]` declarative macros and proc-macro function symbols.
    ///
    /// Attributes are preceding siblings in the AST, so pure tree-sitter queries cannot
    /// express the relationship. We walk root children sequentially, accumulating
    /// attribute_item nodes, then act when we see a macro_definition or function_item.
    pub(super) fn extract_macro_exports(&self, source: &str, root_node: Node) -> Vec<ExportEntry> {
        let source_bytes = source.as_bytes();
        let mut results = Vec::new();
        let mut pending_attrs: Vec<Node> = Vec::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "attribute_item" => {
                    pending_attrs.push(child);
                }
                "macro_definition" => {
                    if self.attrs_contain(source_bytes, &pending_attrs, "macro_export")
                        && let Some(name) = extract_field_text(&child, source_bytes, "name")
                    {
                        let start_line = pending_attrs
                            .first()
                            .map(|a| a.start_position().row + 1)
                            .unwrap_or(child.start_position().row + 1);
                        let end_line = child.end_position().row + 1;
                        results.push(ExportEntry::new(format!("{}!", name), start_line, end_line));
                    }
                    pending_attrs.clear();
                }
                "function_item" => {
                    if let Some(entry) =
                        self.check_proc_macro_attr(source_bytes, &pending_attrs, child)
                    {
                        results.push(entry);
                    }
                    pending_attrs.clear();
                }
                // Comments are transparent -- don't break the attribute chain
                "line_comment" | "block_comment" => {}
                _ => {
                    pending_attrs.clear();
                }
            }
        }

        results
    }

    /// Check if any of the given attribute_item nodes contain an attribute with `name`.
    fn attrs_contain(&self, source_bytes: &[u8], attrs: &[Node], name: &str) -> bool {
        attrs
            .iter()
            .any(|attr| self.attr_item_has_name(source_bytes, *attr, name))
    }

    /// Return true if the attribute_item node has an attribute whose leading identifier is `name`.
    fn attr_item_has_name(&self, source_bytes: &[u8], attr_item: Node, name: &str) -> bool {
        let mut cursor = attr_item.walk();
        for child in attr_item.children(&mut cursor) {
            if child.kind() == "attribute" {
                let mut ac = child.walk();
                for attr_child in child.children(&mut ac) {
                    if attr_child.kind() == "identifier" {
                        return attr_child
                            .utf8_text(source_bytes)
                            .map(|t| t == name)
                            .unwrap_or(false);
                    }
                    // Stop at first meaningful child
                    if attr_child.is_named() {
                        break;
                    }
                }
            }
        }
        false
    }

    /// If `func_node` is preceded by a proc-macro attribute, return the appropriate ExportEntry.
    fn check_proc_macro_attr(
        &self,
        source_bytes: &[u8],
        attrs: &[Node],
        func_node: Node,
    ) -> Option<ExportEntry> {
        let start_line = attrs
            .first()
            .map(|a| a.start_position().row + 1)
            .unwrap_or(func_node.start_position().row + 1);
        let end_line = func_node.end_position().row + 1;

        for attr in attrs {
            if self.attr_item_has_name(source_bytes, *attr, "proc_macro_derive") {
                // Extract derive name from token_tree argument: #[proc_macro_derive(MyDerive)]
                if let Some(name) = self.extract_first_token_in_attr(source_bytes, *attr) {
                    return Some(ExportEntry::new(name, start_line, end_line));
                }
            }
            if self.attr_item_has_name(source_bytes, *attr, "proc_macro_attribute")
                || self.attr_item_has_name(source_bytes, *attr, "proc_macro")
            {
                // Use the function name directly
                if let Some(name) = extract_field_text(&func_node, source_bytes, "name") {
                    return Some(ExportEntry::new(name, start_line, end_line));
                }
            }
        }
        None
    }

    /// Extract the first identifier inside the token_tree of an attribute.
    ///
    /// For `#[proc_macro_derive(MyDerive, attributes(field))]` this returns `MyDerive`.
    fn extract_first_token_in_attr(&self, source_bytes: &[u8], attr_item: Node) -> Option<String> {
        let mut cursor = attr_item.walk();
        for child in attr_item.children(&mut cursor) {
            if child.kind() == "attribute" {
                let mut ac = child.walk();
                for attr_child in child.children(&mut ac) {
                    if attr_child.kind() == "token_tree" {
                        let mut tc = attr_child.walk();
                        for token in attr_child.children(&mut tc) {
                            if token.kind() == "identifier"
                                && let Ok(name) = token.utf8_text(source_bytes)
                            {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}
