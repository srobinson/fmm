use serde_json::Value;
use std::path::Path;

pub(crate) fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

pub(crate) fn setup_mcp_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "src/auth/session.ts",
        "import jwt from 'jwt';\nimport redis from 'redis';\nimport { Types } from './types';\nimport { Config } from '../config';\n\nexport function createSession() {\n  return jwt.sign({});\n}\n\nexport function validateSession(token: string) {\n  return jwt.verify(token);\n}\n",
    );

    write_file(
        root,
        "src/auth/types.ts",
        "export interface SessionToken {\n  token: string;\n  expires: number;\n}\n\nexport type UserRole = 'admin' | 'user';\n",
    );

    write_file(
        root,
        "src/config.ts",
        "import dotenv from 'dotenv';\n\nexport function loadConfig() {\n  dotenv.config();\n  return {};\n}\n\nexport interface AppConfig {\n  port: number;\n}\n",
    );

    write_file(
        root,
        "src/db/pool.ts",
        "import pg from 'pg';\nimport { Config } from '../config';\n\nexport class Pool {\n  private client: pg.Client;\n}\n\nexport function createPool() {\n  return new Pool();\n}\n",
    );

    write_file(
        root,
        "src/utils/crypto.ts",
        "import bcrypt from 'bcrypt';\n\nexport function hashPassword(pw: string) {\n  return bcrypt.hash(pw, 10);\n}\n\nexport function verifyPassword(pw: string, hash: string) {\n  return bcrypt.compare(pw, hash);\n}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

pub(crate) fn setup_go_mcp_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "cmd/main.go",
        "package main\n\nimport (\n\t\"fmt\"\n\t\"github.com/user/project/internal/handler\"\n)\n\nfunc main() {\n\tfmt.Println(handler.NewHandler())\n}\n",
    );

    write_file(
        root,
        "internal/handler/handler.go",
        "package handler\n\nimport \"net/http\"\n\ntype Handler struct{}\n\nfunc NewHandler() *Handler {\n\treturn &Handler{}\n}\n\nfunc (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

pub(crate) fn setup_large_class_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    let mut source = String::from("export class BigService {\n");
    for i in 0..150usize {
        source.push_str(&format!(
            "  doWork{i:03}(input: string): string {{\n    // perform operation {i:03}\n    return input;\n  }}\n"
        ));
    }
    source.push_str("}\n");

    assert!(
        source.len() > 10_240,
        "test source must exceed 10KB, got {} bytes",
        source.len()
    );

    write_file(root, "src/service.ts", &source);

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

pub(crate) fn setup_collision_server() -> (tempfile::TempDir, fmm::mcp::SqliteMcpServer) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    write_file(
        root,
        "packages/renderer/dispatch.ts",
        "export interface DispatchConfig { timeout: number; }\n",
    );

    write_file(
        root,
        "packages/native/dispatch.ts",
        "export interface DispatchConfig { retries: number; }\n",
    );

    write_file(
        root,
        "packages/renderer/session.ts",
        "export function createSession() {}\n",
    );

    fmm::cli::generate(&[root.to_str().unwrap().to_string()], false, false, true).unwrap();
    let server = fmm::mcp::SqliteMcpServer::with_root(root.to_path_buf());
    (tmp, server)
}

pub(crate) fn call_tool_text(
    server: &fmm::mcp::SqliteMcpServer,
    tool: &str,
    args: Value,
) -> String {
    let result = server.call_tool(tool, args).unwrap();
    result["content"][0]["text"].as_str().unwrap().to_string()
}

pub(crate) fn call_tool_expect_error(
    server: &fmm::mcp::SqliteMcpServer,
    tool: &str,
    args: Value,
) -> String {
    let result = server.call_tool(tool, args).unwrap();
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(
        text.starts_with("ERROR:"),
        "Expected ERROR: prefix but got: {}",
        text
    );
    text
}
