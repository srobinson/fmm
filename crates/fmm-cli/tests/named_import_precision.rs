//! Integration tests for ALP-884: named import filtering (Layer 2) and
//! call-site verification (Layer 3) correctness in fmm_glossary.
//!
//! Fixture layout:
//!   src/WorkLoop.ts       — exports scheduleUpdate, requestLane, peekLane
//!   src/HooksModule.ts    — named-imports scheduleUpdate, calls it
//!   src/ProfilerTimer.ts  — named-imports requestLane (not scheduleUpdate), calls it
//!   src/ClassComponent.ts — named-imports both, calls both
//!   src/ReExporter.ts     — re-exports scheduleUpdate (no call site)
//!   src/DeadImport.ts     — named-imports scheduleUpdate but never calls it
//!   src/NamespaceUser.ts  — namespace import; calls WL.scheduleUpdate()
//!

use serde_json::json;

fn call_tool_text(
    server: &fmm::mcp::SqliteMcpServer,
    tool: &str,
    args: serde_json::Value,
) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// Build and return a (TempDir, McpServer) for the named-import-precision fixture.
fn setup_precision_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let root = root.as_path();
    std::fs::create_dir_all(root.join("src")).unwrap();

    // WorkLoop.ts — source of truth; exports scheduleUpdate, requestLane, peekLane
    std::fs::write(
        root.join("src/WorkLoop.ts"),
        "export function scheduleUpdate(root: any) {}\nexport function requestLane(root: any) { return 0; }\nexport function peekLane() { return 0; }\n",
    ).unwrap();

    // HooksModule.ts — named-imports scheduleUpdate, calls it
    std::fs::write(
        root.join("src/HooksModule.ts"),
        "import { scheduleUpdate } from './WorkLoop';\nexport function mountEffect(fiber: any) { scheduleUpdate(fiber); }\n",
    ).unwrap();

    // ProfilerTimer.ts — named-imports requestLane only (NOT scheduleUpdate)
    std::fs::write(
        root.join("src/ProfilerTimer.ts"),
        "import { requestLane } from './WorkLoop';\nexport function getTimerLane() { return requestLane(null); }\n",
    ).unwrap();

    // ClassComponent.ts — named-imports both scheduleUpdate and requestLane, calls both
    std::fs::write(
        root.join("src/ClassComponent.ts"),
        "import { scheduleUpdate, requestLane } from './WorkLoop';\nexport function updateComponent(comp: any) {\n  const lane = requestLane(comp);\n  scheduleUpdate(comp);\n}\n",
    ).unwrap();

    // ReExporter.ts — re-exports scheduleUpdate; no call site
    std::fs::write(
        root.join("src/ReExporter.ts"),
        "export { scheduleUpdate } from './WorkLoop';\n",
    )
    .unwrap();

    // DeadImport.ts — named-imports scheduleUpdate but never calls it
    std::fs::write(
        root.join("src/DeadImport.ts"),
        "import { scheduleUpdate } from './WorkLoop';\n// intentionally never called\nexport const MARKER = 42;\n",
    ).unwrap();

    // NamespaceUser.ts — namespace import; calls WL.scheduleUpdate()
    std::fs::write(
        root.join("src/NamespaceUser.ts"),
        "import * as WL from './WorkLoop';\nexport function debugSchedule(root: any) { WL.scheduleUpdate(root); }\n",
    ).unwrap();

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    (
        tmp,
        fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf()),
    )
}

// ── Layer 2 tests (default precision: "named") ─────────────────────────────

#[test]
fn layer2_includes_named_importers_of_scheduleupdate() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate"}),
    );
    assert!(
        text.contains("HooksModule.ts"),
        "HooksModule.ts must be included; got:\n{}",
        text
    );
    assert!(
        text.contains("ClassComponent.ts"),
        "ClassComponent.ts must be included; got:\n{}",
        text
    );
    assert!(
        text.contains("ReExporter.ts"),
        "ReExporter.ts must be included at Layer 2; got:\n{}",
        text
    );
    assert!(
        text.contains("DeadImport.ts"),
        "DeadImport.ts must be included at Layer 2; got:\n{}",
        text
    );
}

#[test]
fn layer2_excludes_profiler_timer_which_imports_different_symbol() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate"}),
    );
    assert!(
        !text.contains("ProfilerTimer.ts"),
        "ProfilerTimer.ts must be excluded; got:\n{}",
        text
    );
}

#[test]
fn layer2_annotates_namespace_importers() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate"}),
    );
    assert!(
        text.contains("NamespaceUser.ts"),
        "NamespaceUser.ts must appear; got:\n{}",
        text
    );
    assert!(
        text.contains("via namespace import"),
        "must include namespace annotation; got:\n{}",
        text
    );
}

#[test]
fn layer2_discloses_excluded_count() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate"}),
    );
    assert!(
        text.contains("additional"),
        "must disclose excluded count; got:\n{}",
        text
    );
}

// ── Layer 3 tests (precision: "call-site") ─────────────────────────────────

#[test]
fn layer3_excludes_dead_import() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate", "precision": "call-site"}),
    );
    assert!(
        !text.contains("DeadImport.ts"),
        "DeadImport.ts must be excluded by Layer 3; got:\n{}",
        text
    );
}

#[test]
fn layer3_keeps_confirmed_callers() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate", "precision": "call-site"}),
    );
    assert!(
        text.contains("HooksModule.ts"),
        "HooksModule.ts must remain after Layer 3; got:\n{}",
        text
    );
    assert!(
        text.contains("ClassComponent.ts"),
        "ClassComponent.ts must remain; got:\n{}",
        text
    );
}

#[test]
fn layer3_annotates_reexport_files() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate", "precision": "call-site"}),
    );
    assert!(
        text.contains("ReExporter.ts"),
        "ReExporter.ts must be annotated; got:\n{}",
        text
    );
    assert!(
        text.contains("re-exports only"),
        "must include re-exports annotation; got:\n{}",
        text
    );
}

#[test]
fn layer3_namespace_user_still_annotated() {
    let (_tmp, server) = setup_precision_server();
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdate", "precision": "call-site"}),
    );
    assert!(
        text.contains("NamespaceUser.ts"),
        "NamespaceUser.ts must appear at Layer 3; got:\n{}",
        text
    );
    assert!(
        text.contains("via namespace import"),
        "namespace annotation must be present; got:\n{}",
        text
    );
}

// ── React integration test (ignored, requires REACT_SRC env var) ───────────

#[test]
#[ignore = "requires React source indexed at REACT_SRC env var"]
fn schedule_update_precision_react() {
    let react_src =
        std::env::var("REACT_SRC").expect("REACT_SRC must point to an indexed React source tree");
    let server = fmm::mcp::SqliteMcpServer::with_root(std::path::PathBuf::from(&react_src));
    let text = call_tool_text(
        &server,
        "fmm_glossary",
        json!({"pattern": "scheduleUpdateOnFiber"}),
    );
    assert!(
        !text.contains("ReactProfilerTimer"),
        "ReactProfilerTimer must be excluded by Layer 2; got:\n{}",
        text
    );
}
