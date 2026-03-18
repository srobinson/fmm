use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Resolve the root directory from the target path.
/// If a directory, use it directly. If a file, walk up from its parent
/// looking for project root markers (.git, .fmmrc.toml, .fmmrc.json) so that relative
/// paths in the index are consistent regardless of whether `fmm generate`
/// targets a single file or the whole repo.
/// Falls back to the file's parent directory, then CWD.
pub(crate) fn resolve_root(path: &str) -> Result<PathBuf> {
    let target = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    if target.is_dir() {
        Ok(target)
    } else if target.is_file() {
        match target.parent() {
            Some(parent) => Ok(find_project_root(parent).unwrap_or_else(|| parent.to_path_buf())),
            None => std::env::current_dir().context("Failed to get current directory"),
        }
    } else {
        std::env::current_dir().context("Failed to get current directory")
    }
}

/// Walk up from `start` looking for project root markers.
/// Returns the first directory containing `.git`, `.fmmrc.toml`, or `.fmmrc.json`.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists()
            || current.join(".fmmrc.toml").exists()
            || current.join(".fmmrc.json").exists()
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Resolve root from multiple paths: common ancestor if all exist, else CWD.
pub(crate) fn resolve_root_multi(paths: &[String]) -> Result<PathBuf> {
    if paths.len() == 1 {
        return resolve_root(&paths[0]);
    }

    let resolved: Vec<PathBuf> = paths.iter().filter_map(|p| resolve_root(p).ok()).collect();

    if resolved.is_empty() {
        return std::env::current_dir().context("Failed to get current directory");
    }

    // Find common ancestor
    let mut ancestor = resolved[0].clone();
    for path in &resolved[1..] {
        while !path.starts_with(&ancestor) {
            if !ancestor.pop() {
                return std::env::current_dir().context("Failed to get current directory");
            }
        }
    }
    Ok(ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_root_with_absolute_directory() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_root(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap());
        assert!(result.is_absolute());
    }

    #[test]
    fn resolve_root_with_relative_directory() {
        let result = resolve_root(".").unwrap();
        let expected = std::env::current_dir().unwrap().canonicalize().unwrap();
        assert_eq!(result, expected);
        assert!(result.is_absolute());
    }

    #[test]
    fn resolve_root_with_file_finds_project_root() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src").join("deep");
        std::fs::create_dir_all(&src).unwrap();
        // Place a .git marker at the tmp root
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let file_path = src.join("example.ts");
        std::fs::write(&file_path, "export const x = 1;").unwrap();

        let result = resolve_root(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, tmp.path().canonicalize().unwrap());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_root_with_file_falls_back_to_parent_without_markers() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("example.ts");
        std::fs::write(&file_path, "export const x = 1;").unwrap();

        let result = resolve_root(file_path.to_str().unwrap()).unwrap();
        assert!(result.is_dir());
        let parent = file_path.parent().unwrap().canonicalize().unwrap();
        assert!(parent.starts_with(&result) || result == parent);
    }

    #[test]
    fn resolve_root_nonexistent_path_falls_back_to_cwd() {
        let result = resolve_root("/surely/this/does/not/exist/anywhere").unwrap();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(result, cwd);
    }
}
