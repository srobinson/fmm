use super::support::parse;

#[test]
fn named_imports_from_import() {
    let source = "from os.path import join, exists\n";
    let result = parse(source);
    let ni = &result.metadata.named_imports;

    assert_eq!(
        ni.get("os.path").map(|v| v.as_slice()),
        Some(vec!["exists".to_string(), "join".to_string()].as_slice()),
        "from import should populate named_imports; got: {:?}",
        ni
    );
}

#[test]
fn named_imports_aliased() {
    let source = "from collections import OrderedDict as OD, defaultdict\n";
    let result = parse(source);
    let names = result
        .metadata
        .named_imports
        .get("collections")
        .expect("should have collections key");

    assert!(
        names.contains(&"OrderedDict".to_string()),
        "alias should store original name; got: {:?}",
        names
    );
    assert!(
        names.contains(&"defaultdict".to_string()),
        "non aliased import should be captured; got: {:?}",
        names
    );
}

#[test]
fn named_imports_wildcard_goes_to_namespace() {
    let source = "from typing import *\n";
    let result = parse(source);

    assert!(
        result.metadata.named_imports.is_empty(),
        "wildcard should not go to named_imports"
    );
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"typing".to_string()),
        "from import wildcard should go to namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn namespace_imports_bare_import() {
    let source = "import os\nimport sys\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"os".to_string()),
        "import module should populate namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"sys".to_string()),
        "import module should populate namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn namespace_imports_aliased_import() {
    let source = "import numpy as np\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"numpy".to_string()),
        "import alias should store original module; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn named_imports_relative() {
    let source = "from .utils import helper\nfrom ..config import Settings\n";
    let result = parse(source);
    let ni = &result.metadata.named_imports;

    assert!(
        ni.contains_key(".utils"),
        "relative import key should use raw dot notation; got keys: {:?}",
        ni.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        ni.get(".utils").map(|v| v.as_slice()),
        Some(vec!["helper".to_string()].as_slice()),
    );
    assert!(
        ni.contains_key("..config"),
        "double dot relative import key missing; got keys: {:?}",
        ni.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        ni.get("..config").map(|v| v.as_slice()),
        Some(vec!["Settings".to_string()].as_slice()),
    );
}

#[test]
fn named_imports_mixed_forms() {
    let source = "\
import json
from typing import List, Optional
from .models import User
from pathlib import *
import os.path as osp
";
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    let ns = &result.metadata.namespace_imports;

    assert!(ni.contains_key("typing"), "typing key missing: {:?}", ni);
    assert!(ni.contains_key(".models"), ".models key missing: {:?}", ni);
    assert!(
        ns.contains(&"json".to_string()),
        "json should be in namespace_imports; got: {:?}",
        ns
    );
    assert!(
        ns.contains(&"pathlib".to_string()),
        "pathlib wildcard should be in namespace_imports; got: {:?}",
        ns
    );
    assert!(
        ns.contains(&"os.path".to_string()),
        "aliased dotted import should be in namespace_imports; got: {:?}",
        ns
    );
}
