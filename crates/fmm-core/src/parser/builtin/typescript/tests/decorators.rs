use super::support::parse;

fn decorator_names(source: &str) -> Vec<String> {
    let result = parse(source);
    let fields = result.custom_fields.expect("should have custom_fields");

    fields["decorators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn decorator_simple_captured() {
    let source = r#"
@Component
export class AppComponent {}
"#;
    assert!(decorator_names(source).contains(&"Component".to_string()));
}

#[test]
fn decorator_call_expression_captured() {
    let source = r#"
@Injectable()
export class UserService {}
"#;
    assert!(decorator_names(source).contains(&"Injectable".to_string()));
}

#[test]
fn decorator_multiple_unique() {
    let source = r#"
@Controller('/users')
export class UserController {}

@Injectable()
export class AuthService {}
"#;
    let decorators = decorator_names(source);
    assert!(decorators.contains(&"Controller".to_string()));
    assert!(decorators.contains(&"Injectable".to_string()));
}

#[test]
fn no_decorators_custom_fields_none() {
    let result = parse("export class Plain {}");
    assert!(result.custom_fields.is_none());
}
