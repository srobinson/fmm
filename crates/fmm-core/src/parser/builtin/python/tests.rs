use super::*;
use crate::parser::ExportEntry;

#[test]
fn parse_python_functions() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def hello():\n    pass\n\ndef world():\n    pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"hello".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"world".to_string())
    );
    assert_eq!(result.metadata.loc, 5);
}

#[test]
fn parse_python_classes() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class MyClass:\n    pass\n\nclass _Private:\n    pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"MyClass".to_string())
    );
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"_Private".to_string())
    );
}

#[test]
fn parse_python_imports() {
    let mut parser = PythonParser::new().unwrap();
    let source = "import os\nimport json\nfrom pathlib import Path\nfrom .utils import helper\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"os".to_string()));
    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(result.metadata.imports.contains(&"pathlib".to_string()));
    assert!(!result.metadata.imports.contains(&".utils".to_string()));
}

#[test]
fn parse_python_relative_deps() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from .utils import helper\nfrom ..models import User\n";
    let result = parser.parse(source).unwrap();
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
fn parse_python_private_excluded() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def _private():\n    pass\n\ndef public():\n    pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"_private".to_string())
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"public".to_string())
    );
}

#[test]
fn python_custom_fields_decorators() {
    let mut parser = PythonParser::new().unwrap();
    let source = "@staticmethod\ndef foo():\n    pass\n\n@property\ndef bar(self):\n    return 1\n";
    let result = parser.parse(source).unwrap();
    let fields = result.custom_fields.unwrap();
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"staticmethod"));
    assert!(names.contains(&"property"));
}

#[test]
fn python_no_custom_fields_when_no_decorators() {
    let mut parser = PythonParser::new().unwrap();
    // Use source with no functions and no decorators to get None custom_fields
    let source = "BAR = 42\n";
    let result = parser.parse(source).unwrap();
    assert!(result.custom_fields.is_none());
}

#[test]
fn parse_python_dunder_all() {
    let mut parser = PythonParser::new().unwrap();
    let source = r#"
__all__ = ["public_func", "PublicClass"]

def public_func():
    pass

def _private_func():
    pass

class PublicClass:
    pass

class _InternalClass:
    pass
"#;
    let result = parser.parse(source).unwrap();
    assert_eq!(
        result.metadata.export_names(),
        vec!["public_func", "PublicClass"]
    );
    // Verify exports resolve to definition sites, not __all__ line
    let exports = &result.metadata.exports;
    let func_export = exports.iter().find(|e| e.name == "public_func").unwrap();
    assert_eq!(func_export.start_line, 4);
    assert_eq!(func_export.end_line, 5);
    let class_export = exports.iter().find(|e| e.name == "PublicClass").unwrap();
    assert_eq!(class_export.start_line, 10);
    assert_eq!(class_export.end_line, 11);
}

#[test]
fn parse_python_aliased_import() {
    let mut parser = PythonParser::new().unwrap();
    let source = "import pandas as pd\nimport numpy as np\nimport os\n";
    let result = parser.parse(source).unwrap();
    assert!(result.metadata.imports.contains(&"pandas".to_string()));
    assert!(result.metadata.imports.contains(&"numpy".to_string()));
    assert!(result.metadata.imports.contains(&"os".to_string()));
}

#[test]
fn parse_python_decorated_class() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from dataclasses import dataclass\n\n@dataclass\nclass Agent:\n    name: str\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Agent".to_string())
    );
}

#[test]
fn parse_python_decorated_class_with_args() {
    let mut parser = PythonParser::new().unwrap();
    let source = "@dataclass(frozen=True)\nclass Config:\n    debug: bool = False\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"Config".to_string())
    );
}

#[test]
fn parse_python_decorated_function() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from flask import Flask\napp = Flask(__name__)\n\n@app.route(\"/\")\ndef handler():\n    return \"ok\"\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"handler".to_string())
    );
}

#[test]
fn parse_python_decorated_class_line_range() {
    let mut parser = PythonParser::new().unwrap();
    let source = "@dataclass\nclass Agent:\n    name: str\n    role: str\n";
    let result = parser.parse(source).unwrap();
    let agent = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "Agent")
        .expect("Agent should be exported");
    // Range should start at the decorator line (1), not the class line (2)
    assert_eq!(agent.start_line, 1);
    assert_eq!(agent.end_line, 4);
}

#[test]
fn parse_python_dunder_all_with_decorated_class() {
    let mut parser = PythonParser::new().unwrap();
    let source = r#"
from dataclasses import dataclass

__all__ = ["DecoratedModel", "bare_func"]

@dataclass
class DecoratedModel:
    id: int
    name: str

def bare_func():
    pass
"#;
    let result = parser.parse(source).unwrap();
    assert_eq!(
        result.metadata.export_names(),
        vec!["DecoratedModel", "bare_func"]
    );
    // DecoratedModel should resolve to the decorated_definition site, not __all__ line
    let model = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "DecoratedModel")
        .unwrap();
    assert_eq!(model.start_line, 6); // @dataclass line
    assert_eq!(model.end_line, 9);
}

#[test]
fn parse_python_dotted_imports_full_path() {
    // `from agno.models.message import Message` should store "agno.models.message",
    // not just the root "agno". Single-name imports are unaffected.
    let mut parser = PythonParser::new().unwrap();
    let source = "from agno.models.message import Message\nfrom agno.tools.function import Function\nimport os\n";
    let result = parser.parse(source).unwrap();
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
    // Single-name import unchanged
    assert!(result.metadata.imports.contains(&"os".to_string()));
    // Deduplicated: only one entry per unique dotted path
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

// ALP-769: public method extraction tests

fn get_method<'a>(
    exports: &'a [ExportEntry],
    class: &str,
    method: &str,
) -> Option<&'a ExportEntry> {
    exports
        .iter()
        .find(|e| e.parent_class.as_deref() == Some(class) && e.name == method)
}

#[test]
fn python_methods_public_included() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    def bar(self):\n        pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "bar").is_some(),
        "Foo.bar should be indexed"
    );
}

#[test]
fn python_methods_private_excluded() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    def _internal(self):\n        pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "_internal").is_none(),
        "Foo._internal should NOT be indexed"
    );
}

#[test]
fn python_methods_init_included() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    def __init__(self):\n        pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "__init__").is_some(),
        "Foo.__init__ should be indexed"
    );
}

#[test]
fn python_methods_other_dunder_excluded() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    def __str__(self):\n        return ''\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "__str__").is_none(),
        "Foo.__str__ should NOT be indexed"
    );
}

#[test]
fn python_methods_non_exported_class_excluded() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class _Internal:\n    def method(self):\n        pass\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "_Internal", "method").is_none(),
        "methods of non-exported class should NOT be indexed"
    );
}

#[test]
fn python_methods_decorated_included() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    @property\n    def value(self):\n        return self._value\n    @staticmethod\n    def create():\n        return Foo()\n";
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "Foo", "value").is_some(),
        "Foo.value (@property) should be indexed"
    );
    assert!(
        get_method(&result.metadata.exports, "Foo", "create").is_some(),
        "Foo.create (@staticmethod) should be indexed"
    );
}

#[test]
fn python_methods_decorated_line_range_includes_decorator() {
    let mut parser = PythonParser::new().unwrap();
    // line 1: class Foo:
    // line 2:     @property
    // line 3:     def value(self):
    // line 4:         return 1
    let source = "class Foo:\n    @property\n    def value(self):\n        return 1\n";
    let result = parser.parse(source).unwrap();
    let entry =
        get_method(&result.metadata.exports, "Foo", "value").expect("Foo.value should be indexed");
    assert_eq!(
        entry.start_line, 2,
        "start_line should be the decorator line"
    );
}

#[test]
fn python_methods_dunder_all_respects_export_list() {
    let mut parser = PythonParser::new().unwrap();
    let source = r#"
__all__ = ["PublicClass"]

class PublicClass:
    def method(self):
        pass

class HiddenClass:
    def method(self):
        pass
"#;
    let result = parser.parse(source).unwrap();
    assert!(
        get_method(&result.metadata.exports, "PublicClass", "method").is_some(),
        "PublicClass.method should be indexed"
    );
    assert!(
        get_method(&result.metadata.exports, "HiddenClass", "method").is_none(),
        "HiddenClass.method should NOT be indexed (not in __all__)"
    );
}

#[test]
fn parse_python_dunder_all_overrides_discovery() {
    let mut parser = PythonParser::new().unwrap();
    let source = r#"
__all__ = ["only_this"]

def only_this():
    pass

def also_public():
    pass
"#;
    let result = parser.parse(source).unwrap();
    assert_eq!(result.metadata.export_names(), vec!["only_this"]);
    assert!(
        !result
            .metadata
            .export_names()
            .contains(&"also_public".to_string())
    );
}

// -------------------------------------------------------------------------
// ALP-1418: Named import extraction
// -------------------------------------------------------------------------

#[test]
fn named_imports_from_import() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from os.path import join, exists\n";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert_eq!(
        ni.get("os.path").map(|v| v.as_slice()),
        Some(vec!["exists".to_string(), "join".to_string()].as_slice()),
        "from X import A, B -> named_imports; got: {:?}",
        ni
    );
}

#[test]
fn named_imports_aliased() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from collections import OrderedDict as OD, defaultdict\n";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    let names = ni
        .get("collections")
        .expect("should have 'collections' key");
    assert!(
        names.contains(&"OrderedDict".to_string()),
        "alias should store original name; got: {:?}",
        names
    );
    assert!(
        names.contains(&"defaultdict".to_string()),
        "non-aliased should be captured; got: {:?}",
        names
    );
}

#[test]
fn named_imports_wildcard_goes_to_namespace() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from typing import *\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result.metadata.named_imports.is_empty(),
        "wildcard should not go to named_imports"
    );
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"typing".to_string()),
        "from X import * -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn namespace_imports_bare_import() {
    let mut parser = PythonParser::new().unwrap();
    let source = "import os\nimport sys\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"os".to_string()),
        "import module -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"sys".to_string()),
        "import module -> namespace_imports; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn namespace_imports_aliased_import() {
    let mut parser = PythonParser::new().unwrap();
    let source = "import numpy as np\n";
    let result = parser.parse(source).unwrap();
    assert!(
        result
            .metadata
            .namespace_imports
            .contains(&"numpy".to_string()),
        "import X as Y -> namespace_imports stores original; got: {:?}",
        result.metadata.namespace_imports
    );
}

#[test]
fn named_imports_relative() {
    let mut parser = PythonParser::new().unwrap();
    let source = "from .utils import helper\nfrom ..config import Settings\n";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    assert!(
        ni.contains_key(".utils"),
        "relative import key should be raw dot notation; got keys: {:?}",
        ni.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        ni.get(".utils").map(|v| v.as_slice()),
        Some(vec!["helper".to_string()].as_slice()),
    );
    assert!(
        ni.contains_key("..config"),
        "double-dot relative; got keys: {:?}",
        ni.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        ni.get("..config").map(|v| v.as_slice()),
        Some(vec!["Settings".to_string()].as_slice()),
    );
}

#[test]
fn named_imports_mixed_forms() {
    let mut parser = PythonParser::new().unwrap();
    let source = "\
import json
from typing import List, Optional
from .models import User
from pathlib import *
import os.path as osp
";
    let result = parser.parse(source).unwrap();
    let ni = &result.metadata.named_imports;
    let ns = &result.metadata.namespace_imports;

    // Named: typing -> [List, Optional], .models -> [User]
    assert!(ni.contains_key("typing"), "typing key; got: {:?}", ni);
    assert!(ni.contains_key(".models"), ".models key; got: {:?}", ni);

    // Namespace: json, pathlib, os.path
    assert!(
        ns.contains(&"json".to_string()),
        "json in namespace; got: {:?}",
        ns
    );
    assert!(
        ns.contains(&"pathlib".to_string()),
        "pathlib wildcard in namespace; got: {:?}",
        ns
    );
    assert!(
        ns.contains(&"os.path".to_string()),
        "os.path aliased in namespace; got: {:?}",
        ns
    );
}

// --- ALP-1422: function_names custom field ---

#[test]
fn function_names_populated() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass\n";
    let result = parser.parse(source).unwrap();
    let cf = result.custom_fields.expect("custom_fields should be Some");
    let fn_names = cf.get("function_names").expect("function_names key");
    let names: Vec<&str> = fn_names
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(names.contains(&"foo"), "foo missing: {names:?}");
    assert!(names.contains(&"bar"), "bar missing: {names:?}");
    assert!(
        !names.contains(&"Baz"),
        "class should not be in function_names: {names:?}"
    );
}

#[test]
fn function_names_excludes_private() {
    let mut parser = PythonParser::new().unwrap();
    let source = "def public():\n    pass\n\ndef _private():\n    pass\n";
    let result = parser.parse(source).unwrap();
    let cf = result.custom_fields.expect("custom_fields should be Some");
    let fn_names = cf.get("function_names").expect("function_names key");
    let names: Vec<&str> = fn_names
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(names.contains(&"public"), "public missing: {names:?}");
    assert!(
        !names.contains(&"_private"),
        "_private should be excluded: {names:?}"
    );
}

#[test]
fn function_names_empty_for_no_functions() {
    let mut parser = PythonParser::new().unwrap();
    let source = "class Foo:\n    pass\n\nBAR = 42\n";
    let result = parser.parse(source).unwrap();
    // custom_fields may be None or may not have function_names
    let has_fn = result
        .custom_fields
        .as_ref()
        .and_then(|cf| cf.get("function_names"))
        .is_some();
    assert!(!has_fn, "no functions should mean no function_names key");
}
