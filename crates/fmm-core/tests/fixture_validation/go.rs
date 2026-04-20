use super::*;

#[test]
fn validate_go_fixture() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sample.go"
    ));

    let result = parse_fixture(GoParser::new().unwrap(), source);

    // Exported: capitalized names only (StatusActive, StatusInactive are iota consts)
    let expected_exports = vec![
        "MaxRetries",
        "Status",
        "StatusActive",
        "StatusInactive",
        "Config",
        "Handler",
        "NewHandler",
        "Process",
    ];
    assert_eq!(result.metadata.export_names(), expected_exports);

    // Imports: stdlib packages
    assert!(
        result
            .metadata
            .imports
            .contains(&"encoding/json".to_string())
    );
    assert!(result.metadata.imports.contains(&"fmt".to_string()));
    assert!(result.metadata.imports.contains(&"net/http".to_string()));

    // Dependencies: external modules (contain dots)
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"github.com/gin-gonic/gin".to_string())
    );
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"github.com/redis/go-redis/v9".to_string())
    );

    // Non-exported items should not be in exports
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"internalTimeout".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"privateState".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"helperFunc".to_string())
    );

    assert!(result.metadata.loc > 50);
}
