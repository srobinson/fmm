use super::support::parse_file;

#[test]
fn binary_main_exports_all_functions() {
    let source = r#"
fn main() {
    run();
}

fn run() {}

fn helper() -> i32 { 42 }

struct Config {
    name: String,
}

enum Mode { Fast, Slow }

const VERSION: &str = "1.0";
"#;
    let result = parse_file(source, "src/main.rs");
    let names = result.metadata.export_names();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"run".to_string()));
    assert!(names.contains(&"helper".to_string()));
    assert!(names.contains(&"Config".to_string()));
    assert!(names.contains(&"Mode".to_string()));
    assert!(names.contains(&"VERSION".to_string()));
}

#[test]
fn binary_bin_dir_exports_all_functions() {
    let source = "fn main() {}\nfn setup() {}";
    let result = parse_file(source, "src/bin/tool.rs");
    let names = result.metadata.export_names();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"setup".to_string()));
}

#[test]
fn lib_still_requires_pub() {
    let source = "pub fn visible() {}\nfn private() {}\npub struct Exported {}\nstruct Hidden {}";
    let result = parse_file(source, "src/lib.rs");
    let names = result.metadata.export_names();
    assert!(names.contains(&"visible".to_string()));
    assert!(names.contains(&"Exported".to_string()));
    assert!(!names.contains(&"private".to_string()));
    assert!(!names.contains(&"Hidden".to_string()));
}
