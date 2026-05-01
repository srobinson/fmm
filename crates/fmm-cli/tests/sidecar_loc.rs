use std::fs;

#[test]
fn sidecar_entrypoint_stays_under_extraction_limit() {
    let sidecar = format!("{}/src/cli/sidecar.rs", env!("CARGO_MANIFEST_DIR"));
    let contents = fs::read_to_string(&sidecar).unwrap();
    let line_count = contents.lines().count();

    assert!(
        line_count < 400,
        "src/cli/sidecar.rs has {line_count} lines; extract helpers before adding more"
    );
}
