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

pub(crate) fn filter_cycles(
    cycles: Vec<Vec<String>>,
    filter: &str,
    is_test_file: impl Fn(&str) -> bool,
) -> Vec<Vec<String>> {
    cycles
        .into_iter()
        .filter_map(|cycle| {
            let keeps_self_loop = cycle.len() == 1;
            let members = cycle
                .into_iter()
                .filter(|path| match filter {
                    "source" => !is_test_file(path),
                    "tests" => is_test_file(path),
                    _ => true,
                })
                .collect::<Vec<_>>();
            (members.len() > 1 || (keeps_self_loop && members.len() == 1)).then_some(members)
        })
        .collect()
}
