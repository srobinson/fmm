use super::super::tsconfig::{match_alias, strip_json_comments};

#[test]
fn strip_json_comments_basic() {
    let input = r#"{ // a comment
  "key": "value" // inline comment
}"#;
    let stripped = strip_json_comments(input);
    let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
    assert_eq!(parsed["key"], "value");
}

#[test]
fn match_alias_wildcard() {
    let targets = vec!["src/*".to_string()];
    assert_eq!(
        match_alias("@/utils/helper", "@/*", &targets),
        Some("src/utils/helper".to_string())
    );
    assert_eq!(match_alias("@nestjs/common", "@/*", &targets), None);
}
