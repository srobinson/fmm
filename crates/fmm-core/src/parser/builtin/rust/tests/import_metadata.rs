use super::support::parse;

#[test]
fn named_imports_scoped_identifier() {
    let source = "use std::collections::HashMap;\nuse anyhow::Result;";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "use std::collections::HashMap -> named_imports; got: {ni:?}"
    );
    assert_eq!(
        ni.get("anyhow").map(Vec::as_slice),
        Some(vec!["Result".to_string()].as_slice()),
        "use anyhow::Result -> named_imports; got: {ni:?}"
    );
}

#[test]
fn named_imports_grouped() {
    let source = "use std::collections::{HashMap, BTreeMap};";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    let names = ni
        .get("std::collections")
        .expect("should have std::collections");
    assert!(names.contains(&"HashMap".to_string()), "got: {names:?}");
    assert!(names.contains(&"BTreeMap".to_string()), "got: {names:?}");
}

#[test]
fn named_imports_crate_path() {
    let source = "use crate::parser::Metadata;";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("crate::parser").map(Vec::as_slice),
        Some(vec!["Metadata".to_string()].as_slice()),
        "use crate::parser::Metadata -> named_imports; got: {ni:?}"
    );
}

#[test]
fn named_imports_aliased_stores_original() {
    let source = "use std::collections::HashMap as Map;";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "aliased import should store original name; got: {ni:?}"
    );
}

#[test]
fn namespace_imports_wildcard() {
    let source = "use crate::parser::*;";
    let result = parse(source);
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"crate::parser".to_string()),
        "use crate::parser::* -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    assert!(
        result.metadata.named_imports.is_empty(),
        "wildcard should not populate named_imports"
    );
}

#[test]
fn named_imports_nested_groups() {
    let source = "use std::{collections::HashMap, io::Read};";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
        "nested group std::collections::HashMap; got: {ni:?}"
    );
    assert_eq!(
        ni.get("std::io").map(Vec::as_slice),
        Some(vec!["Read".to_string()].as_slice()),
        "nested group std::io::Read; got: {ni:?}"
    );
}

#[test]
fn named_imports_self_in_group() {
    let source = "use crate::parser::{self, Metadata};";
    let result = parse(source);
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"crate::parser".to_string()),
        "self in group -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("crate::parser").map(Vec::as_slice),
        Some(vec!["Metadata".to_string()].as_slice()),
        "Metadata alongside self; got: {ni:?}"
    );
}

#[test]
fn named_imports_aliased_in_group() {
    let source = "use std::collections::{HashMap as Map, BTreeMap};";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    let names = ni
        .get("std::collections")
        .expect("should have std::collections");
    assert!(
        names.contains(&"HashMap".to_string()),
        "aliased in group stores original; got: {names:?}"
    );
    assert!(
        names.contains(&"BTreeMap".to_string()),
        "non-aliased sibling; got: {names:?}"
    );
}

#[test]
fn named_imports_super_path() {
    let source = "use super::utils::Helper;";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("super::utils").map(Vec::as_slice),
        Some(vec!["Helper".to_string()].as_slice()),
        "use super::utils::Helper; got: {ni:?}"
    );
}

#[test]
fn named_imports_mixed_forms() {
    let source = r#"
use std::collections::HashMap;
use crate::parser::{Metadata, ExportEntry};
use crate::config::*;
use anyhow::Result as AnyhowResult;
"#;
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    let ns = &result.metadata.namespace_imports;

    assert_eq!(
        ni.get("std::collections").map(Vec::as_slice),
        Some(vec!["HashMap".to_string()].as_slice()),
    );
    let parser_names = ni.get("crate::parser").expect("crate::parser");
    assert!(parser_names.contains(&"Metadata".to_string()));
    assert!(parser_names.contains(&"ExportEntry".to_string()));
    assert!(ns.contains(&"crate::config".to_string()));
    assert_eq!(
        ni.get("anyhow").map(Vec::as_slice),
        Some(vec!["Result".to_string()].as_slice()),
    );
}
