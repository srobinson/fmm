mod extract_classes;

#[cfg(test)]
mod tests;

use super::query_helpers::{collect_matches, compile_query, make_parser, top_level_ancestor};
use crate::parser::{ExportEntry, Metadata, ParseResult, Parser};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

/// Convert Python relative import dot-notation to path notation for `dep_matches()`.
/// `.utils` -> `./utils`, `..models` -> `../models`, `.` -> `./`
fn dot_import_to_path(raw: &str) -> String {
    let dots = raw.chars().take_while(|c| *c == '.').count();
    let rest = &raw[dots..];
    let module_path = rest.replace('.', "/");
    if dots <= 1 {
        format!("./{}", module_path)
    } else {
        let ups = "../".repeat(dots - 1);
        format!("{}{}", ups, module_path)
    }
}

pub struct PythonParser {
    parser: TSParser,
    func_query: Query,
    class_query: Query,
    assign_query: Query,
    dunder_all_query: Query,
    import_queries: Vec<Query>,
    from_import_query: Query,
    relative_import_query: Query,
    decorator_query: Query,
    dotted_decorator_query: Query,
    /// ALP-769: finds class declarations for public method extraction
    pub(super) class_method_query: Query,
}

impl PythonParser {
    pub fn new() -> Result<Self> {
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let parser = make_parser(&language, "Python")?;

        let func_query = compile_query(
            &language,
            "[(module (function_definition name: (identifier) @name))
              (module (decorated_definition (function_definition name: (identifier) @name)))]",
            "func",
        )?;
        let class_query = compile_query(
            &language,
            "[(module (class_definition name: (identifier) @name))
              (module (decorated_definition (class_definition name: (identifier) @name)))]",
            "class",
        )?;
        let assign_query = compile_query(
            &language,
            "(module (expression_statement (assignment left: (identifier) @name)))",
            "assign",
        )?;
        let dunder_all_query = compile_query(
            &language,
            "(module (expression_statement (assignment left: (identifier) @name right: (list) @values)))",
            "dunder_all",
        )?;
        let import_queries = vec![
            compile_query(
                &language,
                "(import_statement name: (dotted_name) @name)",
                "import",
            )?,
            compile_query(
                &language,
                "(import_statement name: (aliased_import name: (dotted_name) @name))",
                "aliased import",
            )?,
        ];
        let from_import_query = compile_query(
            &language,
            "(import_from_statement module_name: (dotted_name) @name)",
            "from_import",
        )?;
        let relative_import_query = compile_query(
            &language,
            "(import_from_statement module_name: (relative_import) @name)",
            "relative_import",
        )?;
        let decorator_query =
            compile_query(&language, "(decorator (identifier) @name)", "decorator")?;
        let dotted_decorator_query = compile_query(
            &language,
            "(decorator (attribute) @name)",
            "dotted_decorator",
        )?;
        // ALP-769: find class declarations for public method extraction
        let class_method_query = compile_query(
            &language,
            "(class_definition name: (identifier) @class_name) @class",
            "class_method",
        )?;

        Ok(Self {
            parser,
            func_query,
            class_query,
            assign_query,
            dunder_all_query,
            import_queries,
            from_import_query,
            relative_import_query,
            decorator_query,
            dotted_decorator_query,
            class_method_query,
        })
    }

    fn extract_exports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<ExportEntry> {
        if let Some(all_exports) = self.extract_dunder_all(source, root_node) {
            return all_exports;
        }

        let mut seen = HashSet::new();
        let mut exports = Vec::new();
        let source_bytes = source.as_bytes();

        // fmm is a structural tool, not a Python visibility checker: underscore
        // prefix is social convention, not a structural property. Include all
        // top-level def/class/assign names so re-export dereferencing can find
        // them in origin files (e.g. `_port_in_use` in `net.py`).
        let mut collect_filtered = |query: &Query, filter: fn(&str) -> bool| {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes)
                        && filter(text)
                        && seen.insert(text.to_string())
                    {
                        let decl = top_level_ancestor(capture.node);
                        exports.push(ExportEntry::new(
                            text.to_string(),
                            decl.start_position().row + 1,
                            decl.end_position().row + 1,
                        ));
                    }
                }
            }
        };

        collect_filtered(&self.func_query, |_| true);
        collect_filtered(&self.class_query, |_| true);
        collect_filtered(&self.assign_query, |name| {
            name.chars().all(|c| c.is_uppercase() || c == '_')
                || name.chars().next().is_some_and(|c| c.is_uppercase())
        });

        exports.sort_by_key(|e| e.start_line);
        exports
    }

    /// Build a map of top-level definition names to their line ranges.
    fn build_definition_map(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> HashMap<String, (usize, usize)> {
        let source_bytes = source.as_bytes();
        let mut defs = HashMap::new();

        let mut collect_defs = |query: &Query| {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let decl = top_level_ancestor(capture.node);
                        defs.insert(
                            text.to_string(),
                            (decl.start_position().row + 1, decl.end_position().row + 1),
                        );
                    }
                }
            }
        };

        collect_defs(&self.func_query);
        collect_defs(&self.class_query);
        collect_defs(&self.assign_query);
        defs
    }

    /// Build a map of locally bound import names → line range of the import statement.
    ///
    /// Handles all module-level import forms, keyed by the *local* binding:
    /// - `from X import Y` → `Y`
    /// - `from X import Y as Z` → `Z`
    /// - `import X` → `X`
    /// - `import X.Y.Z` → `X` (first segment, matching Python binding semantics)
    /// - `import X as Y` → `Y`
    ///
    /// Used by `extract_dunder_all` to resolve re-exported names (names in
    /// `__all__` that came from an import, not a local definition) to their
    /// import-statement line range.
    fn build_import_position_map(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> HashMap<String, (usize, usize)> {
        let source_bytes = source.as_bytes();
        let mut imports: HashMap<String, (usize, usize)> = HashMap::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            let range = (child.start_position().row + 1, child.end_position().row + 1);
            match child.kind() {
                "import_from_statement" => {
                    let module_name_node = child.child_by_field_name("module_name");
                    let mut inner = child.walk();
                    for c in child.children(&mut inner) {
                        match c.kind() {
                            "dotted_name" | "identifier" => {
                                // Skip the module_name node itself
                                if module_name_node == Some(c) {
                                    continue;
                                }
                                if let Ok(text) = c.utf8_text(source_bytes) {
                                    imports.insert(text.to_string(), range);
                                }
                            }
                            "aliased_import" => {
                                // `Y as Z` — the local binding is the alias
                                if let Some(alias) = c.child_by_field_name("alias")
                                    && let Ok(text) = alias.utf8_text(source_bytes)
                                {
                                    imports.insert(text.to_string(), range);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "import_statement" => {
                    let mut inner = child.walk();
                    for c in child.children(&mut inner) {
                        match c.kind() {
                            "dotted_name" => {
                                // `import X` or `import X.Y.Z` — local binding is
                                // the first segment of the dotted name.
                                if let Ok(text) = c.utf8_text(source_bytes) {
                                    let first = text.split('.').next().unwrap_or(text);
                                    imports.insert(first.to_string(), range);
                                }
                            }
                            "aliased_import" => {
                                if let Some(alias) = c.child_by_field_name("alias")
                                    && let Ok(text) = alias.utf8_text(source_bytes)
                                {
                                    imports.insert(text.to_string(), range);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        imports
    }

    /// Extract names from `__all__ = [...]` if present, resolving to definition sites.
    ///
    /// Resolution order for each name in `__all__`:
    /// 1. Local top-level definition (function, class, module-level assignment).
    /// 2. Import statement that binds this name (re-export case).
    /// 3. `(0, 0)` sentinel meaning "no position". Downstream code (reader.rs,
    ///    manifest/mod.rs) treats `start == 0` as `None`.
    ///
    /// Never falls back to the `__all__` literal's line range — that's structurally
    /// wrong and caused every re-exported name in a manicure `__init__.py` to
    /// collapse to the same 20-line range.
    fn extract_dunder_all(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> Option<Vec<ExportEntry>> {
        let source_bytes = source.as_bytes();
        let capture_names = self.dunder_all_query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.dunder_all_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            let name_capture = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "name"
            })?;
            let values_capture = m.captures.iter().find(|c| {
                let idx = c.index as usize;
                idx < capture_names.len() && capture_names[idx] == "values"
            })?;

            if name_capture.node.utf8_text(source_bytes).ok()? != "__all__" {
                continue;
            }

            // Build lookup maps to resolve names to their source positions.
            let def_map = self.build_definition_map(source, root_node);
            let import_map = self.build_import_position_map(source, root_node);

            let mut seen = HashSet::new();
            let mut exports = Vec::new();
            let list_node = values_capture.node;
            let mut child_cursor = list_node.walk();
            for child in list_node.children(&mut child_cursor) {
                if child.kind() == "string"
                    && let Ok(text) = child.utf8_text(source_bytes)
                {
                    let name = text.trim_matches('\'').trim_matches('"').to_string();
                    if !name.is_empty() && seen.insert(name.clone()) {
                        let (start, end) = def_map
                            .get(&name)
                            .copied()
                            .or_else(|| import_map.get(&name).copied())
                            .unwrap_or((0, 0));
                        exports.push(ExportEntry::new(name, start, end));
                    }
                }
            }
            exports.sort_by_key(|e| e.start_line);
            return Some(exports);
        }
        None
    }

    fn extract_imports(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut imports = Vec::new();
        let source_bytes = source.as_bytes();

        for query in &self.import_queries {
            let mut cursor = QueryCursor::new();
            let mut iter = cursor.matches(query, root_node, source_bytes);
            while let Some(m) = iter.next() {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(source_bytes) {
                        let full_module = text.to_string();
                        if seen.insert(full_module.clone()) {
                            imports.push(full_module);
                        }
                    }
                }
            }
        }

        // from foo import bar
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.from_import_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes)
                    && !text.starts_with('.')
                {
                    let full_module = text.to_string();
                    if seen.insert(full_module.clone()) {
                        imports.push(full_module);
                    }
                }
            }
        }

        imports.sort();
        imports
    }

    /// Extract named imports and namespace imports from Python import statements.
    ///
    /// Returns `(named_imports, namespace_imports)`:
    /// - `from X import A, B` -> `named_imports["X"] = ["A", "B"]`
    /// - `from X import *` -> `namespace_imports.push("X")`
    /// - `import module` -> `namespace_imports.push("module")`
    /// - `from .pkg import A` -> `named_imports[".pkg"] = ["A"]` (raw dot notation)
    fn extract_named_imports(
        &self,
        source: &str,
        root_node: tree_sitter::Node,
    ) -> (HashMap<String, Vec<String>>, Vec<String>) {
        let source_bytes = source.as_bytes();
        let mut named: HashMap<String, Vec<String>> = HashMap::new();
        let mut namespace: Vec<String> = Vec::new();
        let mut namespace_seen: HashSet<String> = HashSet::new();

        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "import_from_statement" => {
                    // Extract module source: dotted_name or relative_import
                    let module_name = child
                        .child_by_field_name("module_name")
                        .and_then(|n| n.utf8_text(source_bytes).ok())
                        .map(|s| s.to_string());
                    let Some(module) = module_name else {
                        continue;
                    };

                    // Check for wildcard: `from X import *`
                    let mut wc = child.walk();
                    let has_wildcard = child
                        .children(&mut wc)
                        .any(|c| c.kind() == "wildcard_import");

                    if has_wildcard {
                        if namespace_seen.insert(module.clone()) {
                            namespace.push(module);
                        }
                        continue;
                    }

                    // Collect imported names from `name` fields and aliased_import nodes
                    let mut names: Vec<String> = Vec::new();
                    let mut inner = child.walk();
                    for c in child.children(&mut inner) {
                        match c.kind() {
                            "dotted_name" | "identifier" => {
                                // Skip the module_name node itself
                                if child.child_by_field_name("module_name") == Some(c) {
                                    continue;
                                }
                                // Check parent field: "name" fields are imported symbols
                                if let Ok(text) = c.utf8_text(source_bytes) {
                                    names.push(text.to_string());
                                }
                            }
                            "aliased_import" => {
                                // `A as B` -> store the original name A
                                if let Some(name_node) = c.child_by_field_name("name")
                                    && let Ok(text) = name_node.utf8_text(source_bytes)
                                {
                                    names.push(text.to_string());
                                }
                            }
                            _ => {}
                        }
                    }

                    if !names.is_empty() {
                        named.entry(module).or_default().extend(names);
                    }
                }
                "import_statement" => {
                    // `import module` or `import module as alias`
                    let mut inner = child.walk();
                    for c in child.children(&mut inner) {
                        match c.kind() {
                            "dotted_name" => {
                                if let Ok(text) = c.utf8_text(source_bytes)
                                    && namespace_seen.insert(text.to_string())
                                {
                                    namespace.push(text.to_string());
                                }
                            }
                            "aliased_import" => {
                                if let Some(name_node) = c.child_by_field_name("name")
                                    && let Ok(text) = name_node.utf8_text(source_bytes)
                                    && namespace_seen.insert(text.to_string())
                                {
                                    namespace.push(text.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // Deduplicate named import values per source
        for names in named.values_mut() {
            names.sort();
            names.dedup();
        }
        namespace.sort();

        (named, namespace)
    }

    /// Collect top-level function definition names for `function_index`.
    ///
    /// Returns names from `function_definition` nodes at module scope,
    /// excluding private names (prefixed with `_`).
    fn extract_function_names(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let mut names = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut iter = cursor.matches(&self.func_query, root_node, source_bytes);
        while let Some(m) = iter.next() {
            for capture in m.captures {
                if let Ok(text) = capture.node.utf8_text(source_bytes)
                    && !text.starts_with('_')
                {
                    names.push(text.to_string());
                }
            }
        }
        names
    }

    fn extract_dependencies(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        collect_matches(&self.relative_import_query, root_node, source.as_bytes())
            .into_iter()
            .map(|s| dot_import_to_path(&s))
            .collect()
    }

    fn extract_decorators(&self, source: &str, root_node: tree_sitter::Node) -> Vec<String> {
        let source_bytes = source.as_bytes();
        let simple = collect_matches(&self.decorator_query, root_node, source_bytes);
        let dotted = collect_matches(&self.dotted_decorator_query, root_node, source_bytes);
        let seen: HashSet<String> = simple.iter().cloned().collect();
        let mut merged = simple;
        merged.extend(dotted.into_iter().filter(|d| !seen.contains(d)));
        merged.sort();
        merged
    }
}

impl Parser for PythonParser {
    fn parse(&mut self, source: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python source"))?;

        let root_node = tree.root_node();

        let mut exports = self.extract_exports(source, root_node);
        let imports = self.extract_imports(source, root_node);
        let dependencies = self.extract_dependencies(source, root_node);
        let (named_imports, namespace_imports) = self.extract_named_imports(source, root_node);
        let loc = source.lines().count();

        // ALP-769: extract public methods from exported classes
        let exported_classes: HashSet<String> = exports
            .iter()
            .filter(|e| e.parent_class.is_none())
            .map(|e| e.name.clone())
            .collect();
        let methods = self.extract_class_methods(source, root_node, &exported_classes);
        exports.extend(methods);
        exports.sort_by_key(|e| e.start_line);

        let decorators = self.extract_decorators(source, root_node);
        let function_names = self.extract_function_names(source, root_node);
        let has_custom = !decorators.is_empty() || !function_names.is_empty();
        let custom_fields = if !has_custom {
            None
        } else {
            let mut fields = HashMap::new();
            if !decorators.is_empty() {
                fields.insert(
                    "decorators".to_string(),
                    serde_json::Value::Array(
                        decorators
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            if !function_names.is_empty() {
                fields.insert(
                    "function_names".to_string(),
                    serde_json::Value::Array(
                        function_names
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            Some(fields)
        };

        Ok(ParseResult {
            metadata: Metadata {
                exports,
                imports,
                dependencies,
                loc,
                named_imports,
                namespace_imports,
            },
            custom_fields,
        })
    }

    fn language_id(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }
}

pub(crate) const DESCRIPTOR: crate::parser::RegisteredLanguage =
    crate::parser::RegisteredLanguage {
        language_id: "python",
        extensions: &["py"],
        reexport_filenames: &["__init__.py"],
        test_patterns: crate::parser::LanguageTestPatterns {
            filename_suffixes: &["_test.py"],
            filename_prefixes: &["test_"],
            test_symbol_prefixes: &["test_"],
        },
    };
