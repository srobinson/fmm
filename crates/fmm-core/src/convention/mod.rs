/// Static test file and test symbol conventions supplied by a convention plugin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConventionTestPatterns {
    /// Path segments that indicate a test file, for example "/test/".
    pub path_contains: &'static [&'static str],
    /// Filename suffixes that indicate a test file, for example ".spec.ts".
    pub filename_suffixes: &'static [&'static str],
    /// Filename prefixes that indicate a test file, for example "test_".
    pub filename_prefixes: &'static [&'static str],
    /// Symbol name prefixes that indicate a test export, for example "test_".
    pub test_symbol_prefixes: &'static [&'static str],
}

impl ConventionTestPatterns {
    pub const EMPTY: Self = Self {
        path_contains: &[],
        filename_suffixes: &[],
        filename_prefixes: &[],
        test_symbol_prefixes: &[],
    };
}

/// Static project convention contract.
///
/// Convention plugins describe framework or project knowledge that is outside
/// parser construction. Implementations expose static data so registry work can
/// collect conventions without constructing language parsers.
pub trait ConventionPlugin: Send + Sync + 'static {
    fn id() -> &'static str;

    fn languages() -> &'static [&'static str] {
        &[]
    }

    fn enablers() -> &'static [&'static str] {
        &[]
    }

    fn entry_patterns() -> &'static [&'static str] {
        &[]
    }

    fn generated_patterns() -> &'static [&'static str] {
        &[]
    }

    fn virtual_module_prefixes() -> &'static [&'static str] {
        &[]
    }

    fn always_used_symbols() -> &'static [&'static str] {
        &[]
    }

    fn test_patterns() -> ConventionTestPatterns {
        ConventionTestPatterns::EMPTY
    }
}
