pub mod builtin;

mod registry;
#[cfg(test)]
mod tests;
mod types;

pub use registry::ParserRegistry;
pub use types::{
    ExportEntry, LanguageTestPatterns, Metadata, ParseResult, Parser, RegisteredLanguage,
};
