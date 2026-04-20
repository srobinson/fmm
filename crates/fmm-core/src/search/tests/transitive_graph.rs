use crate::manifest::Manifest;

use super::super::dependency_graph_transitive;
use super::support::manifest_with;

fn chain_manifest() -> Manifest {
    manifest_with(vec![
        ("app/a.py", vec![".b"]),
        ("app/b.py", vec![".c"]),
        ("app/c.py", vec![".d"]),
        ("app/d.py", vec![]),
    ])
}

#[test]
fn transitive_upstream_depth1_matches_single_hop() {
    let manifest = chain_manifest();
    let entry = manifest.files["app/a.py"].clone();
    let (upstream, _ext, downstream) =
        dependency_graph_transitive(&manifest, "app/a.py", &entry, 1);

    let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(up_files, ["app/b.py"], "depth=1 upstream: direct dep only");
    assert!(
        upstream.iter().all(|(_, d)| *d == 1),
        "all depth=1 entries marked with d=1"
    );
    assert!(
        downstream.is_empty(),
        "nothing depends on app/a.py in the chain"
    );
}

#[test]
fn transitive_upstream_depth2_follows_two_hops() {
    let manifest = chain_manifest();
    let entry = manifest.files["app/a.py"].clone();
    let (upstream, _ext, _) = dependency_graph_transitive(&manifest, "app/a.py", &entry, 2);

    let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
    assert!(
        up_files.contains(&"app/b.py"),
        "app/b.py at depth 1; got: {:?}",
        up_files
    );
    assert!(
        up_files.contains(&"app/c.py"),
        "app/c.py at depth 2; got: {:?}",
        up_files
    );
    assert!(
        !up_files.contains(&"app/d.py"),
        "app/d.py should be beyond depth=2; got: {:?}",
        up_files
    );
    let b_depth = upstream.iter().find(|(f, _)| f == "app/b.py").unwrap().1;
    let c_depth = upstream.iter().find(|(f, _)| f == "app/c.py").unwrap().1;
    assert_eq!(b_depth, 1);
    assert_eq!(c_depth, 2);
}

#[test]
fn transitive_upstream_full_closure() {
    let manifest = chain_manifest();
    let entry = manifest.files["app/a.py"].clone();
    let (upstream, _ext, _) = dependency_graph_transitive(&manifest, "app/a.py", &entry, -1);

    let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
    assert!(up_files.contains(&"app/b.py"), "b in closure");
    assert!(up_files.contains(&"app/c.py"), "c in closure");
    assert!(up_files.contains(&"app/d.py"), "d in closure");
}

#[test]
fn transitive_downstream_multi_hop() {
    let manifest = chain_manifest();
    let entry = manifest.files["app/d.py"].clone();
    let (_up, _ext, downstream) = dependency_graph_transitive(&manifest, "app/d.py", &entry, -1);

    let down_files: Vec<&str> = downstream.iter().map(|(f, _)| f.as_str()).collect();
    assert!(
        down_files.contains(&"app/c.py"),
        "c depends on d at depth 1"
    );
    assert!(
        down_files.contains(&"app/b.py"),
        "b depends on c at depth 2"
    );
    assert!(
        down_files.contains(&"app/a.py"),
        "a depends on b at depth 3"
    );

    let c_depth = downstream.iter().find(|(f, _)| f == "app/c.py").unwrap().1;
    let b_depth = downstream.iter().find(|(f, _)| f == "app/b.py").unwrap().1;
    let a_depth = downstream.iter().find(|(f, _)| f == "app/a.py").unwrap().1;
    assert_eq!(c_depth, 1);
    assert_eq!(b_depth, 2);
    assert_eq!(a_depth, 3);
}

#[test]
fn transitive_cycle_does_not_loop() {
    let manifest = manifest_with(vec![("app/x.py", vec![".y"]), ("app/y.py", vec![".x"])]);
    let entry = manifest.files["app/x.py"].clone();
    let (upstream, _ext, downstream) =
        dependency_graph_transitive(&manifest, "app/x.py", &entry, -1);

    let up_files: Vec<&str> = upstream.iter().map(|(f, _)| f.as_str()).collect();
    assert!(up_files.contains(&"app/y.py"), "y is upstream of x");
    assert!(
        !up_files.contains(&"app/x.py"),
        "x must not appear in its own upstream"
    );

    let down_files: Vec<&str> = downstream.iter().map(|(f, _)| f.as_str()).collect();
    assert!(
        down_files.contains(&"app/y.py"),
        "y depends on x so appears downstream; got: {:?}",
        down_files
    );
    assert!(
        !down_files.contains(&"app/x.py"),
        "x must not appear in its own downstream"
    );
}
