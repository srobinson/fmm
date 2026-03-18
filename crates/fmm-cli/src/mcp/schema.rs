use serde_json::Value;

#[path = "generated_schema.rs"]
mod generated_schema;

pub(super) fn tool_list() -> Value {
    generated_schema::generated_tool_list()
}
