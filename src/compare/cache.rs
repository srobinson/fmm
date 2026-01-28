//! Result caching layer for comparison runs

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::report::ComparisonReport;
use super::runner::RunResult;

/// Cache key for result lookups
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKey {
    pub repo_url: String,
    pub commit_sha: String,
    pub task_id: String,
    pub variant: String,
}

impl CacheKey {
    pub fn new(repo_url: &str, commit_sha: &str, task_id: &str, variant: &str) -> Self {
        Self {
            repo_url: repo_url.to_string(),
            commit_sha: commit_sha.to_string(),
            task_id: task_id.to_string(),
            variant: variant.to_string(),
        }
    }

    /// Generate a filesystem-safe cache filename
    pub fn to_filename(&self) -> String {
        let url_hash = simple_hash(&self.repo_url);
        format!(
            "{}_{}_{}_{}",
            url_hash, self.commit_sha, self.task_id, self.variant
        )
    }
}

/// Cached result entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    pub key: CacheKey,
    pub result: RunResult,
    pub cached_at: String,
    pub expires_at: String,
}

/// Cache manager for comparison results
pub struct CacheManager {
    cache_dir: PathBuf,
    ttl: Duration,
    max_size_mb: u64,
    /// In-memory cache for current session
    memory_cache: HashMap<CacheKey, CachedResult>,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(cache_dir: Option<PathBuf>) -> Result<Self> {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("fmm")
                .join("compare")
        });

        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        Ok(Self {
            cache_dir,
            ttl: Duration::from_secs(7 * 24 * 3600), // 7 days
            max_size_mb: 100,
            memory_cache: HashMap::new(),
        })
    }

    /// Set cache TTL
    #[allow(dead_code)]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set max cache size
    #[allow(dead_code)]
    pub fn with_max_size(mut self, max_size_mb: u64) -> Self {
        self.max_size_mb = max_size_mb;
        self
    }

    /// Get a cached result
    pub fn get(&mut self, key: &CacheKey) -> Option<RunResult> {
        // Check memory cache first
        if let Some(cached) = self.memory_cache.get(key) {
            if !Self::is_expired(&cached.expires_at) {
                return Some(cached.result.clone());
            }
        }

        // Check disk cache
        let filename = key.to_filename();
        let cache_path = self.cache_dir.join(format!("{}.json", filename));

        if cache_path.exists() {
            if let Ok(content) = fs::read_to_string(&cache_path) {
                if let Ok(cached) = serde_json::from_str::<CachedResult>(&content) {
                    if !Self::is_expired(&cached.expires_at) {
                        // Update memory cache
                        self.memory_cache.insert(key.clone(), cached.clone());
                        return Some(cached.result);
                    } else {
                        // Clean up expired entry
                        let _ = fs::remove_file(&cache_path);
                    }
                }
            }
        }

        None
    }

    /// Store a result in cache
    pub fn set(&mut self, key: CacheKey, result: RunResult) -> Result<()> {
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::from_std(self.ttl).unwrap_or_default();

        let cached = CachedResult {
            key: key.clone(),
            result,
            cached_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        };

        // Store in memory
        self.memory_cache.insert(key.clone(), cached.clone());

        // Store on disk
        let filename = key.to_filename();
        let cache_path = self.cache_dir.join(format!("{}.json", filename));
        let json = serde_json::to_string_pretty(&cached)?;
        fs::write(&cache_path, json).context("Failed to write cache file")?;

        // Evict if needed
        self.evict_if_needed()?;

        Ok(())
    }

    /// Check if a cache key exists and is valid
    #[allow(dead_code)]
    pub fn has(&mut self, key: &CacheKey) -> bool {
        self.get(key).is_some()
    }

    /// Clear all cached results for a repository
    #[allow(dead_code)]
    pub fn clear_repo(&mut self, repo_url: &str) -> Result<u32> {
        let url_hash = simple_hash(repo_url);
        let mut cleared = 0u32;

        // Clear from memory
        self.memory_cache
            .retain(|k, _| !k.repo_url.contains(repo_url));

        // Clear from disk
        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with(&url_hash) {
                fs::remove_file(entry.path())?;
                cleared += 1;
            }
        }

        Ok(cleared)
    }

    /// Clear all cache
    #[allow(dead_code)]
    pub fn clear_all(&mut self) -> Result<u32> {
        self.memory_cache.clear();

        let mut cleared = 0u32;
        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                fs::remove_file(entry.path())?;
                cleared += 1;
            }
        }

        Ok(cleared)
    }

    /// Save a full comparison report
    pub fn save_report(&self, report: &ComparisonReport) -> Result<PathBuf> {
        let reports_dir = self.cache_dir.join("reports");
        fs::create_dir_all(&reports_dir)?;

        let filename = format!("{}.json", report.job_id);
        let report_path = reports_dir.join(filename);

        let json = serde_json::to_string_pretty(report)?;
        fs::write(&report_path, json)?;

        Ok(report_path)
    }

    /// Load a comparison report by job ID
    #[allow(dead_code)]
    pub fn load_report(&self, job_id: &str) -> Result<Option<ComparisonReport>> {
        let report_path = self
            .cache_dir
            .join("reports")
            .join(format!("{}.json", job_id));

        if !report_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&report_path)?;
        let report: ComparisonReport = serde_json::from_str(&content)?;

        Ok(Some(report))
    }

    /// List all cached reports
    #[allow(dead_code)]
    pub fn list_reports(&self) -> Result<Vec<String>> {
        let reports_dir = self.cache_dir.join("reports");
        if !reports_dir.exists() {
            return Ok(vec![]);
        }

        let mut reports = vec![];
        for entry in fs::read_dir(reports_dir)? {
            let entry = entry?;
            if let Some(name) = entry.path().file_stem() {
                reports.push(name.to_string_lossy().to_string());
            }
        }

        Ok(reports)
    }

    fn is_expired(expires_at: &str) -> bool {
        if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) {
            chrono::Utc::now() > expires
        } else {
            true
        }
    }

    fn evict_if_needed(&self) -> Result<()> {
        let total_size = self.calculate_cache_size()?;
        if total_size <= self.max_size_mb * 1_000_000 {
            return Ok(());
        }

        // Get all cache files with their modification times
        let mut entries: Vec<(PathBuf, SystemTime)> = vec![];
        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        entries.push((entry.path(), modified));
                    }
                }
            }
        }

        // Sort by modification time (oldest first)
        entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest entries until under limit
        let target_size = self.max_size_mb * 1_000_000 * 80 / 100; // Target 80%
        let mut current_size = total_size;

        for (path, _) in entries {
            if current_size <= target_size {
                break;
            }

            if let Ok(metadata) = fs::metadata(&path) {
                current_size -= metadata.len();
                let _ = fs::remove_file(&path);
            }
        }

        Ok(())
    }

    fn calculate_cache_size(&self) -> Result<u64> {
        let mut total = 0u64;

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total += metadata.len();
                }
            }
        }

        Ok(total)
    }

    /// Get cache directory path
    #[allow(dead_code)]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Simple string hash for cache filenames
fn simple_hash(s: &str) -> String {
    let mut hash = 0u64;
    for c in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(c as u64);
    }
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_test_result(task_id: &str, variant: &str) -> RunResult {
        RunResult {
            task_id: task_id.to_string(),
            variant: variant.to_string(),
            tool_calls: 5,
            tools_by_name: HashMap::new(),
            files_accessed: vec![],
            read_calls: 3,
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            total_cost_usd: 0.01,
            duration_ms: 1000,
            num_turns: 2,
            response: "test".to_string(),
            success: true,
            error: None,
        }
    }

    #[test]
    fn test_cache_set_and_get() {
        let temp = tempdir().unwrap();
        let mut cache = CacheManager::new(Some(temp.path().to_path_buf())).unwrap();

        let key = CacheKey::new("https://github.com/test/repo", "abc123", "task1", "control");
        let result = create_test_result("task1", "control");

        cache.set(key.clone(), result.clone()).unwrap();

        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved.task_id, result.task_id);
        assert_eq!(retrieved.tool_calls, result.tool_calls);
    }

    #[test]
    fn test_cache_key_filename() {
        let key = CacheKey::new("https://github.com/test/repo", "abc123", "task1", "fmm");
        let filename = key.to_filename();
        assert!(filename.contains("abc123"));
        assert!(filename.contains("task1"));
        assert!(filename.contains("fmm"));
    }
}
