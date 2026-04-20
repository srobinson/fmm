use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ImportResolver;
use super::workspace::read_go_module_path;

/// Go workspace import resolver.
///
/// Resolves module import paths with Go's longest-prefix module matching.
/// The resolved path is a package directory because Go imports address
/// packages, not individual source files.
#[derive(Debug, Clone, Default)]
pub struct GoImportResolver {
    modules: Vec<GoModule>,
}

#[derive(Debug, Clone)]
struct GoModule {
    module_path: String,
    root: PathBuf,
}

impl GoImportResolver {
    pub fn new(workspace_packages: &HashMap<String, PathBuf>) -> Self {
        let mut modules: Vec<GoModule> = workspace_packages
            .values()
            .filter_map(|root| {
                let module_path = read_go_module_path(root)?;
                Some(GoModule {
                    module_path,
                    root: root.clone(),
                })
            })
            .collect();

        modules.sort_by(|a, b| {
            b.module_path
                .len()
                .cmp(&a.module_path.len())
                .then_with(|| a.module_path.cmp(&b.module_path))
        });

        Self { modules }
    }

    pub fn resolve(&self, _importer: &Path, specifier: &str) -> Option<PathBuf> {
        let module = self
            .modules
            .iter()
            .find(|module| module_path_matches(&module.module_path, specifier))?;
        let remainder = specifier.strip_prefix(&module.module_path).unwrap_or("");
        let remainder = remainder.strip_prefix('/').unwrap_or(remainder);
        let package_dir = if remainder.is_empty() {
            module.root.clone()
        } else {
            module.root.join(remainder)
        };

        package_dir.exists().then_some(package_dir)
    }
}

impl ImportResolver for GoImportResolver {
    fn resolve(&self, importer: &Path, specifier: &str) -> Option<PathBuf> {
        GoImportResolver::resolve(self, importer, specifier)
    }
}

fn module_path_matches(module_path: &str, specifier: &str) -> bool {
    specifier == module_path
        || specifier
            .strip_prefix(module_path)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(base: &Path, rel: &str, content: &str) -> PathBuf {
        let path = base.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    fn workspace_packages(tmp: &TempDir) -> HashMap<String, PathBuf> {
        let mut packages = HashMap::new();
        packages.insert(
            "github.com/acme/app".to_string(),
            tmp.path().join("services/app"),
        );
        packages.insert(
            "github.com/acme/app/tools".to_string(),
            tmp.path().join("services/tools"),
        );
        packages
    }

    fn write_workspace(tmp: &TempDir) {
        write_file(
            tmp.path(),
            "services/app/go.mod",
            "module github.com/acme/app\n\ngo 1.23.0\n",
        );
        write_file(
            tmp.path(),
            "services/app/internal/handler/handler.go",
            "package handler\n",
        );
        write_file(
            tmp.path(),
            "services/tools/go.mod",
            "module github.com/acme/app/tools\n\ngo 1.23.0\n",
        );
        write_file(tmp.path(), "services/tools/cli/cli.go", "package cli\n");
    }

    #[test]
    fn workspace_local_import_resolves_to_package_directory() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let resolver = GoImportResolver::new(&workspace_packages(&tmp));
        let importer = tmp.path().join("services/tools/cli/cli.go");

        assert_eq!(
            resolver.resolve(&importer, "github.com/acme/app/internal/handler"),
            Some(tmp.path().join("services/app/internal/handler"))
        );
    }

    #[test]
    fn standard_library_import_returns_none() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let resolver = GoImportResolver::new(&workspace_packages(&tmp));
        let importer = tmp.path().join("services/tools/cli/cli.go");

        assert_eq!(resolver.resolve(&importer, "fmt"), None);
        assert_eq!(resolver.resolve(&importer, "net/http"), None);
    }

    #[test]
    fn external_dependency_returns_none() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let resolver = GoImportResolver::new(&workspace_packages(&tmp));
        let importer = tmp.path().join("services/tools/cli/cli.go");

        assert_eq!(
            resolver.resolve(&importer, "golang.org/x/net/context"),
            None
        );
    }

    #[test]
    fn longest_module_prefix_wins() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let resolver = GoImportResolver::new(&workspace_packages(&tmp));
        let importer = tmp.path().join("services/app/internal/handler/handler.go");

        assert_eq!(
            resolver.resolve(&importer, "github.com/acme/app/tools/cli"),
            Some(tmp.path().join("services/tools/cli"))
        );
    }

    #[test]
    fn module_prefix_requires_path_boundary() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let resolver = GoImportResolver::new(&workspace_packages(&tmp));
        let importer = tmp.path().join("services/tools/cli/cli.go");

        assert_eq!(
            resolver.resolve(&importer, "github.com/acme/application/internal/handler"),
            None
        );
    }

    #[test]
    fn module_path_comes_from_go_mod_not_merged_workspace_key() {
        let tmp = TempDir::new().unwrap();
        write_workspace(&tmp);
        let mut packages = HashMap::new();
        packages.insert("api".to_string(), tmp.path().join("services/app"));
        let resolver = GoImportResolver::new(&packages);
        let importer = tmp.path().join("services/tools/cli/cli.go");

        assert_eq!(
            resolver.resolve(&importer, "github.com/acme/app/internal/handler"),
            Some(tmp.path().join("services/app/internal/handler"))
        );
        assert_eq!(resolver.resolve(&importer, "api/internal/handler"), None);
    }

    #[test]
    fn dotless_workspace_module_resolves_before_stdlib_classification() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "services/shared/go.mod",
            "module acme/shared\n\ngo 1.23.0\n",
        );
        write_file(
            tmp.path(),
            "services/shared/config/config.go",
            "package config\n",
        );
        let mut packages = HashMap::new();
        packages.insert(
            "acme/shared".to_string(),
            tmp.path().join("services/shared"),
        );
        let resolver = GoImportResolver::new(&packages);
        let importer = tmp.path().join("services/api/handler/handler.go");

        assert_eq!(
            resolver.resolve(&importer, "acme/shared/config"),
            Some(tmp.path().join("services/shared/config"))
        );
    }

    #[test]
    fn quoted_go_mod_module_path_resolves() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "services/quoted/go.mod",
            "module \"github.com/acme/quoted\"\n\ngo 1.23.0\n",
        );
        write_file(tmp.path(), "services/quoted/api/api.go", "package api\n");
        let mut packages = HashMap::new();
        packages.insert("quoted".to_string(), tmp.path().join("services/quoted"));
        let resolver = GoImportResolver::new(&packages);
        let importer = tmp.path().join("services/client/client.go");

        assert_eq!(
            resolver.resolve(&importer, "github.com/acme/quoted/api"),
            Some(tmp.path().join("services/quoted/api"))
        );
    }
}
