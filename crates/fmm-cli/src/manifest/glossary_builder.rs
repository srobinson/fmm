use serde::Serialize;

use super::dependency_matcher::{
    builtin_source_extensions, dep_matches, dotted_dep_matches, python_dep_matches,
};
use super::{ExportLines, Manifest};

/// Controls which exports are included when building the glossary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlossaryMode {
    /// Definitions from non-test files, `used_by` filtered to non-test callers (default).
    #[default]
    Source,
    /// Definitions from non-test files, `used_by` filtered to test callers only — shows coverage.
    Tests,
    /// All definitions, all `used_by` unfiltered.
    All,
}

/// Entry for a single export name in the glossary.
#[derive(Debug, Clone, Serialize)]
pub struct GlossaryEntry {
    pub name: String,
    pub sources: Vec<GlossarySource>,
}

/// One definition of a glossary export — the file it lives in, its line range,
/// and all files that import it.
#[derive(Debug, Clone, Serialize)]
pub struct GlossarySource {
    pub file: String,
    pub lines: Option<ExportLines>,
    pub used_by: Vec<String>,
    /// ALP-865: files that import via a namespace (`import * as ns`) — call-site precision
    /// unavailable for these; they are reported separately with a disclosure note.
    /// Each entry is `(file_path, namespace_name)`.
    #[serde(skip)]
    pub namespace_callers: Vec<(String, String)>,
    /// ALP-882: count of files excluded by Layer 2 (import the module but not this specific symbol).
    #[serde(skip)]
    pub layer2_excluded_count: usize,
    /// ALP-882: files that use a namespace import from the source module, detected at Layer 2.
    /// Symbol use cannot be verified without call-site analysis; disclosed separately.
    #[serde(skip)]
    pub layer2_namespace_callers: Vec<String>,
    /// ALP-883: files that re-export the symbol but have no call site (Layer 3 detection).
    /// These ARE impacted by a rename — disclosed separately as "re-exports only".
    #[serde(skip)]
    pub reexport_files: Vec<String>,
}

/// Return a reference to the lazily-initialised `ParserRegistry` used for
/// test file and test symbol detection.
fn builtin_registry() -> &'static crate::parser::ParserRegistry {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<crate::parser::ParserRegistry> = OnceLock::new();
    REGISTRY.get_or_init(crate::parser::ParserRegistry::with_builtins)
}

/// Returns true if a file path is a test file (by path conventions only, ignoring symbol name).
///
/// Two layers of detection:
/// 1. **Language-specific**: delegates to `ParserRegistry::is_language_test_file()`,
///    which checks filename suffixes/prefixes declared by each language descriptor.
///    Adding `test_file_patterns()` to a new parser automatically extends this.
/// 2. **Directory-based** (language-neutral): `tests/`, `test/`, `__tests__/` path segments.
fn is_test_file(file: &str) -> bool {
    // Language-specific filename patterns (from registry descriptors)
    if builtin_registry().is_language_test_file(file) {
        return true;
    }
    // Language-neutral directory conventions
    file.starts_with("tests/")
        || file.starts_with("test/")
        || file.starts_with("__tests__/")
        || file.contains("/tests/")
        || file.contains("/test/")
        || file.contains("/__tests__/")
}

/// Returns true if an export should be classified as a test artifact.
///
/// Checks symbol name conventions (from registry `test_symbol_prefixes`)
/// and file path conventions (see `is_test_file`).
fn is_test_export(name: &str, file: &str) -> bool {
    // Check symbol name against all registered test symbol prefixes
    let registry = builtin_registry();
    for desc in registry.descriptors() {
        for prefix in desc.test_patterns.test_symbol_prefixes {
            if name.starts_with(prefix) {
                return true;
            }
        }
    }
    is_test_file(file)
}

impl Manifest {
    /// Build the glossary: for each export name matching `pattern` (case-insensitive
    /// substring), collect all definitions and their dependents.
    /// Returns entries sorted alphabetically by name (case-insensitive).
    ///
    /// `mode` controls test filtering:
    /// - `Source` (default): definitions from non-test files, `used_by` filtered to non-test callers
    /// - `Tests`: definitions from non-test files, `used_by` filtered to test callers — shows coverage
    /// - `All`: all definitions, all `used_by` unfiltered
    pub fn build_glossary(&self, pattern: &str, mode: GlossaryMode) -> Vec<GlossaryEntry> {
        let pat_lower = pattern.to_lowercase();
        let mut entries: Vec<GlossaryEntry> = self
            .export_all
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pat_lower))
            .filter_map(|(name, locations)| {
                let sources: Vec<GlossarySource> = locations
                    .iter()
                    .filter(|loc| match mode {
                        GlossaryMode::Source | GlossaryMode::Tests => {
                            !is_test_export(name, &loc.file)
                        }
                        GlossaryMode::All => true,
                    })
                    .map(|loc| {
                        let all_used_by = self.find_dependents(&loc.file);
                        let used_by = match mode {
                            GlossaryMode::Source => all_used_by
                                .into_iter()
                                .filter(|f| !is_test_file(f))
                                .collect(),
                            GlossaryMode::Tests => all_used_by
                                .into_iter()
                                .filter(|f| is_test_file(f))
                                .collect(),
                            GlossaryMode::All => all_used_by,
                        };
                        GlossarySource {
                            file: loc.file.clone(),
                            lines: loc.lines.clone(),
                            used_by,
                            namespace_callers: Vec::new(),
                            layer2_excluded_count: 0,
                            layer2_namespace_callers: Vec::new(),
                            reexport_files: Vec::new(),
                        }
                    })
                    .collect();
                if sources.is_empty() {
                    None
                } else {
                    Some(GlossaryEntry {
                        name: name.clone(),
                        sources,
                    })
                }
            })
            .collect();
        // Second pass: method_index entries (dotted names like "ClassName.method").
        // Pattern matches on the full dotted name, so both "create" and
        // "NestFactoryStatic.create" substring queries will hit the same entry.
        for (dotted_name, loc) in &self.method_index {
            let lower = dotted_name.to_lowercase();
            if !lower.contains(&pat_lower) {
                continue;
            }
            // For Source/Tests modes, skip methods defined in test files.
            if matches!(mode, GlossaryMode::Source | GlossaryMode::Tests) && is_test_file(&loc.file)
            {
                continue;
            }
            let all_used_by = self.find_dependents(&loc.file);
            let used_by = match mode {
                GlossaryMode::Source => all_used_by
                    .into_iter()
                    .filter(|f| !is_test_file(f))
                    .collect(),
                GlossaryMode::Tests => all_used_by
                    .into_iter()
                    .filter(|f| is_test_file(f))
                    .collect(),
                GlossaryMode::All => all_used_by,
            };
            entries.push(GlossaryEntry {
                name: dotted_name.clone(),
                sources: vec![GlossarySource {
                    file: loc.file.clone(),
                    lines: loc.lines.clone(),
                    used_by,
                    namespace_callers: Vec::new(),
                    layer2_excluded_count: 0,
                    layer2_namespace_callers: Vec::new(),
                    reexport_files: Vec::new(),
                }],
            });
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        entries
    }

    /// Find all files that depend on `target_file`.
    ///
    /// When the manifest is loaded from SQLite, uses the precomputed `reverse_deps` index
    /// (O(1), includes cross-package bare specifier edges built by `build_reverse_deps`).
    /// Falls back to scanning all files when `reverse_deps` is empty — used for manifests
    /// built programmatically via `add_file` without calling `rebuild_reverse_deps`.
    pub fn find_dependents(&self, target_file: &str) -> Vec<String> {
        if !self.reverse_deps.is_empty() {
            let mut deps = self
                .reverse_deps
                .get(target_file)
                .cloned()
                .unwrap_or_default();
            deps.sort();
            return deps;
        }

        // Fallback: scan all files for dependency matches (programmatic manifest path).
        let mut dependents: Vec<String> = self
            .files
            .iter()
            .filter(|(path, entry)| {
                let path = path.as_str();
                if path == target_file {
                    return false;
                }
                let exts = builtin_source_extensions();
                entry.dependencies.iter().any(|d| {
                    dep_matches(d, target_file, path, exts)
                        || python_dep_matches(d, target_file, path)
                }) || entry
                    .imports
                    .iter()
                    .any(|i| dotted_dep_matches(i, target_file))
            })
            .map(|(path, _)| path.clone())
            .collect();
        dependents.sort();
        dependents
    }

    /// Count how many test files depend on `target_file`.
    ///
    /// Used by the MCP glossary tool to surface a test-caller hint when
    /// a dotted query returns empty results in source mode.
    pub fn count_test_dependents(&self, target_file: &str) -> usize {
        self.find_dependents(target_file)
            .into_iter()
            .filter(|f| is_test_file(f))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;
    use crate::parser::{ExportEntry, Metadata};

    fn entry(name: &str, start: usize, end: usize) -> ExportEntry {
        ExportEntry::new(name.to_string(), start, end)
    }

    fn method_entry(class: &str, method: &str, start: usize, end: usize) -> ExportEntry {
        ExportEntry::method(method.to_string(), start, end, class.to_string())
    }

    #[test]
    fn build_glossary_returns_alphabetically_sorted_entries() {
        let mut manifest = Manifest::new();
        for (name, file) in [
            ("zebra", "z.ts"),
            ("alpha", "a.ts"),
            ("Config", "c.ts"),
            ("beta", "b.ts"),
        ] {
            manifest.add_file(
                file,
                Metadata {
                    exports: vec![entry(name, 1, 5)],
                    imports: vec![],
                    dependencies: vec![],
                    loc: 10,
                    ..Default::default()
                },
            );
        }

        let entries = manifest.build_glossary("a", GlossaryMode::All);
        // "alpha" matches; "zebra" matches (contains "a"); "beta" matches; "Config" does not
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        // Sorted case-insensitively
        assert!(names
            .windows(2)
            .all(|w| w[0].to_lowercase() <= w[1].to_lowercase()));
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"zebra"));
        assert!(names.contains(&"beta"));
        assert!(!names.contains(&"Config"));
    }

    #[test]
    fn build_glossary_case_insensitive_pattern() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/config.ts",
            Metadata {
                exports: vec![entry("AppConfig", 1, 5), entry("loadConfig", 7, 12)],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
                ..Default::default()
            },
        );

        let entries = manifest.build_glossary("CONFIG", GlossaryMode::All);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"AppConfig"));
        assert!(names.contains(&"loadConfig"));
    }

    #[test]
    fn build_glossary_filters_test_exports_by_default() {
        let mut manifest = Manifest::new();
        // Normal export alongside a test function in the same file
        manifest.add_file(
            "src/agent.py",
            Metadata {
                exports: vec![
                    entry("run_dispatch", 1, 50),
                    entry("test_run_dispatch", 51, 80),
                ],
                imports: vec![],
                dependencies: vec![],
                loc: 80,
                ..Default::default()
            },
        );
        // Go test function (Test prefix) in a _test.go file
        manifest.add_file(
            "agent_test.go",
            Metadata {
                exports: vec![entry("TestRunDispatch", 1, 20)],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
                ..Default::default()
            },
        );
        // Export under tests/ directory
        manifest.add_file(
            "tests/helpers.py",
            Metadata {
                exports: vec![entry("helper_fixture", 1, 10)],
                imports: vec![],
                dependencies: vec![],
                loc: 10,
                ..Default::default()
            },
        );

        // Source mode (default): test artifacts excluded
        let entries = manifest.build_glossary("", GlossaryMode::Source);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"run_dispatch"),
            "normal export should be included"
        );
        assert!(
            !names.contains(&"test_run_dispatch"),
            "test_ prefix should be excluded"
        );
        assert!(
            !names.contains(&"TestRunDispatch"),
            "Test prefix should be excluded"
        );
        assert!(
            !names.contains(&"helper_fixture"),
            "tests/ dir export should be excluded"
        );

        // Tests mode: same definition filter as source (non-test files only),
        // but used_by is filtered to test callers. Add a test file dependent to verify.
        manifest.add_file(
            "tests/agent_spec.py",
            Metadata {
                exports: vec![],
                imports: vec![],
                dependencies: vec!["../src/agent".to_string()],
                loc: 5,
                ..Default::default()
            },
        );
        let entries_tests = manifest.build_glossary("", GlossaryMode::Tests);
        let names_tests: Vec<&str> = entries_tests.iter().map(|e| e.name.as_str()).collect();
        // Source symbols appear (same definition filter as source mode)
        assert!(
            names_tests.contains(&"run_dispatch"),
            "run_dispatch should appear in tests mode (source file definition)"
        );
        // Test-named symbol from a non-test file is still filtered out (is_test_export)
        assert!(
            !names_tests.contains(&"test_run_dispatch"),
            "test_ prefix excluded from definitions in tests mode"
        );
        // Test-file definitions excluded
        assert!(
            !names_tests.contains(&"TestRunDispatch"),
            "agent_test.go exports excluded from tests mode definitions"
        );
        assert!(
            !names_tests.contains(&"helper_fixture"),
            "tests/ dir exports excluded from tests mode definitions"
        );
        // used_by for run_dispatch should contain the test-file dependent
        let rd_entry = entries_tests
            .iter()
            .find(|e| e.name == "run_dispatch")
            .unwrap();
        assert!(
            rd_entry.sources[0]
                .used_by
                .iter()
                .any(|f| f == "tests/agent_spec.py"),
            "tests/agent_spec.py should appear in used_by for tests mode"
        );

        // All mode: everything returned
        let entries_all = manifest.build_glossary("", GlossaryMode::All);
        let names_all: Vec<&str> = entries_all.iter().map(|e| e.name.as_str()).collect();
        assert!(names_all.contains(&"run_dispatch"));
        assert!(names_all.contains(&"test_run_dispatch"));
        assert!(names_all.contains(&"TestRunDispatch"));
        assert!(names_all.contains(&"helper_fixture"));
    }

    #[test]
    fn is_test_export_covers_all_conventions() {
        // Symbol name prefix
        assert!(is_test_export("test_foo", "src/agent.py"));
        assert!(is_test_export("TestFoo", "agent.go"));
        assert!(!is_test_export("Config", "src/config.ts"));
        // Go test file
        assert!(is_test_export("anything", "agent_test.go"));
        assert!(!is_test_export("anything", "agent.go"));
        // Python test files
        assert!(is_test_export("foo", "test_agent.py"));
        assert!(is_test_export("foo", "agent_test.py"));
        assert!(!is_test_export("foo", "agent.py"));
        // Test directories
        assert!(is_test_export("foo", "tests/helpers.py"));
        assert!(is_test_export("foo", "test/fixtures.ts"));
        assert!(is_test_export("foo", "__tests__/utils.ts"));
        assert!(is_test_export("foo", "src/tests/helpers.py"));
        assert!(!is_test_export("foo", "src/config.ts"));
    }

    #[test]
    fn build_glossary_includes_method_definitions() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/factory.ts",
            Metadata {
                exports: vec![
                    entry("NestFactoryStatic", 1, 381),
                    method_entry("NestFactoryStatic", "createApplicationContext", 166, 195),
                ],
                imports: vec![],
                dependencies: vec![],
                loc: 381,
                ..Default::default()
            },
        );

        // Pattern matches on the method name alone (substring of dotted name)
        let entries = manifest.build_glossary("createApplicationContext", GlossaryMode::Source);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"NestFactoryStatic.createApplicationContext"),
            "should find method via substring pattern, got: {:?}",
            names
        );

        // Pattern matches the full dotted name
        let entries2 = manifest.build_glossary(
            "NestFactoryStatic.createApplicationContext",
            GlossaryMode::All,
        );
        let names2: Vec<&str> = entries2.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names2.contains(&"NestFactoryStatic.createApplicationContext"),
            "should find method via full dotted name, got: {:?}",
            names2
        );
    }

    #[test]
    fn build_glossary_method_tests_mode_returns_test_dependents() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/factory.ts",
            Metadata {
                exports: vec![
                    entry("NestFactoryStatic", 1, 200),
                    method_entry("NestFactoryStatic", "create", 79, 113),
                ],
                imports: vec![],
                dependencies: vec![],
                loc: 200,
                ..Default::default()
            },
        );
        // Test file that depends on the class file
        manifest.add_file(
            "tests/factory.spec.ts",
            Metadata {
                exports: vec![],
                imports: vec![],
                dependencies: vec!["../src/factory".to_string()],
                loc: 10,
                ..Default::default()
            },
        );

        let entries = manifest.build_glossary("create", GlossaryMode::Tests);
        let method_entry_found = entries
            .iter()
            .find(|e| e.name == "NestFactoryStatic.create");
        assert!(
            method_entry_found.is_some(),
            "method should appear in tests mode"
        );
        let used_by = &method_entry_found.unwrap().sources[0].used_by;
        assert!(
            used_by.iter().any(|f| f == "tests/factory.spec.ts"),
            "test file should appear in used_by for tests mode, got: {:?}",
            used_by
        );
    }

    #[test]
    fn build_glossary_sorted_with_method_entries_mixed_in() {
        let mut manifest = Manifest::new();
        manifest.add_file(
            "src/a.ts",
            Metadata {
                exports: vec![
                    entry("create_a", 1, 5),
                    method_entry("MyClass", "createB", 10, 20),
                ],
                imports: vec![],
                dependencies: vec![],
                loc: 20,
                ..Default::default()
            },
        );

        let entries = manifest.build_glossary("create", GlossaryMode::All);
        // Both export and method match; result must be sorted
        assert!(entries.len() >= 2, "should have at least 2 matches");
        assert!(entries
            .windows(2)
            .all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()));
    }
}
