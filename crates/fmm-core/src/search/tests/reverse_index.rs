use super::super::dependency_graph;
use super::support::manifest_with;

#[test]
fn reverse_index_large_manifest_correctness() {
    let hub = "core/base.ts";
    let mut files: Vec<(&str, Vec<&str>)> = vec![(hub, vec![])];
    let paths: Vec<String> = (0..999).map(|i| format!("spoke/file_{}.ts", i)).collect();
    for path in &paths {
        files.push((path.as_str(), vec!["../core/base"]));
    }

    let manifest = manifest_with(files);
    let entry = manifest.files[hub].clone();
    let (local, _, downstream) = dependency_graph(&manifest, hub, &entry);

    assert!(local.is_empty(), "hub has no upstream deps");
    assert_eq!(
        downstream.len(),
        999,
        "all 999 spoke files should appear downstream, got {}",
        downstream.len()
    );

    assert!(
        downstream.contains(&&"spoke/file_0.ts".to_string()),
        "spoke/file_0.ts should be downstream"
    );
    assert!(
        downstream.contains(&&"spoke/file_998.ts".to_string()),
        "spoke/file_998.ts should be downstream"
    );

    let spoke_entry = manifest.files["spoke/file_0.ts"].clone();
    let (spoke_local, _, spoke_down) = dependency_graph(&manifest, "spoke/file_0.ts", &spoke_entry);
    assert!(
        spoke_local.contains(&hub.to_string()),
        "hub should be upstream of spoke, got: {:?}",
        spoke_local
    );
    assert!(spoke_down.is_empty(), "spokes have no downstream");
}
