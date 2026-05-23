use super::{run_fmm, setup_large_class_project, setup_scopepath_collision_project, write_file};
use tempfile::TempDir;

fn setup_member_error_project() -> TempDir {
    setup_member_error_project_with("")
}

fn setup_member_error_project_with(extra_classes: &str) -> TempDir {
    let source = format!(
        "export class SpawnCoordinator {{\n  private pending_launches: Map<string, string> = new Map();\n  private pending_ready: Map<string, string> = new Map();\n\n  begin_spawn(): void {{}}\n  validate_spawn_target(): void {{}}\n  validate_target_request(): void {{}}\n  validate_target(): void {{}}\n  begin_ready_wait(): void {{}}\n  cancel_spawn(): void {{}}\n  take_launch_spec(): void {{}}\n  complete_shim_ready(): void {{}}\n  record_running(): void {{}}\n  record_reconnected_ready(): void {{}}\n  pending_shim_socket_count(): number {{ return 0; }}\n}}\n\nexport class ServerState {{\n  spawn: SpawnCoordinator;\n}}\n\nexport class FieldOnly {{\n  private client: string = '';\n}}\n\nexport class MethodOnly {{\n  run(): void {{}}\n  reset(): void {{}}\n}}\n{extra_classes}\n"
    );
    setup_member_error_source(&source)
}

fn setup_member_error_source(source: &str) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_file(root, "src/members.ts", source);

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    tmp
}

#[test]
fn missing_member_lists_fields_methods_and_substring_suggestions() {
    let tmp = setup_member_error_project();
    let output = run_fmm(tmp.path(), &["read", "SpawnCoordinator.spawn"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Member 'SpawnCoordinator.spawn' not found"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("'spawn' is not a member of 'SpawnCoordinator'"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Did you mean: begin_spawn, cancel_spawn, validate_spawn_target?"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Cross-type: ServerState.spawn (field of type SpawnCoordinator)."),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("Fields: pending_launches, pending_ready"),
        "got: {stderr}"
    );
    assert!(stderr.contains("Methods:"), "got: {stderr}");
    assert!(stderr.contains("begin_spawn"), "got: {stderr}");
    assert!(
        stderr.contains("pending_shim_socket_count"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains(
            "(13 members total; use fmm outline src/members.ts --include-private for full list.)"
        ),
        "got: {stderr}"
    );
    assert!(
        !stderr.contains("public or private method"),
        "got: {stderr}"
    );
    assert!(!stderr.contains("fmm_file_outline"), "got: {stderr}");
}

#[test]
fn missing_member_field_only_omits_methods_and_uses_distance_suggestion() {
    let tmp = setup_member_error_project();
    let output = run_fmm(tmp.path(), &["read", "FieldOnly.clint"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Did you mean: client?"), "got: {stderr}");
    assert!(stderr.contains("Fields: client"), "got: {stderr}");
    assert!(!stderr.contains("Methods:"), "got: {stderr}");
    assert!(!stderr.contains("Cross-type:"), "got: {stderr}");
    assert!(
        stderr.contains(
            "(1 member total; use fmm outline src/members.ts --include-private for full list.)"
        ),
        "got: {stderr}"
    );
}

#[test]
fn missing_member_method_only_omits_fields() {
    let tmp = setup_member_error_project();
    let output = run_fmm(tmp.path(), &["read", "MethodOnly.rn"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Did you mean: run?"), "got: {stderr}");
    assert!(stderr.contains("Methods: run, reset"), "got: {stderr}");
    assert!(!stderr.contains("Fields:"), "got: {stderr}");
}

#[test]
fn missing_member_cross_type_suggestions_are_capped() {
    let tmp = setup_member_error_project_with(
        "export class AlphaState {\n  spawn: SpawnCoordinator;\n}\n\nexport class BetaState {\n  spawn: SpawnCoordinator;\n}\n\nexport class GammaState {\n  spawn: SpawnCoordinator;\n}\n",
    );
    let output = run_fmm(tmp.path(), &["read", "SpawnCoordinator.spawn"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Cross-type: AlphaState.spawn (field of type SpawnCoordinator)"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("BetaState.spawn (field of type SpawnCoordinator)"),
        "got: {stderr}"
    );
    assert!(!stderr.contains("GammaState"), "got: {stderr}");
    assert!(!stderr.contains("ServerState"), "got: {stderr}");
}

#[test]
fn missing_member_cross_type_ignores_type_prefix_collisions() {
    let tmp = setup_member_error_source(
        "export class SpawnCoordinator {\n  begin_spawn(): void {}\n  cancel_spawn(): void {}\n}\n\nexport class SpawnCoordinatorFactory {}\n\nexport class FactoryState {\n  spawn: SpawnCoordinatorFactory;\n}\n",
    );
    let output = run_fmm(tmp.path(), &["read", "SpawnCoordinator.spawn"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!stderr.contains("Cross-type:"), "got: {stderr}");
    assert!(!stderr.contains("FactoryState"), "got: {stderr}");
}

#[test]
fn missing_member_large_method_list_is_capped() {
    let tmp = setup_large_class_project();
    let output = run_fmm(tmp.path(), &["read", "BigService.work"]);

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Did you mean: doWork000"), "got: {stderr}");
    assert!(stderr.contains("Methods:"), "got: {stderr}");
    assert!(stderr.contains("doWork000"), "got: {stderr}");
    assert!(stderr.contains("doWork019"), "got: {stderr}");
    assert!(!stderr.contains("doWork020"), "got: {stderr}");
    assert!(stderr.contains("... +130 more"), "got: {stderr}");
    assert!(stderr.contains("(150 members total;"), "got: {stderr}");
}

#[test]
fn file_qualified_missing_member_uses_requested_file_in_hint() {
    let tmp = setup_scopepath_collision_project();
    let output = run_fmm(
        tmp.path(),
        &["read", "crates/cm-core/src/types.rs:ScopePath.validte"],
    );

    assert!(
        !output.status.success(),
        "fmm read should fail for a missing member"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Did you mean: validate?"), "got: {stderr}");
    assert!(
        stderr.contains(
            "use fmm outline crates/cm-core/src/types.rs --include-private for full list"
        ),
        "got: {stderr}"
    );
    assert!(
        !stderr.contains("crates/cm-web/frontend/src/api/generated/ScopePath.ts"),
        "got: {stderr}"
    );
}
