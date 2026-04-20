
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
