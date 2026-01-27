use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Languages to process
    #[serde(default = "default_languages")]
    pub languages: HashSet<String>,

    /// Format for frontmatter
    #[serde(default = "default_format")]
    pub format: FrontmatterFormat,

    /// Include LOC (lines of code)
    #[serde(default = "default_true")]
    pub include_loc: bool,

    /// Include complexity metrics
    #[serde(default)]
    pub include_complexity: bool,

    /// Maximum file size to process (in KB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontmatterFormat {
    Yaml,
    Json,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            languages: default_languages(),
            format: default_format(),
            include_loc: true,
            include_complexity: false,
            max_file_size: 1024, // 1MB
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Path::new(".fmmrc.json");
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn is_supported_language(&self, extension: &str) -> bool {
        self.languages.contains(extension)
    }

    pub fn language_from_extension(&self, extension: &str) -> Option<Language> {
        match extension {
            "ts" | "tsx" => Some(Language::TypeScript),
            "js" | "jsx" => Some(Language::JavaScript),
            "py" => Some(Language::Python),
            "rs" => Some(Language::Rust),
            "go" => Some(Language::Go),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Language {
    TypeScript,
    JavaScript,
    Python,
    Rust,
    Go,
}

impl Language {
    pub fn comment_prefix(&self) -> &'static str {
        match self {
            Language::TypeScript | Language::JavaScript | Language::Rust | Language::Go => "//",
            Language::Python => "#",
        }
    }
}

fn default_languages() -> HashSet<String> {
    ["ts", "tsx", "js", "jsx", "py", "rs", "go"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_format() -> FrontmatterFormat {
    FrontmatterFormat::Yaml
}

fn default_true() -> bool {
    true
}

fn default_max_file_size() -> usize {
    1024 // 1MB in KB
}
