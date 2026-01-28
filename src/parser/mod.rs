mod typescript;

pub use typescript::TypeScriptParser;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    pub loc: usize,
}

pub trait Parser {
    fn parse(&mut self, source: &str) -> Result<Metadata>;
}
