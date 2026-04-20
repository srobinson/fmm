use super::support::parse;

#[test]
fn named_imports_basic() {
    let result = parse("import { foo, bar } from './mod';");
    let ni = &result.metadata.named_imports;
    assert_eq!(ni.get("./mod").unwrap(), &vec!["bar", "foo"]);
}

#[test]
fn named_imports_aliased_stores_original_name() {
    let result = parse("import { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(
        names.contains(&"scheduleUpdateOnFiber".to_string()),
        "should store original name"
    );
    assert!(
        !names.contains(&"schedule".to_string()),
        "should not store alias"
    );
}

#[test]
fn named_imports_default_import_not_included() {
    let result = parse("import React from 'react';");
    assert!(
        result.metadata.named_imports.is_empty(),
        "default imports should not appear in named_imports"
    );
}

#[test]
fn namespace_imports_captured() {
    let result = parse("import * as NS from './module';");
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"./module".to_string())
    );
    assert!(
        result.metadata.named_imports.is_empty(),
        "namespace import should not populate named_imports"
    );
}

#[test]
fn named_reexports_captured() {
    let result = parse("export { scheduleUpdateOnFiber } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
}

#[test]
fn wildcard_reexport_goes_to_namespace_imports() {
    let result = parse("export * from './utils';");
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"./utils".to_string())
    );
}

#[test]
fn type_only_import_included_in_named_imports() {
    let result = parse("import type { Foo } from './types';");
    let ni = &result.metadata.named_imports;
    assert!(
        ni.contains_key("./types"),
        "type-only import should be included"
    );
    assert!(ni["./types"].contains(&"Foo".to_string()));
}

#[test]
fn named_imports_multiple_sources() {
    let source = r#"
import { a, b } from './mod-a';
import { c } from './mod-b';
"#;
    let result = parse(source);
    let ni = &result.metadata.named_imports;
    assert_eq!(ni["./mod-a"], vec!["a", "b"]);
    assert_eq!(ni["./mod-b"], vec!["c"]);
}

#[test]
fn named_reexport_aliased_stores_original_name() {
    let result = parse("export { scheduleUpdateOnFiber as schedule } from './ReactFiberWorkLoop';");
    let ni = &result.metadata.named_imports;
    let names = ni.get("./ReactFiberWorkLoop").unwrap();
    assert!(names.contains(&"scheduleUpdateOnFiber".to_string()));
    assert!(!names.contains(&"schedule".to_string()));
}
