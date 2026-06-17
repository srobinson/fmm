use crate::config::is_test_export_symbol;
use crate::convention::builtin_is_test_file;

/// Returns true if a file path is a test file by path conventions only.
pub(crate) fn is_test_file(file: &str) -> bool {
    builtin_is_test_file(file)
}

/// Returns true if an export should be classified as a test artifact.
pub(crate) fn is_test_export(name: &str, file: &str, declaration_kind: Option<&str>) -> bool {
    is_test_export_symbol(name, declaration_kind) || is_test_file(file)
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
