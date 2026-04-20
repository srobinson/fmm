use super::support::parse;

#[test]
fn nested_fn_extracted_from_exported_function() {
    let source = r#"
export function createTypeChecker(host: any): any {
  var silentNeverType = createIntrinsicType(TypeFlags.Never, "never");
  function getIndexType(type: any): any { return undefined; }
  function getReturnType(sig: any): any { return undefined; }
  return {};
}
"#;
    let result = parse(source);
    let nested: Vec<_> = result
        .metadata
        .exports
        .iter()
        .filter(|e| e.parent_class.as_deref() == Some("createTypeChecker"))
        .collect();
    let names: Vec<&str> = nested.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.contains(&"getIndexType"),
        "getIndexType missing; names={:?}",
        names
    );
    assert!(
        names.contains(&"getReturnType"),
        "getReturnType missing; names={:?}",
        names
    );
    assert!(
        names.contains(&"silentNeverType"),
        "silentNeverType missing; names={:?}",
        names
    );
}

#[test]
fn nested_fn_has_correct_kind() {
    let source = r#"
export function outer(): void {
  var state = createState();
  function inner(): void {}
}
"#;
    let result = parse(source);
    let inner_entry = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "inner")
        .expect("inner not found");
    assert_eq!(inner_entry.kind.as_deref(), Some("nested-fn"));
    assert_eq!(inner_entry.parent_class.as_deref(), Some("outer"));

    let state_entry = result
        .metadata
        .exports
        .iter()
        .find(|e| e.name == "state")
        .expect("state (closure-state) not found");
    assert_eq!(state_entry.kind.as_deref(), Some("closure-state"));
}

#[test]
fn trivial_var_not_extracted_as_closure_state() {
    let source = r#"
export function outer(): void {
  let counter = 0;
  var flag = false;
  function inner(): void {}
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        !names.contains(&"counter"),
        "trivial counter should not be extracted"
    );
    assert!(
        !names.contains(&"flag"),
        "trivial flag should not be extracted"
    );
}

#[test]
fn depth2_nested_fn_not_extracted() {
    let source = r#"
export function outer(): void {
  function depth1(): void {
function depth2(): void {}
  }
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&"depth2"), "depth2 should not be extracted");
    assert!(names.contains(&"depth1"), "depth1 should be extracted");
}

#[test]
fn prologue_var_after_first_nested_fn_not_extracted() {
    let source = r#"
export function outer(): void {
  var before = createA();
  function inner(): void {}
  var after = createB();
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        names.contains(&"before"),
        "before (prologue) should be extracted"
    );
    assert!(
        !names.contains(&"after"),
        "after (post-first-fn) should not be extracted"
    );
}

#[test]
fn nested_symbols_in_non_exported_function() {
    let source = r#"
function internalHelper(): void {
  var state = createState();
  function processItem(item: any): void {}
}
"#;
    let result = parse(source);
    let names: Vec<&str> = result
        .metadata
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        names.contains(&"processItem"),
        "processItem should be extracted"
    );
    assert!(
        names.contains(&"state"),
        "state closure-state should be extracted"
    );
}
