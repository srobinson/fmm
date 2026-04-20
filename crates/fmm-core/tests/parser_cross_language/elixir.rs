use crate::support::parse_with;
use fmm_core::parser::builtin::elixir::ElixirParser;

// Elixir validation

#[test]
fn elixir_real_genserver_pattern() {
    // Pattern: GenServer with public API and private callbacks
    let source = include_str!("fixtures/elixir/elixir_real_genserver_pattern.ex");
    let result = parse_with(ElixirParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    // Module and public API
    assert!(names.contains(&"MyApp.Cache".to_string()));
    assert!(names.contains(&"start_link".to_string()));
    assert!(names.contains(&"get".to_string()));
    assert!(names.contains(&"put".to_string()));

    // Private callbacks excluded
    assert!(!names.contains(&"init".to_string()));
    assert!(!names.contains(&"handle_call".to_string()));
    assert!(!names.contains(&"handle_cast".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"GenServer".to_string()));
}

#[test]
fn elixir_real_phoenix_context_pattern() {
    // Pattern: Phoenix context module with Ecto queries
    let source = include_str!("fixtures/elixir/elixir_real_phoenix_context_pattern.ex");
    let result = parse_with(ElixirParser::new().unwrap(), source);
    let names = result.metadata.export_names();

    assert!(names.contains(&"MyApp.Accounts".to_string()));
    assert!(names.contains(&"list_users".to_string()));
    assert!(names.contains(&"get_user!".to_string()));
    assert!(names.contains(&"create_user".to_string()));
    assert!(!names.contains(&"base_query".to_string()));

    assert!(result.metadata.imports.contains(&"Ecto".to_string()));
    assert!(result.metadata.imports.contains(&"MyApp".to_string()));
}
