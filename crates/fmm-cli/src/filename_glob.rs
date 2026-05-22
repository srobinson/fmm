//! Shared filename glob matching for CLI and MCP surfaces.

/// Compiled shell-style glob pattern scoped to a single filename.
#[derive(Debug)]
pub(crate) struct FilenameGlob {
    pattern: glob::Pattern,
}

impl FilenameGlob {
    pub(crate) fn new(pattern: &str) -> Result<Self, glob::PatternError> {
        Ok(Self {
            pattern: glob::Pattern::new(pattern)?,
        })
    }

    pub(crate) fn matches(&self, filename: &str) -> bool {
        self.pattern.matches(filename)
    }
}

/// Match a shell-style glob pattern against a filename.
///
/// Invalid patterns return `false`; call `FilenameGlob::new` when callers need
/// to surface pattern syntax errors.
#[cfg(test)]
pub(crate) fn glob_filename_matches(pattern: &str, filename: &str) -> bool {
    match FilenameGlob::new(pattern) {
        Ok(matcher) => matcher.matches(filename),
        Err(_) => false,
    }
}
