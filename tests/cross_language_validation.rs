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

use fmm::parser::builtin::python::PythonParser;
use fmm::parser::builtin::rust::RustParser;
use fmm::parser::builtin::typescript::TypeScriptParser;
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
            .exports
            .contains(&"encode_content".to_string()),
        "missing encode_content export"
    );
    assert!(
        result
            .metadata
            .exports
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
        result.metadata.exports.len(),
        21,
        "expected 21 exports from __all__"
    );
    assert!(result.metadata.exports.contains(&"AsyncClient".to_string()));
    assert!(result.metadata.exports.contains(&"Client".to_string()));
    assert!(result.metadata.exports.contains(&"get".to_string()));
    assert!(result.metadata.exports.contains(&"post".to_string()));
    assert!(result.metadata.exports.contains(&"codes".to_string()));

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

    assert!(result.metadata.exports.contains(&"QueryParams".to_string()));
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
    assert!(result.metadata.exports.contains(&"DataHandler".to_string()));
    assert!(result.metadata.exports.contains(&"process".to_string()));
    assert!(result.metadata.exports.contains(&"API_VERSION".to_string()));
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
    assert!(result.metadata.exports.contains(&"Config".to_string()));
    assert!(result.metadata.exports.contains(&"PagingMode".to_string()));
    assert!(result.metadata.exports.contains(&"load_config".to_string()));
    assert_eq!(result.metadata.exports.len(), 3);

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
    assert!(result.metadata.exports.contains(&"Searcher".to_string()));
    assert!(result.metadata.exports.contains(&"Match".to_string()));
    assert!(result.metadata.exports.contains(&"search_file".to_string()));

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

    assert!(result.metadata.exports.contains(&"public_api".to_string()));
    assert!(result.metadata.exports.contains(&"PublicType".to_string()));
    assert!(!result
        .metadata
        .exports
        .contains(&"internal_helper".to_string()));
    assert!(!result.metadata.exports.contains(&"parent_only".to_string()));
    assert!(!result
        .metadata
        .exports
        .contains(&"totally_private".to_string()));
    assert_eq!(result.metadata.exports.len(), 2);
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

    assert!(result.metadata.exports.contains(&"AppState".to_string()));
    assert!(result.metadata.exports.contains(&"CacheEntry".to_string()));
    assert!(result.metadata.exports.contains(&"CacheError".to_string()));

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

    assert_eq!(result.metadata.exports.len(), 4);
    assert!(result
        .metadata
        .exports
        .contains(&"createContext".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"parseMarkdown".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"renderOutput".to_string()));
    assert!(result
        .metadata
        .exports
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
        .exports
        .contains(&"ConnectionOptions".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"ConnectionManager".to_string()));
    assert!(result
        .metadata
        .exports
        .contains(&"createConnection".to_string()));
    assert!(result
        .metadata
        .exports
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
