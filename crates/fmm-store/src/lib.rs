//! Persistence layer for the fmm code index.
//!
//! Provides `SqliteStore`, the primary `FmmStore` implementation backed by
//! SQLite in WAL mode. Also re-exports connection management and low-level
//! reader/writer functions for the transition period while call sites migrate
//! from direct DB access to the `FmmStore` trait.

pub mod connection;
pub mod error;
pub mod reader;
mod schema;
pub mod sqlite_store;
pub mod writer;

// Primary public API
pub use connection::DB_FILENAME;
pub use error::StoreError;
pub use sqlite_store::SqliteStore;

// Transitional re-exports: connection management
pub use connection::{open_db, open_or_create};
