use super::super::{SearchFilters, filter_search};
use super::support::{manifest_with, manifest_with_imports};

#[test]
fn depends_on_with_extension_equals_without() {
    let manifest = manifest_with(vec![
        ("src/db/schema.ts", vec![]),
        ("src/routes/users.ts", vec!["../db/schema"]),
        ("src/routes/posts.ts", vec!["../db/schema.ts"]),
        ("src/services/auth.ts", vec!["../db/schema"]),
    ]);

    let filters_with_ext = SearchFilters {
        export: None,
        imports: None,
        depends_on: Some("src/db/schema.ts".to_string()),
        min_loc: None,
        max_loc: None,
    };
    let filters_without_ext = SearchFilters {
        export: None,
        imports: None,
        depends_on: Some("src/db/schema".to_string()),
        min_loc: None,
        max_loc: None,
    };

    let results_with = filter_search(&manifest, &filters_with_ext);
    let results_without = filter_search(&manifest, &filters_without_ext);

    let files_with: Vec<&str> = results_with.iter().map(|r| r.file.as_str()).collect();
    let files_without: Vec<&str> = results_without.iter().map(|r| r.file.as_str()).collect();

    assert_eq!(
        results_with.len(),
        results_without.len(),
        "extension vs no-extension should return same count; with: {:?}, without: {:?}",
        files_with,
        files_without
    );

    for file in &files_with {
        assert!(
            files_without.contains(file),
            "file {:?} in with-ext results but not in without-ext; without: {:?}",
            file,
            files_without
        );
    }

    assert!(
        files_with.contains(&"src/routes/users.ts"),
        "users.ts should match; got: {:?}",
        files_with
    );
    assert!(
        files_with.contains(&"src/routes/posts.ts"),
        "posts.ts should match; got: {:?}",
        files_with
    );
    assert!(
        files_with.contains(&"src/services/auth.ts"),
        "auth.ts should match; got: {:?}",
        files_with
    );
}

#[test]
fn imports_filter_local_path_checks_dependencies() {
    let manifest = manifest_with(vec![
        ("src/db/client.ts", vec![]),
        ("src/routes/users.ts", vec!["../db/client"]),
        ("src/services/auth.ts", vec!["../db/client"]),
    ]);

    let filters = SearchFilters {
        export: None,
        imports: Some("src/db/client".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };

    let results = filter_search(&manifest, &filters);
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&"src/routes/users.ts"),
        "users.ts should match local-path imports filter; got: {:?}",
        files
    );
    assert!(
        files.contains(&"src/services/auth.ts"),
        "auth.ts should match local-path imports filter; got: {:?}",
        files
    );
    assert!(
        !files.contains(&"src/db/client.ts"),
        "client.ts should not match; got: {:?}",
        files
    );
}

#[test]
fn imports_filter_external_package_unaffected() {
    let manifest = manifest_with_imports(vec![
        ("src/utils.ts", vec![], vec!["lodash"]),
        ("src/app.ts", vec![], vec!["lodash", "react"]),
        ("src/pure.ts", vec![], vec![]),
    ]);

    let filters = SearchFilters {
        export: None,
        imports: Some("lodash".to_string()),
        depends_on: None,
        min_loc: None,
        max_loc: None,
    };

    let results = filter_search(&manifest, &filters);
    let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();

    assert!(
        files.contains(&"src/utils.ts"),
        "utils.ts imports lodash; got: {:?}",
        files
    );
    assert!(
        files.contains(&"src/app.ts"),
        "app.ts imports lodash; got: {:?}",
        files
    );
    assert!(
        !files.contains(&"src/pure.ts"),
        "pure.ts does not import lodash; got: {:?}",
        files
    );
}
