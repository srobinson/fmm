use super::support::parse;

#[test]
fn python_custom_fields_decorators() {
    let source = "@staticmethod\ndef foo():\n    pass\n\n@property\ndef bar(self):\n    return 1\n";
    let result = parse(source);
    let fields = result.custom_fields.unwrap();
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();

    assert!(names.contains(&"staticmethod"));
    assert!(names.contains(&"property"));
}

#[test]
fn python_no_custom_fields_when_no_decorators() {
    let source = "BAR = 42\n";
    let result = parse(source);

    assert!(result.custom_fields.is_none());
}

#[test]
fn parse_python_decorated_class() {
    let source = "from dataclasses import dataclass\n\n@dataclass\nclass Agent:\n    name: str\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Agent".to_string())
    );
}

#[test]
fn parse_python_decorated_class_with_args() {
    let source = "@dataclass(frozen=True)\nclass Config:\n    debug: bool = False\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Config".to_string())
    );
}

#[test]
fn parse_python_decorated_function() {
    let source = "from flask import Flask\napp = Flask(__name__)\n\n@app.route(\"/\")\ndef handler():\n    return \"ok\"\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .export_names()
            .contains(&"handler".to_string())
    );
}

#[test]
fn parse_python_decorated_class_line_range() {
    let source = "@dataclass\nclass Agent:\n    name: str\n    role: str\n";
    let result = parse(source);
    let agent = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "Agent")
        .expect("Agent should be exported");

    assert_eq!(agent.start_line, 1);
    assert_eq!(agent.end_line, 4);
}
