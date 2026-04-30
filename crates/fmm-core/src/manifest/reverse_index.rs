use std::collections::HashMap;

use super::{Manifest, dependency_matcher::build_reverse_deps};

pub type ReverseDeps = HashMap<String, Vec<String>>;

impl Manifest {
    /// Rebuild the reverse dependency index from the current file set.
    ///
    /// Called automatically by `load_from_sidecars`. Call this manually when
    /// building a manifest incrementally via `add_file` (e.g. in tests or
    /// benchmarks) to ensure downstream lookups are accurate.
    pub fn rebuild_reverse_deps(&mut self) {
        self.reverse_deps = build_reverse_deps(self);
    }
}
