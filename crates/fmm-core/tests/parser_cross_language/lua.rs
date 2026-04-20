use crate::support::parse_with;
use fmm_core::parser::builtin::lua::LuaParser;

// Lua validation

/// Neovim plugin pattern — typical Lua plugin module for Neovim.
/// Inspired by common Neovim plugin structure (telescope, nvim-cmp, etc.).
#[test]
fn lua_real_neovim_plugin_pattern() {
    let source = include_str!("fixtures/lua/lua_real_neovim_plugin_pattern.lua");
    let result = parse_with(LuaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Module methods
    assert!(names.contains(&"setup".to_string()));
    assert!(names.contains(&"run".to_string()));
    assert!(names.contains(&"get_status".to_string()));

    // Local functions excluded
    assert!(!names.contains(&"validate_opts".to_string()));
    assert!(!names.contains(&"apply_highlights".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"vim.api".to_string()));
    assert!(result.metadata.imports.contains(&"vim.fn".to_string()));

    // Dependencies
    assert!(
        result
            .metadata
            .dependencies
            .contains(&"./config".to_string())
    );
}

// =============================================================================
// Lua validation — Love2D game pattern
// =============================================================================

/// Love2D game pattern — typical Love2D game module with global callbacks.
/// Inspired by Love2D game structure.
#[test]
fn lua_real_love2d_game_pattern() {
    let source = include_str!("fixtures/lua/lua_real_love2d_game_pattern.lua");
    let result = parse_with(LuaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Global functions (Love2D callbacks)
    assert!(names.contains(&"love_load".to_string()));
    assert!(names.contains(&"love_update".to_string()));
    assert!(names.contains(&"love_draw".to_string()));

    // Local functions excluded
    assert!(!names.contains(&"reset_player".to_string()));
    assert!(!names.contains(&"check_bounds".to_string()));

    // Imports
    assert!(
        result
            .metadata
            .imports
            .contains(&"love.physics".to_string())
    );
    assert!(
        result
            .metadata
            .imports
            .contains(&"love.graphics".to_string())
    );

    // No dependencies
    assert!(result.metadata.dependencies.is_empty());
}
