use super::*;
use fmm_core::parser::{ParseResult, SymbolVisibility};

#[test]
fn validate_rust_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.rs"
    ));

    let result = parse_fixture(RustParser::new().unwrap(), source);

    let expected_exports = vec![
        "Config",
        "Status",
        "Pipeline",
        "Error",
        "impl fmt::Display for Error",
        "process",
        "internal_helper",
        "parent_visible",
        "fetch_remote",
        "private_fn",
    ];
    assert_eq!(result.metadata.export_names(), expected_exports);

    // Expected imports: anyhow, serde, std, tokio (all crates including stdlib)
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));

    // Expected dependencies: full paths, not bare root keywords
    let deps = &result.metadata.dependencies;
    assert!(
        deps.contains(&"crate::config".to_string()),
        "expected crate::config in {:?}",
        deps
    );
    assert!(
        deps.contains(&"../utils".to_string()),
        "expected ../utils in {:?}",
        deps
    );

    // LOC
    assert!(result.metadata.loc > 50);

    // Custom fields
    let fields = result
        .custom_fields
        .as_ref()
        .expect("should have custom fields");

    // unsafe_blocks: 1
    assert_eq!(
        fields.get("unsafe_blocks").unwrap().as_u64().unwrap(),
        1,
        "should have exactly 1 unsafe block"
    );

    // derives: Clone, Debug, Deserialize, Serialize
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let derive_names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(derive_names.contains(&"Debug"));
    assert!(derive_names.contains(&"Clone"));
    assert!(derive_names.contains(&"Serialize"));
    assert!(derive_names.contains(&"Deserialize"));

    // trait_impls: Display for Error
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let impl_names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(impl_names.contains(&"Display for Error"));

    // lifetimes: 'a, 'static
    let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
    let lt_names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(lt_names.contains(&"'a"));
    assert!(lt_names.contains(&"'static"));

    // async_functions: 1
    assert_eq!(
        fields.get("async_functions").unwrap().as_u64().unwrap(),
        1,
        "should have exactly 1 async function"
    );

    assert_visibility(&result, "process", SymbolVisibility::Public);
    assert_visibility(&result, "internal_helper", SymbolVisibility::Crate);
    assert_visibility(&result, "parent_visible", SymbolVisibility::Crate);
    assert_visibility(&result, "private_fn", SymbolVisibility::Private);
}

fn assert_visibility(result: &ParseResult, name: &str, visibility: SymbolVisibility) {
    let entry = result
        .metadata
        .exports
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} should be indexed"));
    assert_eq!(entry.visibility, Some(visibility));
}
