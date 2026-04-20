use super::super::dot_import_to_path;
use super::support::parse;

#[test]
fn parse_python_imports() {
    let source = "import os\nimport json\nfrom pathlib import Path\nfrom .utils import helper\n";
    let result = parse(source);

    assert!(result.metadata.imports.contains(&"os".to_string()));
    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(result.metadata.imports.contains(&"pathlib".to_string()));
    assert!(!result.metadata.imports.contains(&".utils".to_string()));
}

#[test]
fn parse_python_relative_deps() {
    let source = "from .utils import helper\nfrom ..models import User\n";
    let result = parse(source);
    let deps = &result.metadata.dependencies;

    assert!(
        deps.contains(&"./utils".to_string()),
        "expected ./utils in {:?}",
        deps
    );
    assert!(
        deps.contains(&"../models".to_string()),
        "expected ../models in {:?}",
        deps
    );
}

#[test]
fn dot_import_to_path_conversions() {
    assert_eq!(dot_import_to_path(".utils"), "./utils");
    assert_eq!(dot_import_to_path("..models"), "../models");
    assert_eq!(dot_import_to_path("...deep.sub"), "../../deep/sub");
    assert_eq!(dot_import_to_path("."), "./");
}

#[test]
fn parse_python_aliased_import() {
    let source = "import pandas as pd\nimport numpy as np\nimport os\n";
    let result = parse(source);

    assert!(result.metadata.imports.contains(&"pandas".to_string()));
    assert!(result.metadata.imports.contains(&"numpy".to_string()));
    assert!(result.metadata.imports.contains(&"os".to_string()));
}

#[test]
fn parse_python_dotted_imports_full_path() {
    let source = "from agno.models.message import Message\nfrom agno.tools.function import Function\nimport os\n";
    let result = parse(source);

    assert!(
        result
            .metadata
            .imports
            .contains(&"agno.models.message".to_string()),
        "expected full dotted path, got: {:?}",
        result.metadata.imports
    );
    assert!(
        result
            .metadata
            .imports
            .contains(&"agno.tools.function".to_string()),
        "expected full dotted path, got: {:?}",
        result.metadata.imports
    );
    assert!(result.metadata.imports.contains(&"os".to_string()));
    assert_eq!(
        result
            .metadata
            .imports
            .iter()
            .filter(|i| i.as_str() == "agno.models.message")
            .count(),
        1
    );
}
