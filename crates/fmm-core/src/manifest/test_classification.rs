use std::sync::OnceLock;

fn builtin_registry() -> &'static crate::parser::ParserRegistry {
    static REGISTRY: OnceLock<crate::parser::ParserRegistry> = OnceLock::new();
    REGISTRY.get_or_init(crate::parser::ParserRegistry::with_builtins)
}

fn builtin_convention_registry() -> &'static crate::convention::ConventionRegistry<'static> {
    static REGISTRY: OnceLock<crate::convention::ConventionRegistry<'static>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        crate::convention::ConventionRegistry::with_builtin_conventions(builtin_registry())
    })
}

/// Returns true if a file path is a test file by path conventions only.
pub(crate) fn is_test_file(file: &str) -> bool {
    builtin_convention_registry().is_test_file(file)
}

/// Returns true if an export should be classified as a test artifact.
pub(crate) fn is_test_export(name: &str, file: &str, declaration_kind: Option<&str>) -> bool {
    if matches!(declaration_kind, Some("test")) || name == "tests" {
        return true;
    }

    let registry = builtin_convention_registry();
    for patterns in registry.language_test_patterns() {
        for prefix in patterns.test_symbol_prefixes {
            if name.starts_with(prefix) {
                return true;
            }
        }
    }
    is_test_file(file)
}

#[cfg(test)]
mod tests {
    use super::is_test_export;

    #[test]
    fn is_test_export_covers_symbol_and_path_conventions() {
        assert!(is_test_export("unit_foo", "src/agent.py", Some("test")));
        assert!(is_test_export("tests", "src/agent.py", None));

        assert!(is_test_export("test_foo", "src/agent.py", None));
        assert!(is_test_export("TestFoo", "agent.go", None));
        assert!(!is_test_export("Config", "src/config.ts", None));

        assert!(is_test_export("anything", "agent_test.go", None));
        assert!(!is_test_export("anything", "agent.go", None));

        assert!(is_test_export("foo", "test_agent.py", None));
        assert!(is_test_export("foo", "agent_test.py", None));
        assert!(!is_test_export("foo", "agent.py", None));

        assert!(is_test_export("foo", "tests/helpers.py", None));
        assert!(is_test_export("foo", "test/fixtures.ts", None));
        assert!(is_test_export("foo", "__tests__/utils.ts", None));
        assert!(is_test_export("foo", "src/tests/helpers.py", None));
        assert!(!is_test_export("foo", "src/config.ts", None));
    }
}
