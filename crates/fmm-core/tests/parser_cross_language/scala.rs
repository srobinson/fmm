use crate::support::parse_with;
use fmm_core::parser::builtin::scala::ScalaParser;

// Scala validation

/// Akka actor pattern — typed actors with message handling.
/// Inspired by Akka actor patterns.
#[test]
fn scala_real_akka_actor_pattern() {
    let source = include_str!("fixtures/scala/scala_real_akka_actor_pattern.scala");
    let result = parse_with(ScalaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Case classes (messages)
    assert!(names.contains(&"ProcessMessage".to_string()));
    assert!(names.contains(&"ResultMessage".to_string()));

    // Trait
    assert!(names.contains(&"MessageHandler".to_string()));

    // Class
    assert!(names.contains(&"DataActor".to_string()));

    // Object
    assert!(names.contains(&"DataActor".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"akka".to_string()));
    assert!(result.metadata.imports.contains(&"scala".to_string()));

    // Custom fields: case_classes
    let fields = result.custom_fields.unwrap();
    let cc = fields.get("case_classes").unwrap().as_array().unwrap();
    assert_eq!(cc.len(), 2);
}

// =============================================================================
// Scala validation — Spark job pattern
// =============================================================================

/// Spark job pattern — typical Spark data processing pipeline.
/// Inspired by Apache Spark job structures.
#[test]
fn scala_real_spark_job_pattern() {
    let source = include_str!("fixtures/scala/scala_real_spark_job_pattern.scala");
    let result = parse_with(ScalaParser::new().unwrap(), source);

    let names = result.metadata.export_names();

    // Case class
    assert!(names.contains(&"JobConfig".to_string()));

    // Object
    assert!(names.contains(&"SparkJob".to_string()));

    // Top-level function
    assert!(names.contains(&"transformData".to_string()));

    // Implicit val
    assert!(names.contains(&"defaultConfig".to_string()));

    // Imports
    assert!(result.metadata.imports.contains(&"org".to_string()));

    // Custom fields
    let fields = result.custom_fields.unwrap();
    assert!(fields.contains_key("case_classes"));
    assert_eq!(fields.get("implicits").unwrap().as_u64().unwrap(), 1);
}
