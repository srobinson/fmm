use super::support::parse;

#[test]
fn pub_use_simple_path_indexes_rightmost_segment() {
    let source = "pub use crate::runtime::Runtime;";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Runtime".to_string()),
        "expected Runtime in {:?}",
        names
    );
}

#[test]
fn pub_use_alias_indexes_alias_name() {
    let source = "pub use crate::runtime::Runtime as Rt;";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Rt".to_string()),
        "expected Rt in {:?}",
        names
    );
    assert!(
        !names.contains(&"Runtime".to_string()),
        "Runtime should not appear (aliased)"
    );
}

#[test]
fn pub_use_grouped_indexes_each_name() {
    let source = "pub use crate::task::{JoinHandle, LocalSet};";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"JoinHandle".to_string()),
        "expected JoinHandle in {:?}",
        names
    );
    assert!(
        names.contains(&"LocalSet".to_string()),
        "expected LocalSet in {:?}",
        names
    );
}

#[test]
fn pub_use_grouped_with_alias_indexes_alias() {
    let source = "pub use crate::task::{JoinHandle as JH};";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"JH".to_string()),
        "expected JH in {:?}",
        names
    );
    assert!(
        !names.contains(&"JoinHandle".to_string()),
        "JoinHandle should not appear (aliased)"
    );
}

#[test]
fn pub_use_wildcard_skipped() {
    let source = "pub use crate::prelude::*;";
    let result = parse(source);
    assert!(
        result.metadata.exports.is_empty(),
        "wildcard pub use should emit no exports, got {:?}",
        result.metadata.export_names()
    );
}

#[test]
fn non_pub_use_not_indexed() {
    let source = "use crate::runtime::Runtime;";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"Runtime".to_string()),
        "non-pub use should not be indexed"
    );
}

#[test]
fn pub_use_external_crate_indexes_rightmost() {
    let source = "pub use tokio_util::codec::Framed;";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        names.contains(&"Framed".to_string()),
        "expected Framed in {:?}",
        names
    );
}

#[test]
fn pub_crate_use_not_indexed() {
    let source = "pub(crate) use crate::runtime::Runtime;";
    let result = parse(source);
    let names = result.metadata.export_names();
    assert!(
        !names.contains(&"Runtime".to_string()),
        "pub(crate) use should not be indexed as a public export"
    );
}
