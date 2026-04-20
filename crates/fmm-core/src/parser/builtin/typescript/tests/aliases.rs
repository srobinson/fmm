use std::collections::HashMap;

use super::support::{parse, parse_with_aliases};

#[test]
fn alias_wildcard_classified_as_dependency() {
    let mut aliases = HashMap::new();
    aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { helper } from "@/utils/helper";"#;
    let result = parse_with_aliases(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/utils/helper".to_string()),
        "alias import should be a dependency, got: {:?}",
        result.metadata.dependencies
    );
    assert!(
        !result
            .metadata
            .imports
            .contains(&"@/utils/helper".to_string()),
        "alias import must not appear in imports, got: {:?}",
        result.metadata.imports
    );
}

#[test]
fn scoped_package_without_alias_stays_external() {
    let mut aliases = HashMap::new();
    aliases.insert("@/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { Injectable } from "@nestjs/common";"#;
    let result = parse_with_aliases(source, aliases);
    assert!(
        result
            .metadata
            .imports
            .contains(&"@nestjs/common".to_string()),
        "@nestjs/common must stay in imports, got: {:?}",
        result.metadata.imports
    );
    assert!(
        result.metadata.dependencies.is_empty(),
        "no deps expected, got: {:?}",
        result.metadata.dependencies
    );
}

#[test]
fn no_aliases_falls_back_to_heuristic() {
    let source = r#"import { x } from "@/utils/helper";"#;
    let result = parse(source);
    assert!(
        result
            .metadata
            .imports
            .contains(&"@/utils/helper".to_string()),
        "without aliases, @/ import should stay in imports, got: {:?}",
        result.metadata.imports
    );
}

#[test]
fn alias_tilde_pattern() {
    let mut aliases = HashMap::new();
    aliases.insert("~/*".to_string(), vec!["src/*".to_string()]);
    let source = r#"import { config } from "~/config/app";"#;
    let result = parse_with_aliases(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/config/app".to_string()),
        "tilde alias should resolve to dependency, got: {:?}",
        result.metadata.dependencies
    );
}

#[test]
fn alias_exact_pattern() {
    let mut aliases = HashMap::new();
    aliases.insert("@app".to_string(), vec!["src/app".to_string()]);
    let source = r#"import App from "@app";"#;
    let result = parse_with_aliases(source, aliases);
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"src/app".to_string()),
        "exact alias should resolve, got: {:?}",
        result.metadata.dependencies
    );
}
