use crate::manifest::dotted_dep_matches;

use super::super::dependency_graph;
use super::support::manifest_with_imports;

#[test]
fn dotted_dep_matches_basic() {
    assert!(dotted_dep_matches(
        "agno.models.message",
        "agno/models/message.py"
    ));
    assert!(dotted_dep_matches(
        "agno.models.message",
        "src/agno/models/message.py"
    ));
}

#[test]
fn dotted_dep_matches_package_init() {
    assert!(dotted_dep_matches("agno.models", "agno/models/__init__.py"));
    assert!(dotted_dep_matches(
        "agno.models",
        "src/agno/models/__init__.py"
    ));
}

#[test]
fn dotted_dep_matches_ignores_relative_and_paths() {
    assert!(!dotted_dep_matches("._run", "agno/agent/_run.py"));
    assert!(!dotted_dep_matches("./utils", "utils.py"));
    assert!(!dotted_dep_matches("os", "os.py"));
    assert!(!dotted_dep_matches("crate::config", "src/config.rs"));
}

#[test]
fn dependency_graph_resolves_dotted_absolute_imports() {
    let manifest = manifest_with_imports(vec![
        ("agno/models/message.py", vec![], vec![]),
        ("agno/models/response.py", vec![], vec![]),
        (
            "agno/models/interfaces.py",
            vec![],
            vec!["agno.models.message", "agno.models.response", "typing"],
        ),
    ]);
    let entry = manifest.files["agno/models/interfaces.py"].clone();

    let (local, external, downstream) =
        dependency_graph(&manifest, "agno/models/interfaces.py", &entry);

    assert!(
        local.contains(&"agno/models/message.py".to_string()),
        "should resolve agno.models.message, got local: {:?}",
        local
    );
    assert!(
        local.contains(&"agno/models/response.py".to_string()),
        "should resolve agno.models.response, got local: {:?}",
        local
    );
    assert!(
        external.contains(&"typing".to_string()),
        "typing should stay external, got: {:?}",
        external
    );
    assert!(downstream.is_empty(), "no downstream expected");
}

#[test]
fn dependency_graph_dotted_downstream_detection() {
    let manifest = manifest_with_imports(vec![
        ("agno/models/message.py", vec![], vec![]),
        (
            "agno/models/interfaces.py",
            vec![],
            vec!["agno.models.message"],
        ),
    ]);
    let entry = manifest.files["agno/models/message.py"].clone();

    let (_, _, downstream) = dependency_graph(&manifest, "agno/models/message.py", &entry);

    assert!(
        downstream.contains(&&"agno/models/interfaces.py".to_string()),
        "interfaces.py should appear as downstream of message.py, got: {:?}",
        downstream
    );
}

#[test]
fn dependency_graph_dotted_src_layout() {
    let manifest = manifest_with_imports(vec![
        ("src/agno/models/message.py", vec![], vec![]),
        (
            "src/agno/models/interfaces.py",
            vec![],
            vec!["agno.models.message"],
        ),
    ]);
    let entry = manifest.files["src/agno/models/interfaces.py"].clone();

    let (local, _, _) = dependency_graph(&manifest, "src/agno/models/interfaces.py", &entry);

    assert!(
        local.contains(&"src/agno/models/message.py".to_string()),
        "src layout should resolve agno.models.message, got: {:?}",
        local
    );
}
