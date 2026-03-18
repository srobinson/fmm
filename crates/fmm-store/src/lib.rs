//! Persistence layer for the fmm code index.
//!
//! Provides `SqliteStore`, the primary `FmmStore` implementation backed by
//! SQLite in WAL mode. Re-exports connection management and DB_FILENAME
//! for callers that need raw database access (e.g. status queries).

pub mod connection;
pub mod error;
#[cfg(any(test, feature = "test-support"))]
pub mod memory_store;
pub(crate) mod reader;
mod schema;
pub mod sqlite_store;
pub mod writer;

// Primary public API
pub use connection::DB_FILENAME;
pub use error::StoreError;
pub use sqlite_store::SqliteStore;

#[cfg(any(test, feature = "test-support"))]
pub use memory_store::{InMemoryStore, MemoryStoreError};

// Transitional re-exports: connection management
pub use connection::{open_db, open_or_create};
