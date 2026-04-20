use super::super::dependency_graph;
use super::support::manifest_with_imports;

#[test]
fn dependency_graph_no_ghost_from_scoped_package_imports() {
    let manifest = manifest_with_imports(vec![
        ("src/logger/transient-logger.service.ts", vec![], vec![]),
        (
            "src/nest-factory.ts",
            vec![],
            vec![
                "@nestjs/common",
                "@nestjs/common/services/logger.service",
                "rxjs",
            ],
        ),
    ]);
    let entry = manifest.files["src/nest-factory.ts"].clone();

    let (local, external, _) = dependency_graph(&manifest, "src/nest-factory.ts", &entry);

    assert!(
        local.is_empty(),
        "scoped package imports must not resolve to local files, got local: {:?}",
        local
    );
    assert!(
        external.contains(&"@nestjs/common".to_string()),
        "external should contain @nestjs/common, got: {:?}",
        external
    );
    assert!(
        external.contains(&"@nestjs/common/services/logger.service".to_string()),
        "external should contain deep package path, got: {:?}",
        external
    );
}
