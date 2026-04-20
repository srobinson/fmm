use crate::manifest::python_dep_matches;

use super::super::dependency_graph;
use super::support::manifest_with;

#[test]
fn python_dep_matches_single_dot() {
    // `from ._run import X` in `agno/agent/agent.py` should match `agno/agent/_run.py`.
    assert!(python_dep_matches(
        "._run",
        "agno/agent/_run.py",
        "agno/agent/agent.py"
    ));
    assert!(!python_dep_matches(
        "._run",
        "agno/agent/other.py",
        "agno/agent/agent.py"
    ));
}

#[test]
fn python_dep_matches_double_dot() {
    // `from ..config import X` in `agno/agent/agent.py` should match `agno/config.py`.
    assert!(python_dep_matches(
        "..config",
        "agno/config.py",
        "agno/agent/agent.py"
    ));
    assert!(!python_dep_matches(
        "..config",
        "agno/agent/config.py",
        "agno/agent/agent.py"
    ));
}

#[test]
fn python_dep_matches_dot_only_returns_false() {
    // `from . import X` cannot resolve to a specific file.
    assert!(!python_dep_matches(
        ".",
        "agno/agent/_run.py",
        "agno/agent/agent.py"
    ));
}

#[test]
fn python_dep_does_not_match_js_style() {
    // JS/TS style paths are handled by dep_matches.
    assert!(!python_dep_matches(
        "./utils",
        "src/utils.ts",
        "src/index.ts"
    ));
}

#[test]
fn dependency_graph_resolves_python_deps() {
    let manifest = manifest_with(vec![
        ("agno/agent/_run.py", vec![]),
        ("agno/agent/models.py", vec![]),
        (
            "agno/agent/agent.py",
            vec!["._run", ".models", "pydantic", "typing"],
        ),
    ]);
    let entry = manifest.files["agno/agent/agent.py"].clone();

    let (local, external, downstream) = dependency_graph(&manifest, "agno/agent/agent.py", &entry);

    assert!(
        local.contains(&"agno/agent/_run.py".to_string()),
        "should resolve ._run, got: {:?}",
        local
    );
    assert!(
        local.contains(&"agno/agent/models.py".to_string()),
        "should resolve .models, got: {:?}",
        local
    );
    assert!(
        external.contains(&"pydantic".to_string()),
        "pydantic should stay external, got: {:?}",
        external
    );
    assert!(
        external.contains(&"typing".to_string()),
        "typing should stay external, got: {:?}",
        external
    );
    assert!(downstream.is_empty(), "no downstream expected");
}

#[test]
fn dependency_graph_downstream_detects_python_dependents() {
    let manifest = manifest_with(vec![
        ("agno/agent/_run.py", vec![]),
        (
            "agno/agent/agent.py",
            vec!["._run"], // agent.py depends on _run.py through a relative import.
        ),
    ]);
    let entry = manifest.files["agno/agent/_run.py"].clone();

    let (_, _, downstream) = dependency_graph(&manifest, "agno/agent/_run.py", &entry);

    assert!(
        downstream.contains(&&"agno/agent/agent.py".to_string()),
        "agent.py should appear as downstream of _run.py, got: {:?}",
        downstream
    );
}
