use crate::support::parse_with;
use fmm_core::parser::builtin::go::GoParser;

// Go validation

/// Standard Go HTTP handler pattern with exported types and functions
#[test]
fn go_real_repo_http_handler() {
    let source = include_str!("fixtures/go/go_real_repo_http_handler.go");
    let result = parse_with(GoParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Response".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Handler".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"NewHandler".to_string())
    );
    // healthCheck is unexported (lowercase)
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"healthCheck".to_string())
    );

    assert!(
        result
            .metadata
            .imports
            .contains(&"encoding/json".to_string())
    );
    assert!(result.metadata.imports.contains(&"net/http".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));
}

/// Go interface pattern with multiple exported types
#[test]
fn go_real_repo_interface_pattern() {
    let source = include_str!("fixtures/go/go_real_repo_interface_pattern.go");
    let result = parse_with(GoParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Store".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"PostgresStore".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"NewPostgresStore".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"cacheEntry".to_string())
    );

    assert!(result.metadata.imports.contains(&"context".to_string()));
    assert!(result.metadata.imports.contains(&"time".to_string()));
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"github.com/jackc/pgx/v5/pgxpool".to_string())
    );
}
