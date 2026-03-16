//! Tests for call-site detection.

use crate::manifest::call_site_finder::{
    find_bare_function_callers, find_call_sites, is_valid_identifier,
};
use std::io::Write;
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    name.to_string()
}

#[test]
fn ts_caller_is_included() {
    let dir = TempDir::new().unwrap();
    let caller = write_file(
        &dir,
        "caller.ts",
        "import { Foo } from './foo';\nconst f = new Foo();\nf.doThing();\n",
    );
    let bystander = write_file(
        &dir,
        "bystander.ts",
        "import { Foo } from './foo';\nconst f = new Foo();\n// never calls doThing\n",
    );
    let result = find_call_sites(dir.path(), "doThing", &[caller.clone(), bystander.clone()]);
    assert!(result.contains(&caller), "caller should be included");
    assert!(
        !result.contains(&bystander),
        "bystander should be excluded; got: {:?}",
        result
    );
}

#[test]
fn py_caller_is_included() {
    let dir = TempDir::new().unwrap();
    let caller = write_file(
        &dir,
        "caller.py",
        "from foo import Foo\nf = Foo()\nf.do_thing()\n",
    );
    let bystander = write_file(
        &dir,
        "bystander.py",
        "from foo import Foo\nf = Foo()\n# no call\n",
    );
    let result = find_call_sites(dir.path(), "do_thing", &[caller.clone(), bystander.clone()]);
    assert!(result.contains(&caller), "py caller included");
    assert!(!result.contains(&bystander), "py bystander excluded");
}

#[test]
fn rs_caller_is_included() {
    let dir = TempDir::new().unwrap();
    let caller = write_file(
        &dir,
        "caller.rs",
        "fn main() { let f = Foo::new(); f.do_thing(); }\n",
    );
    let bystander = write_file(
        &dir,
        "bystander.rs",
        "fn main() { let f = Foo::new(); /* no do_thing */ }\n",
    );
    let result = find_call_sites(dir.path(), "do_thing", &[caller.clone(), bystander.clone()]);
    assert!(result.contains(&caller), "rs caller included");
    assert!(!result.contains(&bystander), "rs bystander excluded");
}

#[test]
fn unreadable_file_is_included() {
    let dir = TempDir::new().unwrap();
    let ghost = "ghost.ts".to_string();
    let result = find_call_sites(dir.path(), "someMethod", std::slice::from_ref(&ghost));
    assert!(
        result.contains(&ghost),
        "unreadable file included by fallback"
    );
}

#[test]
fn unsupported_extension_is_included() {
    let dir = TempDir::new().unwrap();
    let f = write_file(&dir, "module.go", "package main\nfunc main() {}\n");
    let result = find_call_sites(dir.path(), "someMethod", std::slice::from_ref(&f));
    assert!(result.contains(&f), "unsupported ext included by fallback");
}

#[test]
fn invalid_identifier_returns_all_candidates() {
    let dir = TempDir::new().unwrap();
    let f = write_file(&dir, "some.ts", "const x = 1;\n");
    let result = find_call_sites(dir.path(), "bad\"name", std::slice::from_ref(&f));
    assert!(
        result.contains(&f),
        "invalid identifier falls back to include-all"
    );
}

#[test]
fn is_valid_identifier_accepts_common_names() {
    assert!(is_valid_identifier("doThing"));
    assert!(is_valid_identifier("_private"));
    assert!(is_valid_identifier("camelCase123"));
}

#[test]
fn is_valid_identifier_rejects_bad_input() {
    assert!(!is_valid_identifier(""));
    assert!(!is_valid_identifier("123abc"));
    assert!(!is_valid_identifier("has\"quote"));
    assert!(!is_valid_identifier("has.dot"));
    assert!(!is_valid_identifier("has space"));
}

// --- ALP-866: bare function call-site tests ---

/// Fixture 1: direct call `scheduleUpdate()` -- must appear in confirmed callers.
#[test]
fn bare_fn_direct_call_is_confirmed() {
    let dir = TempDir::new().unwrap();
    let caller = write_file(
        &dir,
        "caller.ts",
        "import { scheduleUpdate } from './scheduler';\nscheduleUpdate();\n",
    );
    let bystander = write_file(
        &dir,
        "bystander.ts",
        "import { scheduleUpdate } from './scheduler';\n// never calls it\nconst x = 1;\n",
    );
    let (confirmed, ns) = find_bare_function_callers(
        dir.path(),
        "scheduleUpdate",
        &[caller.clone(), bystander.clone()],
    );
    assert!(
        confirmed.contains(&caller),
        "direct caller should be confirmed"
    );
    assert!(
        !confirmed.contains(&bystander),
        "non-caller should be excluded"
    );
    assert!(ns.is_empty(), "no namespace callers expected");
}

/// Fixture 2: `import { scheduleUpdate as su }` + calls `su()` -- must appear.
#[test]
fn bare_fn_aliased_import_is_resolved() {
    let dir = TempDir::new().unwrap();
    let aliased = write_file(
        &dir,
        "aliased.ts",
        "import { scheduleUpdate as su } from './scheduler';\nsu();\n",
    );
    let (confirmed, ns) =
        find_bare_function_callers(dir.path(), "scheduleUpdate", std::slice::from_ref(&aliased));
    assert!(
        confirmed.contains(&aliased),
        "aliased caller should be confirmed"
    );
    assert!(ns.is_empty());
}

/// Fixture 3: `import * as wl` -- must appear as namespace caller.
#[test]
fn bare_fn_namespace_import_becomes_namespace_caller() {
    let dir = TempDir::new().unwrap();
    let ns_file = write_file(
        &dir,
        "ns_user.ts",
        "import * as wl from './scheduler';\nwl.scheduleUpdate();\n",
    );
    let (confirmed, ns) =
        find_bare_function_callers(dir.path(), "scheduleUpdate", std::slice::from_ref(&ns_file));
    assert!(
        !confirmed.contains(&ns_file),
        "namespace user should NOT be in confirmed"
    );
    assert!(
        ns.iter().any(|(f, _)| f == &ns_file),
        "namespace user should be in ns_callers"
    );
    let ns_name = ns
        .iter()
        .find(|(f, _)| f == &ns_file)
        .map(|(_, n)| n.as_str())
        .unwrap_or("");
    assert_eq!(ns_name, "wl", "namespace name should be 'wl'");
}

/// Fixture 4: imports but never calls -- must NOT appear.
#[test]
fn bare_fn_import_without_call_is_excluded() {
    let dir = TempDir::new().unwrap();
    let importer = write_file(
        &dir,
        "importer.ts",
        "import { scheduleUpdate } from './scheduler';\n// never calls scheduleUpdate\nconst x = 42;\n",
    );
    let (confirmed, ns) = find_bare_function_callers(
        dir.path(),
        "scheduleUpdate",
        std::slice::from_ref(&importer),
    );
    assert!(
        !confirmed.contains(&importer),
        "importer-without-call should be excluded"
    );
    assert!(ns.is_empty());
}

/// Fixture 5: re-exports but doesn't call -- must NOT appear.
#[test]
fn bare_fn_reexport_without_call_is_excluded() {
    let dir = TempDir::new().unwrap();
    let reexporter = write_file(
        &dir,
        "index.ts",
        "export { scheduleUpdate } from './scheduler';\n",
    );
    let (confirmed, ns) = find_bare_function_callers(
        dir.path(),
        "scheduleUpdate",
        std::slice::from_ref(&reexporter),
    );
    assert!(
        !confirmed.contains(&reexporter),
        "re-exporter should be excluded"
    );
    assert!(ns.is_empty());
}

/// A file importing a different function from same module should be excluded.
#[test]
fn bare_fn_unrelated_import_from_same_module_is_excluded() {
    let dir = TempDir::new().unwrap();
    let other = write_file(
        &dir,
        "other.ts",
        "import { otherFn } from './scheduler';\notherFn();\n",
    );
    let (confirmed, ns) =
        find_bare_function_callers(dir.path(), "scheduleUpdate", std::slice::from_ref(&other));
    assert!(
        !confirmed.contains(&other),
        "unrelated importer should be excluded"
    );
    assert!(ns.is_empty());
}
