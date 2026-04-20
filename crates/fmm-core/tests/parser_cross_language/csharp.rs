use crate::support::parse_with;
use fmm_core::parser::builtin::csharp::CSharpParser;

// C# validation

/// ASP.NET style service with DI and async
#[test]
fn csharp_real_repo_aspnet_service() {
    let source = include_str!("fixtures/csharp/csharp_real_repo_aspnet_service.cs");
    let result = parse_with(CSharpParser::new().unwrap(), source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"IUserService".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"UserService".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"GetUserAsync".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Delete".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"Validate".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"CacheHelper".to_string())
    );
}
