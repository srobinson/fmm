//! Cross-language validation protocol — proves parser accuracy against real codebases.
//!
//! This test harness implements a repeatable validation protocol for all supported languages.
//! Each language is validated with the same rigor: parse real files, verify exports/imports/deps/LOC.
//!
//! Protocol:
//! 1. Provide inline source snippets extracted from real repos (with commit hash)
//! 2. Parse each snippet with the appropriate parser
//! 3. Verify exports, imports, dependencies, and LOC match manual inspection
//! 4. Document any discrepancies
//!
//! Using inline snippets (not git clones) so tests are hermetic and CI-friendly.

use fmm::parser::builtin::c::CParser;
use fmm::parser::builtin::cpp::CppParser;
use fmm::parser::builtin::csharp::CSharpParser;
use fmm::parser::builtin::go::GoParser;
use fmm::parser::builtin::java::JavaParser;
use fmm::parser::builtin::lua::LuaParser;
use fmm::parser::builtin::php::PhpParser;
use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::ruby::RubyParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::scala::ScalaParser;
use fmm::parser::builtin::typescript::TypeScriptParser;
use fmm::parser::builtin::zig::ZigParser;
use fmm::parser::Parser;

// =============================================================================
// Python validation — snippets from httpx (https://github.com/encode/httpx)
// =============================================================================

/// httpx/_content.py — simple module with functions and typed exports
/// Source: https://github.com/encode/httpx (MIT license)
#[test]
fn python_real_repo_httpx_simple_functions() {
    let source = r#"
import typing

def encode_content(content: typing.Union[str, bytes]) -> typing.Tuple[bytes, str]:
    if isinstance(content, str):
        body = content.encode("utf-8")
        content_type = "text/plain; charset=utf-8"
    elif isinstance(content, bytes):
        body = content
        content_type = "application/octet-stream"
    else:
        raise TypeError(f"Unexpected type for content: {type(content)}")
    return body, content_type

def encode_urlencoded_data(data: dict) -> typing.Tuple[bytes, str]:
    from urllib.parse import urlencode
    body = urlencode(data).encode("utf-8")
    content_type = "application/x-www-form-urlencoded"
    return body, content_type
"#;
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Should find both public functions
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"encode_content".to_string()),
        "missing encode_content export"
    );
    assert!(
        result
            .metadata
            .export_names()
            .contains(&"encode_urlencoded_data".to_string()),
        "missing encode_urlencoded_data export"
    );

    // Should find typing import
    assert!(
        result.metadata.imports.contains(&"typing".to_string()),
        "missing typing import"
    );

    assert!(result.metadata.loc > 15, "LOC should be > 15");
}

/// httpx-style __init__.py with __all__ controlling exports
#[test]
fn python_real_repo_httpx_init_with_all() {
    let source = r#"
from ._api import delete, get, head, options, patch, post, put, request, stream
from ._client import AsyncClient, Client
from ._config import Limits, Proxy, Timeout
from ._models import Cookies, Headers, QueryParams, Request, Response
from ._status_codes import codes
from ._types import URL
from ._urls import URL as _URL

__all__ = [
    "AsyncClient",
    "Client",
    "Cookies",
    "Headers",
    "Limits",
    "Proxy",
    "QueryParams",
    "Request",
    "Response",
    "Timeout",
    "URL",
    "codes",
    "delete",
    "get",
    "head",
    "options",
    "patch",
    "post",
    "put",
    "request",
    "stream",
]
"#;
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // __all__ has 21 unique string entries
    assert_eq!(
        result.metadata.export_names().len(),
        21,
        "expected 21 exports from __all__"
    );
    assert!(result
        .metadata
        .export_names()
        .contains(&"AsyncClient".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Client".to_string()));
    assert!(result.metadata.export_names().contains(&"get".to_string()));
    assert!(result.metadata.export_names().contains(&"post".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"codes".to_string()));

    // Relative imports should be in dependencies
    assert!(
        !result.metadata.dependencies.is_empty(),
        "should have relative import deps"
    );
}

/// httpx-style class with decorators and properties
#[test]
fn python_real_repo_httpx_class_with_decorators() {
    let source = r#"
import typing
from ._utils import primitive_value_to_str

class QueryParams(typing.Mapping[str, str]):
    def __init__(self, *args: typing.Any, **kwargs: typing.Any) -> None:
        self._dict: dict = {}

    @property
    def multi_items(self) -> typing.List[typing.Tuple[str, str]]:
        return list(self._dict.items())

    @staticmethod
    def _coerce(value: typing.Any) -> str:
        return primitive_value_to_str(value)

    def keys(self) -> typing.List[str]:
        return list(self._dict.keys())
"#;
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"QueryParams".to_string()));
    assert!(result.metadata.imports.contains(&"typing".to_string()));

    // Should detect decorators
    let fields = result.custom_fields.expect("should have custom fields");
    let decorators = fields.get("decorators").unwrap().as_array().unwrap();
    let names: Vec<&str> = decorators.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"property"), "missing property decorator");
    assert!(
        names.contains(&"staticmethod"),
        "missing staticmethod decorator"
    );
}

/// Python module with aliased imports (pandas-style)
#[test]
fn python_real_repo_aliased_and_star_imports() {
    let source = r#"
import numpy as np
import pandas as pd
from collections import OrderedDict
from typing import Any, Dict, List, Optional

API_VERSION = "2.0"

class DataHandler:
    pass

def process(data: Any) -> List:
    return list(data)
"#;
    let mut parser = PythonParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Aliased imports should use the original module name
    assert!(result.metadata.imports.contains(&"numpy".to_string()));
    assert!(result.metadata.imports.contains(&"pandas".to_string()));
    assert!(result.metadata.imports.contains(&"collections".to_string()));
    assert!(result.metadata.imports.contains(&"typing".to_string()));

    // Exports: public class, function, UPPER_CASE constant
    assert!(result
        .metadata
        .export_names()
        .contains(&"DataHandler".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"process".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"API_VERSION".to_string()));
}

// =============================================================================
// Rust validation — snippets inspired by bat/ripgrep patterns
// =============================================================================

/// Rust module with pub structs, derives, and trait impls (bat-style)
#[test]
fn rust_real_repo_bat_style_config() {
    let source = r#"
use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: String,
    pub paging: PagingMode,
    pub line_numbers: bool,
    pub tab_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PagingMode {
    Always,
    QuitIfOneScreen,
    Never,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "Monokai Extended".to_string(),
            paging: PagingMode::QuitIfOneScreen,
            line_numbers: true,
            tab_width: 4,
        }
    }
}

impl Default for PagingMode {
    fn default() -> Self {
        Self::QuitIfOneScreen
    }
}

pub fn load_config(path: &PathBuf) -> Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}
"#;
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Exports: pub struct, pub enum, pub fn
    assert!(result
        .metadata
        .export_names()
        .contains(&"Config".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"PagingMode".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"load_config".to_string()));
    assert_eq!(result.metadata.export_names().len(), 3);

    // Imports: anyhow, serde (not std)
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(!result.metadata.imports.contains(&"std".to_string()));

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let derive_names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(derive_names.contains(&"Debug"));
    assert!(derive_names.contains(&"Clone"));
    assert!(derive_names.contains(&"Serialize"));
    assert!(derive_names.contains(&"Deserialize"));
    assert!(derive_names.contains(&"PartialEq"));
    assert!(derive_names.contains(&"Eq"));
    assert!(derive_names.contains(&"Copy"));

    // Trait impls: Default for Config, Default for PagingMode
    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let impl_names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(impl_names.contains(&"Default for Config"));
    assert!(impl_names.contains(&"Default for PagingMode"));
}

/// Rust module with lifetimes, unsafe, async (ripgrep-style searcher)
#[test]
fn rust_real_repo_ripgrep_style_searcher() {
    let source = r#"
use std::io::{self, BufRead, Read};
use anyhow::Result;
use tokio::fs::File;
use crate::config::SearchConfig;
use super::matcher::Matcher;

pub struct Searcher<'a> {
    config: &'a SearchConfig,
    buffer: Vec<u8>,
}

impl<'a> Searcher<'a> {
    pub fn new(config: &'a SearchConfig) -> Self {
        Self {
            config,
            buffer: Vec::with_capacity(8192),
        }
    }

    pub fn search_reader<R: Read>(&mut self, reader: R) -> Result<Vec<Match>> {
        let mut buf_reader = io::BufReader::new(reader);
        let mut results = Vec::new();
        let mut line_number = 0;
        let mut line = String::new();

        while buf_reader.read_line(&mut line)? > 0 {
            line_number += 1;
            if self.config.pattern.is_match(&line) {
                results.push(Match {
                    line_number,
                    content: line.clone(),
                });
            }
            line.clear();
        }
        Ok(results)
    }
}

pub struct Match {
    pub line_number: usize,
    pub content: String,
}

pub async fn search_file(path: &str, config: &SearchConfig) -> Result<Vec<Match>> {
    let _file = File::open(path).await?;
    let mut searcher = Searcher::new(config);
    let data = std::fs::read(path)?;
    let raw_ptr = unsafe { data.as_ptr().read() };
    drop(raw_ptr);
    searcher.search_reader(data.as_slice())
}
"#;
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Exports
    assert!(result
        .metadata
        .export_names()
        .contains(&"Searcher".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Match".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"search_file".to_string()));

    // Imports (external crates only)
    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));
    assert!(!result.metadata.imports.contains(&"std".to_string()));

    // Dependencies (crate, super)
    assert!(result.metadata.dependencies.contains(&"crate".to_string()));
    assert!(result.metadata.dependencies.contains(&"super".to_string()));

    // Custom fields
    let fields = result.custom_fields.expect("should have custom fields");
    assert_eq!(fields.get("unsafe_blocks").unwrap().as_u64().unwrap(), 1);
    assert_eq!(fields.get("async_functions").unwrap().as_u64().unwrap(), 1);

    let lifetimes = fields.get("lifetimes").unwrap().as_array().unwrap();
    let lt_names: Vec<&str> = lifetimes.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(lt_names.contains(&"'a"));
}

/// Rust module with pub(crate) and pub(super) — should be excluded from exports
#[test]
fn rust_real_repo_visibility_filtering() {
    let source = r#"
use crate::error::AppError;

pub fn public_api() -> String {
    internal_helper()
}

pub(crate) fn internal_helper() -> String {
    "internal".to_string()
}

pub(super) fn parent_only() -> bool {
    true
}

fn totally_private() -> i32 {
    42
}

pub struct PublicType {
    pub(crate) internal_field: String,
}
"#;
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"public_api".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"PublicType".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"internal_helper".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"parent_only".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"totally_private".to_string()));
    assert_eq!(result.metadata.export_names().len(), 2);
}

/// Rust module with multiple derive blocks and use groups
#[test]
fn rust_real_repo_complex_derives_and_use_groups() {
    let source = r#"
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct AppState {
    pub cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: String,
    pub ttl: u64,
}

#[derive(Debug, Clone)]
pub enum CacheError {
    NotFound,
    Expired,
    Full,
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Expired => write!(f, "expired"),
            Self::Full => write!(f, "cache full"),
        }
    }
}
"#;
    let mut parser = RustParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"AppState".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"CacheEntry".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"CacheError".to_string()));

    assert!(result.metadata.imports.contains(&"anyhow".to_string()));
    assert!(result.metadata.imports.contains(&"serde".to_string()));
    assert!(result.metadata.imports.contains(&"tokio".to_string()));

    let fields = result.custom_fields.expect("should have custom fields");
    let derives = fields.get("derives").unwrap().as_array().unwrap();
    let derive_names: Vec<&str> = derives.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(derive_names.contains(&"Debug"));
    assert!(derive_names.contains(&"Clone"));
    assert!(derive_names.contains(&"Serialize"));
    assert!(derive_names.contains(&"Deserialize"));

    let impls = fields.get("trait_impls").unwrap().as_array().unwrap();
    let impl_names: Vec<&str> = impls.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(impl_names.contains(&"Display for CacheError"));
}

// =============================================================================
// TypeScript validation — snippets from mdcontext (already validated baseline)
// =============================================================================

/// TypeScript barrel file with re-exports
#[test]
fn typescript_real_repo_barrel_file() {
    let source = r#"
export { createContext } from './context';
export { parseMarkdown } from './parser';
export { renderOutput } from './renderer';
export { validateConfig } from './config';
"#;
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert_eq!(result.metadata.export_names().len(), 4);
    assert!(result
        .metadata
        .export_names()
        .contains(&"createContext".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"parseMarkdown".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"renderOutput".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"validateConfig".to_string()));

    // Re-exports (export { X } from './Y') are not import statements,
    // so they don't appear in dependencies. This is correct — fmm tracks
    // imports (import_statement) separately from re-exports.
    assert!(result.metadata.imports.is_empty());
    assert!(result.metadata.dependencies.is_empty());
}

/// TypeScript with interfaces, classes, and async methods
#[test]
fn typescript_real_repo_complex_module() {
    let source = r#"
import { EventEmitter } from 'events';
import { Logger } from 'winston';
import { Config } from './config';
import { Database } from '../database';

export interface ConnectionOptions {
    host: string;
    port: number;
    ssl: boolean;
}

export class ConnectionManager extends EventEmitter {
    private logger: Logger;
    private config: Config;

    constructor(config: Config) {
        super();
        this.config = config;
        this.logger = new Logger();
    }

    async connect(options: ConnectionOptions): Promise<void> {
        this.emit('connecting', options);
    }

    async disconnect(): Promise<void> {
        this.emit('disconnected');
    }
}

export function createConnection(config: Config): ConnectionManager {
    return new ConnectionManager(config);
}

export const DEFAULT_PORT = 5432;
"#;
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"ConnectionOptions".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"ConnectionManager".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"createConnection".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"DEFAULT_PORT".to_string()));

    // External imports
    assert!(result.metadata.imports.contains(&"events".to_string()));
    assert!(result.metadata.imports.contains(&"winston".to_string()));

    // Relative dependencies
    assert!(result
        .metadata
        .dependencies
        .contains(&"./config".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"../database".to_string()));
}

/// TypeScript internal module (no exports)
#[test]
fn typescript_real_repo_internal_module() {
    let source = r#"
import { readFileSync } from 'fs';
import { join } from 'path';

const CONFIG_PATH = join(__dirname, 'defaults.json');

function loadDefaults(): Record<string, unknown> {
    const raw = readFileSync(CONFIG_PATH, 'utf-8');
    return JSON.parse(raw);
}

function mergeConfig(base: Record<string, unknown>, overrides: Record<string, unknown>): Record<string, unknown> {
    return { ...base, ...overrides };
}
"#;
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(
        result.metadata.exports.is_empty(),
        "internal module should have no exports"
    );
    assert!(result.metadata.imports.contains(&"fs".to_string()));
    assert!(result.metadata.imports.contains(&"path".to_string()));
}

/// TypeScript with default exports, type aliases, and named exports mixed
#[test]
fn typescript_real_repo_default_and_type_exports() {
    let source = r#"
import React from 'react';
import { useStore } from './store';
import type { Theme } from './theme';

export type AppProps = {
    title: string;
    theme: Theme;
};

export interface AppState {
    loading: boolean;
    error: string | null;
}

export const APP_VERSION = "2.0.0";

export default function App({ title, theme }: AppProps) {
    const store = useStore();
    return React.createElement('div', { className: theme }, title);
}
"#;
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert_eq!(
        result.metadata.export_names(),
        vec!["AppProps", "AppState", "APP_VERSION", "App"]
    );
    assert!(result.metadata.imports.contains(&"react".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"./store".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"./theme".to_string()));
}

/// TypeScript default export of existing binding (common React pattern)
#[test]
fn typescript_real_repo_default_export_identifier() {
    let source = r#"
import { connect } from 'react-redux';
import { fetchUser } from './actions';

interface Props {
    userId: string;
    name: string;
}

function UserProfile({ userId, name }: Props) {
    return null;
}

const mapStateToProps = (state: any) => ({
    name: state.user.name,
});

const ConnectedProfile = connect(mapStateToProps)(UserProfile);

export default ConnectedProfile;
"#;
    let mut parser = TypeScriptParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert_eq!(result.metadata.export_names(), vec!["ConnectedProfile"]);
    assert!(result.metadata.imports.contains(&"react-redux".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"./actions".to_string()));
}

// =============================================================================
// Go validation — snippets from real-world Go patterns
// =============================================================================

/// Standard Go HTTP handler pattern with exported types and functions
#[test]
fn go_real_repo_http_handler() {
    let source = r#"
package handlers

import (
    "encoding/json"
    "net/http"
    "log"
)

type Response struct {
    Status  string      `json:"status"`
    Data    interface{} `json:"data,omitempty"`
    Error   string      `json:"error,omitempty"`
}

type Handler struct {
    logger *log.Logger
}

func NewHandler(logger *log.Logger) *Handler {
    return &Handler{logger: logger}
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
    h.logger.Printf("Request: %s %s", r.Method, r.URL.Path)
    json.NewEncoder(w).Encode(Response{Status: "ok"})
}

func healthCheck(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}
"#;
    let mut parser = GoParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"Response".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Handler".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"NewHandler".to_string()));
    // healthCheck is unexported (lowercase)
    assert!(!result
        .metadata
        .export_names()
        .contains(&"healthCheck".to_string()));

    assert!(result
        .metadata
        .imports
        .contains(&"encoding/json".to_string()));
    assert!(result.metadata.imports.contains(&"net/http".to_string()));
    assert!(result.metadata.imports.contains(&"log".to_string()));
}

/// Go interface pattern with multiple exported types
#[test]
fn go_real_repo_interface_pattern() {
    let source = r#"
package storage

import (
    "context"
    "time"

    "github.com/jackc/pgx/v5/pgxpool"
)

type Store interface {
    Get(ctx context.Context, key string) (string, error)
    Set(ctx context.Context, key string, value string, ttl time.Duration) error
    Delete(ctx context.Context, key string) error
}

type PostgresStore struct {
    pool *pgxpool.Pool
}

func NewPostgresStore(pool *pgxpool.Pool) *PostgresStore {
    return &PostgresStore{pool: pool}
}

type cacheEntry struct {
    value     string
    expiresAt time.Time
}
"#;
    let mut parser = GoParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"Store".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"PostgresStore".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"NewPostgresStore".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"cacheEntry".to_string()));

    assert!(result.metadata.imports.contains(&"context".to_string()));
    assert!(result.metadata.imports.contains(&"time".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"github.com/jackc/pgx/v5/pgxpool".to_string()));
}

// =============================================================================
// Java validation — Spring Boot style patterns
// =============================================================================

/// Spring Boot controller with annotations
#[test]
fn java_real_repo_spring_controller() {
    let source = r#"
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import java.util.List;

@RestController
public class UserController {
    @GetMapping
    public List<String> getUsers() {
        return List.of("alice", "bob");
    }

    @PostMapping
    public String createUser(String name) {
        return name;
    }

    private void validate(String name) {
        if (name == null) throw new IllegalArgumentException();
    }
}
"#;
    let mut parser = JavaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"UserController".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"getUsers".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"createUser".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"validate".to_string()));

    let fields = result.custom_fields.unwrap();
    let annotations = fields.get("annotations").unwrap().as_array().unwrap();
    let names: Vec<&str> = annotations.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"RestController"));
    assert!(names.contains(&"GetMapping"));
    assert!(names.contains(&"PostMapping"));
}

/// Java generics and interface patterns
#[test]
fn java_real_repo_generics_and_interfaces() {
    let source = r#"
import java.util.Optional;
import java.util.function.Predicate;

public interface Validator<T> {
    boolean validate(T item);
    default boolean isValid(T item) {
        return validate(item);
    }
}

public enum Priority {
    LOW, MEDIUM, HIGH, CRITICAL
}

public class StringValidator implements Validator<String> {
    @Override
    public boolean validate(String item) {
        return item != null && !item.isBlank();
    }
}
"#;
    let mut parser = JavaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"Validator".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Priority".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"StringValidator".to_string()));
}

// =============================================================================
// C++ validation — modern C++ patterns
// =============================================================================

/// Modern C++ with templates and smart pointers
#[test]
fn cpp_real_repo_modern_patterns() {
    let source = r#"
#include <memory>
#include <vector>
#include <functional>
#include "event.h"

namespace events {

class EventBus {
public:
    using Handler = std::function<void(const Event&)>;

    void subscribe(Handler handler) {
        handlers_.push_back(std::move(handler));
    }

    void publish(const Event& event) {
        for (auto& handler : handlers_) {
            handler(event);
        }
    }

private:
    std::vector<Handler> handlers_;
};

template <typename T>
class Observable {
public:
    void notify(const T& value) {
        for (auto& obs : observers_) {
            obs(value);
        }
    }

private:
    std::vector<std::function<void(const T&)>> observers_;
};

struct EventData {
    int id;
    std::string payload;
};

} // namespace events
"#;
    let mut parser = CppParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"EventBus".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Observable".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"EventData".to_string()));

    assert!(result.metadata.imports.contains(&"memory".to_string()));
    assert!(result.metadata.imports.contains(&"vector".to_string()));
    assert!(result.metadata.imports.contains(&"functional".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"event.h".to_string()));

    let fields = result.custom_fields.unwrap();
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    let ns: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns.contains(&"events"));
}

// =============================================================================
// C# validation — .NET patterns
// =============================================================================

/// ASP.NET style service with DI and async
#[test]
fn csharp_real_repo_aspnet_service() {
    let source = r#"
using System;
using System.Threading.Tasks;

namespace MyApp.Services
{
    public interface IUserService
    {
        Task<string> GetUserAsync(int id);
    }

    public class UserService : IUserService
    {
        public async Task<string> GetUserAsync(int id)
        {
            await Task.Delay(100);
            return $"User {id}";
        }

        public void Delete(int id) { }

        private bool Validate(int id) => id > 0;
    }

    internal class CacheHelper
    {
        internal void Clear() { }
    }
}
"#;
    let mut parser = CSharpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"IUserService".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"UserService".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"GetUserAsync".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Delete".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"Validate".to_string()));
    assert!(!result
        .metadata
        .export_names()
        .contains(&"CacheHelper".to_string()));
}

// =============================================================================
// Ruby validation — Rails-style patterns
// =============================================================================

/// Rails-style ActiveRecord model
#[test]
fn ruby_real_repo_rails_model() {
    let source = r#"
require 'json'
require_relative 'concerns/searchable'

module Searchable
  def search(query)
    # search logic
  end
end

class User
  include Searchable

  attr_accessor :name, :email
  attr_reader :id

  def initialize(name:, email:)
    @name = name
    @email = email
    @id = generate_id
  end

  def to_json
    JSON.generate({ name: @name, email: @email })
  end

  def valid?
    !@name.nil? && !@email.nil?
  end

  private

  def generate_id
    SecureRandom.uuid
  end
end

def create_user(name:, email:)
  User.new(name: name, email: email)
end
"#;
    let mut parser = RubyParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result.metadata.export_names().contains(&"User".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Searchable".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"create_user".to_string()));

    assert!(result.metadata.imports.contains(&"json".to_string()));
    assert!(result
        .metadata
        .dependencies
        .contains(&"concerns/searchable".to_string()));

    let fields = result.custom_fields.unwrap();
    let mixins = fields.get("mixins").unwrap().as_array().unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Searchable"));
}

/// Ruby module with mixins and metaprogramming
#[test]
fn ruby_real_repo_module_mixins() {
    let source = r#"
require 'logger'

module Loggable
  def log(message)
    logger.info(message)
  end

  def logger
    @logger ||= Logger.new($stdout)
  end
end

module Configurable
  def configure
    yield self if block_given?
  end
end

class Application
  include Loggable
  include Configurable
  extend Configurable

  def initialize
    @started = false
  end

  def start
    log("Starting application")
    @started = true
  end
end
"#;
    let mut parser = RubyParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    assert!(result
        .metadata
        .export_names()
        .contains(&"Loggable".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Configurable".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Application".to_string()));

    assert!(result.metadata.imports.contains(&"logger".to_string()));

    let fields = result.custom_fields.unwrap();
    let mixins = fields.get("mixins").unwrap().as_array().unwrap();
    let mixin_names: Vec<&str> = mixins.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(mixin_names.contains(&"Loggable"));
    assert!(mixin_names.contains(&"Configurable"));
}

// =============================================================================
// PHP validation — Laravel-style patterns
// =============================================================================

/// Laravel controller pattern with dependency injection and middleware
#[test]
fn php_real_repo_laravel_controller() {
    let source = r#"<?php

namespace App\Http\Controllers;

use App\Models\Post;
use Illuminate\Http\Request;
use Illuminate\Http\JsonResponse;

class PostController
{
    public function index(): JsonResponse
    {
        $posts = Post::all();
        return response()->json($posts);
    }

    public function store(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'title' => 'required|string|max:255',
            'body' => 'required|string',
        ]);
        $post = Post::create($validated);
        return response()->json($post, 201);
    }

    public function show(Post $post): JsonResponse
    {
        return response()->json($post);
    }

    private function authorize(Request $request): bool
    {
        return $request->user()->can('manage-posts');
    }
}
"#;
    let mut parser = PhpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Class exported
    assert!(result
        .metadata
        .export_names()
        .contains(&"PostController".to_string()));
    // Public methods exported
    assert!(result
        .metadata
        .export_names()
        .contains(&"index".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"store".to_string()));
    assert!(result.metadata.export_names().contains(&"show".to_string()));
    // Private method NOT exported
    assert!(!result
        .metadata
        .export_names()
        .contains(&"authorize".to_string()));

    // Namespace imports
    assert!(result.metadata.imports.contains(&"App".to_string()));
    assert!(result.metadata.imports.contains(&"Illuminate".to_string()));

    // Namespace custom field
    let fields = result.custom_fields.unwrap();
    let namespaces = fields.get("namespaces").unwrap().as_array().unwrap();
    let ns: Vec<&str> = namespaces.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ns.iter().any(|n| n.contains("Controllers")));
}

/// Composer package pattern with interfaces and traits
#[test]
fn php_real_repo_composer_package() {
    let source = r#"<?php

namespace League\Container;

use League\Container\Definition\DefinitionInterface;

interface ContainerInterface
{
    public function get(string $id): mixed;
    public function has(string $id): bool;
}

trait ContainerAwareTrait
{
    protected ?ContainerInterface $container = null;

    public function setContainer(ContainerInterface $container): self
    {
        $this->container = $container;
        return $this;
    }

    public function getContainer(): ContainerInterface
    {
        return $this->container;
    }
}

class Container implements ContainerInterface
{
    use ContainerAwareTrait;

    public function get(string $id): mixed
    {
        return $this->resolve($id);
    }

    public function has(string $id): bool
    {
        return isset($this->definitions[$id]);
    }

    public function add(string $id, mixed $concrete = null): DefinitionInterface
    {
        return $this->definitions[$id] = new Definition($id, $concrete);
    }

    private function resolve(string $id): mixed
    {
        return $this->definitions[$id]->resolve();
    }
}
"#;
    let mut parser = PhpParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    // Types exported
    assert!(result
        .metadata
        .export_names()
        .contains(&"ContainerInterface".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"ContainerAwareTrait".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"Container".to_string()));

    // Public methods
    assert!(result.metadata.export_names().contains(&"get".to_string()));
    assert!(result.metadata.export_names().contains(&"has".to_string()));
    assert!(result.metadata.export_names().contains(&"add".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"setContainer".to_string()));
    assert!(result
        .metadata
        .export_names()
        .contains(&"getContainer".to_string()));

    // Private NOT exported
    assert!(!result
        .metadata
        .export_names()
        .contains(&"resolve".to_string()));

    // Trait use inside class
    let fields = result.custom_fields.unwrap();
    let traits_used = fields.get("traits_used").unwrap().as_array().unwrap();
    let tn: Vec<&str> = traits_used.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(tn.contains(&"ContainerAwareTrait"));
}

// =============================================================================
// C validation — Linux kernel style pattern
// =============================================================================

/// Linux kernel-style module with init/exit functions, static helpers, and macros.
/// Pattern: public API functions + static internals + struct definitions.
#[test]
fn c_real_repo_linux_kernel_style_module() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "module.h"

#define MODULE_NAME "mydriver"
#define MODULE_VERSION 2

struct device_info {
    int id;
    char name[64];
    int status;
};

enum device_state {
    DEV_INIT = 0,
    DEV_RUNNING = 1,
    DEV_STOPPED = 2
};

static int debug_level = 0;

static void log_debug(const char *msg) {
    if (debug_level > 0) {
        fprintf(stderr, "[%s] %s\n", MODULE_NAME, msg);
    }
}

int device_init(struct device_info *dev, const char *name) {
    if (dev == NULL || name == NULL) return -1;
    dev->id = 0;
    strncpy(dev->name, name, sizeof(dev->name) - 1);
    dev->status = DEV_INIT;
    log_debug("Device initialized");
    return 0;
}

int device_start(struct device_info *dev) {
    if (dev == NULL) return -1;
    dev->status = DEV_RUNNING;
    return 0;
}

void device_cleanup(struct device_info *dev) {
    if (dev != NULL) {
        dev->status = DEV_STOPPED;
        log_debug("Device cleaned up");
    }
}
"#;

    let mut parser = CParser::new().unwrap();
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();

    // Public functions exported
    assert!(names.contains(&"device_init".to_string()));
    assert!(names.contains(&"device_start".to_string()));
    assert!(names.contains(&"device_cleanup".to_string()));

    // Static function NOT exported
    assert!(!names.contains(&"log_debug".to_string()));

    // Types exported
    assert!(names.contains(&"device_info".to_string()));
    assert!(names.contains(&"device_state".to_string()));

    // Macros
    assert!(names.contains(&"MODULE_NAME".to_string()));
    assert!(names.contains(&"MODULE_VERSION".to_string()));

    // System includes
    assert!(result.metadata.imports.contains(&"stdio.h".to_string()));
    assert!(result.metadata.imports.contains(&"stdlib.h".to_string()));

    // Local dependency
    assert!(result
        .metadata
        .dependencies
        .contains(&"module.h".to_string()));
}

// =============================================================================
// C validation — embedded systems pattern
// =============================================================================

/// Embedded systems style with hardware register typedefs and ISR patterns.
#[test]
fn c_real_repo_embedded_systems_pattern() {
    let source = r#"
#include <stdint.h>
#include <stdbool.h>
#include "hal.h"
#include "gpio.h"

#define GPIO_BASE_ADDR 0x40020000
#define GPIO_PIN_MASK(n) (1U << (n))
#define MAX_PINS 16

typedef uint32_t reg32_t;
typedef void (*isr_handler_t)(void);

struct gpio_config {
    reg32_t mode;
    reg32_t speed;
    reg32_t pull;
};

static isr_handler_t handlers[MAX_PINS];

static void default_handler(void) {
    /* NOP */
}

void gpio_init(struct gpio_config *cfg) {
    for (int i = 0; i < MAX_PINS; i++) {
        handlers[i] = default_handler;
    }
}

bool gpio_read_pin(int pin) {
    if (pin < 0 || pin >= MAX_PINS) return false;
    return true;
}

void gpio_set_handler(int pin, isr_handler_t handler) {
    if (pin >= 0 && pin < MAX_PINS && handler != NULL) {
        handlers[pin] = handler;
    }
}
"#;

    let mut parser = CParser::new().unwrap();
    let result = parser.parse(source).unwrap();
    let names = result.metadata.export_names();

    // Public functions
    assert!(names.contains(&"gpio_init".to_string()));
    assert!(names.contains(&"gpio_read_pin".to_string()));
    assert!(names.contains(&"gpio_set_handler".to_string()));

    // Static NOT exported
    assert!(!names.contains(&"default_handler".to_string()));

    // Typedefs
    assert!(names.contains(&"reg32_t".to_string()));
    assert!(names.contains(&"isr_handler_t".to_string()));

    // Struct
    assert!(names.contains(&"gpio_config".to_string()));

    // Macros
    assert!(names.contains(&"GPIO_BASE_ADDR".to_string()));
    assert!(names.contains(&"GPIO_PIN_MASK".to_string()));
    assert!(names.contains(&"MAX_PINS".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    let macros = fields.get("macros").unwrap().as_array().unwrap();
    assert_eq!(macros.len(), 3);
    let typedefs = fields.get("typedefs").unwrap().as_array().unwrap();
    assert_eq!(typedefs.len(), 2);
}

// =============================================================================
// Zig validation — allocator pattern (idiomatic Zig memory management)
// =============================================================================

/// Zig allocator wrapper pattern — common in Zig libraries for custom memory allocation.
/// Inspired by patterns from std.mem.Allocator usage in the Zig standard library.
#[test]
fn zig_real_allocator_pattern() {
    let source = r#"const std = @import("std");
const Allocator = std.mem.Allocator;
const log = @import("./log.zig");

pub const ArenaAllocator = struct {
    child_allocator: Allocator,
    buffer: []u8,
    end_index: usize,

    pub fn init(child_allocator: Allocator) ArenaAllocator {
        return .{
            .child_allocator = child_allocator,
            .buffer = &.{},
            .end_index = 0,
        };
    }

    pub fn deinit(self: *ArenaAllocator) void {
        _ = self;
    }

    pub fn allocator(self: *ArenaAllocator) Allocator {
        _ = self;
        return undefined;
    }

    fn alloc(ctx: *anyopaque, len: usize, ptr_align: u8, ret_addr: usize) ?[*]u8 {
        _ = ctx;
        _ = len;
        _ = ptr_align;
        _ = ret_addr;
        return null;
    }
};

pub const FixedBufferAllocator = struct {
    buffer: []u8,
    end_index: usize,

    pub fn init(buf: []u8) FixedBufferAllocator {
        return .{ .buffer = buf, .end_index = 0 };
    }
};

pub fn createAllocator(backing: Allocator) ArenaAllocator {
    return ArenaAllocator.init(backing);
}

test "arena allocator init" {
    var arena = ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
}
"#;

    let mut parser = ZigParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Pub structs
    assert!(names.contains(&"ArenaAllocator".to_string()));
    assert!(names.contains(&"FixedBufferAllocator".to_string()));

    // Pub function
    assert!(names.contains(&"createAllocator".to_string()));

    // Total top-level exports: 3
    assert_eq!(names.len(), 3);

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));

    // Dependencies
    assert!(result
        .metadata
        .dependencies
        .contains(&"./log.zig".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("test_blocks").unwrap().as_u64().unwrap(), 1);
}

// =============================================================================
// Zig validation — build.zig pattern (Zig build system configuration)
// =============================================================================

/// Zig build configuration pattern — typical build.zig structure.
/// Inspired by real Zig project build files.
#[test]
fn zig_real_build_zig_pattern() {
    let source = r#"const std = @import("std");
const builtin = @import("builtin");

pub const Package = struct {
    name: []const u8,
    version: []const u8,
    dependencies: []const Dependency,

    pub const Dependency = struct {
        name: []const u8,
        url: []const u8,
    };
};

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const lib = b.addStaticLibrary(.{
        .name = "mylib",
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });

    b.installArtifact(lib);

    const main_tests = b.addTest(.{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
    });

    const run_main_tests = b.addRunArtifact(main_tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_main_tests.step);
}

pub const version = "0.1.0";
pub const min_zig_version = "0.13.0";

comptime {
    const expected_zig = "0.13.0";
    _ = expected_zig;
}
"#;

    let mut parser = ZigParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Pub struct
    assert!(names.contains(&"Package".to_string()));

    // Pub function (the build function)
    assert!(names.contains(&"build".to_string()));

    // Pub const values
    assert!(names.contains(&"version".to_string()));
    assert!(names.contains(&"min_zig_version".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"std".to_string()));
    assert!(result.metadata.imports.contains(&"builtin".to_string()));

    // No dependencies (no relative imports)
    assert!(result.metadata.dependencies.is_empty());

    // Custom fields: comptime but no tests
    let fields = result.custom_fields.unwrap();
    assert_eq!(fields.get("comptime_blocks").unwrap().as_u64().unwrap(), 1);
    assert!(!fields.contains_key("test_blocks"));
}

// =============================================================================
// Lua validation — Neovim plugin pattern
// =============================================================================

/// Neovim plugin pattern — typical Lua plugin module for Neovim.
/// Inspired by common Neovim plugin structure (telescope, nvim-cmp, etc.).
#[test]
fn lua_real_neovim_plugin_pattern() {
    let source = r#"local M = {}

local api = require("vim.api")
local fn = require("vim.fn")
local config = require("./config")

function M.setup(opts)
    opts = opts or {}
    M.config = vim.tbl_deep_extend("force", M.defaults, opts)
end

function M.run(args)
    if not M.config then
        error("Plugin not configured. Call setup() first.")
    end
    return M.config
end

function M.get_status()
    return { active = true, version = "1.0" }
end

local function validate_opts(opts)
    return type(opts) == "table"
end

local function apply_highlights()
    api.nvim_set_hl(0, "MyPluginHL", { fg = "white" })
end

return M
"#;

    let mut parser = LuaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Module methods
    assert!(names.contains(&"setup".to_string()));
    assert!(names.contains(&"run".to_string()));
    assert!(names.contains(&"get_status".to_string()));

    // Local functions excluded
    assert!(!names.contains(&"validate_opts".to_string()));
    assert!(!names.contains(&"apply_highlights".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"vim.api".to_string()));
    assert!(result.metadata.imports.contains(&"vim.fn".to_string()));

    // Dependencies
    assert!(result
        .metadata
        .dependencies
        .contains(&"./config".to_string()));
}

// =============================================================================
// Lua validation — Love2D game pattern
// =============================================================================

/// Love2D game pattern — typical Love2D game module with global callbacks.
/// Inspired by Love2D game structure.
#[test]
fn lua_real_love2d_game_pattern() {
    let source = r#"local physics = require("love.physics")
local graphics = require("love.graphics")

local world
local player = { x = 0, y = 0, speed = 200 }

function love_load()
    world = physics.newWorld(0, 9.81 * 64, true)
end

function love_update(dt)
    world:update(dt)
    player.x = player.x + player.speed * dt
end

function love_draw()
    graphics.print("Hello World", 400, 300)
    graphics.circle("fill", player.x, player.y, 20)
end

local function reset_player()
    player.x = 0
    player.y = 0
end

local function check_bounds(x, y)
    return x >= 0 and x <= 800 and y >= 0 and y <= 600
end
"#;

    let mut parser = LuaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Global functions (Love2D callbacks)
    assert!(names.contains(&"love_load".to_string()));
    assert!(names.contains(&"love_update".to_string()));
    assert!(names.contains(&"love_draw".to_string()));

    // Local functions excluded
    assert!(!names.contains(&"reset_player".to_string()));
    assert!(!names.contains(&"check_bounds".to_string()));

    // Imports
    assert!(result
        .metadata
        .imports
        .contains(&"love.physics".to_string()));
    assert!(result
        .metadata
        .imports
        .contains(&"love.graphics".to_string()));

    // No dependencies
    assert!(result.metadata.dependencies.is_empty());
}

// =============================================================================
// Scala validation — Akka actor pattern
// =============================================================================

/// Akka actor pattern — typed actors with message handling.
/// Inspired by Akka actor patterns.
#[test]
fn scala_real_akka_actor_pattern() {
    let source = r#"package com.example.actors

import akka.actor.Actor
import akka.actor.Props
import scala.collection.mutable

case class ProcessMessage(data: String)
case class ResultMessage(result: String)

trait MessageHandler {
  def handle(msg: Any): Unit
}

class DataActor extends Actor with MessageHandler {
  private val buffer = mutable.ListBuffer.empty[String]

  override def receive: Receive = {
    case ProcessMessage(data) => sender() ! ResultMessage(data.toUpperCase)
    case _ => ()
  }

  override def handle(msg: Any): Unit = ()
  private def cleanup(): Unit = buffer.clear()
}

object DataActor {
  def props: Props = Props(new DataActor)
  val MAX_BUFFER: Int = 1000
}
"#;

    let mut parser = ScalaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Case classes (messages)
    assert!(names.contains(&"ProcessMessage".to_string()));
    assert!(names.contains(&"ResultMessage".to_string()));

    // Trait
    assert!(names.contains(&"MessageHandler".to_string()));

    // Class
    assert!(names.contains(&"DataActor".to_string()));

    // Object
    assert!(names.contains(&"DataActor".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"akka".to_string()));
    assert!(result.metadata.imports.contains(&"scala".to_string()));

    // Custom fields: case_classes
    let fields = result.custom_fields.unwrap();
    let cc = fields.get("case_classes").unwrap().as_array().unwrap();
    assert_eq!(cc.len(), 2);
}

// =============================================================================
// Scala validation — Spark job pattern
// =============================================================================

/// Spark job pattern — typical Spark data processing pipeline.
/// Inspired by Apache Spark job structures.
#[test]
fn scala_real_spark_job_pattern() {
    let source = r#"package com.example.spark

import org.apache.spark.SparkConf
import org.apache.spark.sql.SparkSession
import org.apache.spark.sql.DataFrame

case class JobConfig(
  appName: String,
  master: String,
  inputPath: String,
  outputPath: String
)

object SparkJob {
  def main(args: Array[String]): Unit = {
    val config = JobConfig("MyJob", "local[*]", args(0), args(1))
    val spark = createSession(config)
    val df = loadData(spark, config.inputPath)
    val result = transformData(df)
    saveData(result, config.outputPath)
    spark.stop()
  }

  def createSession(config: JobConfig): SparkSession = {
    SparkSession.builder()
      .appName(config.appName)
      .master(config.master)
      .getOrCreate()
  }

  private def loadData(spark: SparkSession, path: String): DataFrame = {
    spark.read.parquet(path)
  }

  private def saveData(df: DataFrame, path: String): Unit = {
    df.write.parquet(path)
  }
}

def transformData(df: DataFrame): DataFrame = df

implicit val defaultConfig: JobConfig =
  JobConfig("default", "local", "/input", "/output")
"#;

    let mut parser = ScalaParser::new().unwrap();
    let result = parser.parse(source).unwrap();

    let names = result.metadata.export_names();

    // Case class
    assert!(names.contains(&"JobConfig".to_string()));

    // Object
    assert!(names.contains(&"SparkJob".to_string()));

    // Top-level function
    assert!(names.contains(&"transformData".to_string()));

    // Implicit val
    assert!(names.contains(&"defaultConfig".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"org".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert!(fields.contains_key("case_classes"));
    assert_eq!(fields.get("implicits").unwrap().as_u64().unwrap(), 1);
}
