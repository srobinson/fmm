use fmm_core::search::CycleEdgeMode;

pub(crate) fn parse_edge_mode(edge_mode: Option<&str>) -> Result<CycleEdgeMode, String> {
    match edge_mode.unwrap_or("runtime") {
        "runtime" => Ok(CycleEdgeMode::Runtime),
        "all" => Ok(CycleEdgeMode::All),
        other => Err(format!(
            "Invalid edge_mode '{}'. Valid values: runtime, all.",
            other
        )),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CycleFileFilter {
    All,
    Source,
    Tests,
}

impl CycleFileFilter {
    pub(crate) fn parse(filter: &str) -> Result<Self, String> {
        match filter {
            "all" => Ok(Self::All),
            "source" => Ok(Self::Source),
            "tests" => Ok(Self::Tests),
            other => Err(format!(
                "Invalid filter '{}'. Valid values: all, source, tests.",
                other
            )),
        }
    }

    pub(crate) fn keeps(self, path: &str, is_test_file: impl FnOnce(&str) -> bool) -> bool {
        match self {
            Self::All => true,
            Self::Source => !is_test_file(path),
            Self::Tests => is_test_file(path),
        }
    }
}
