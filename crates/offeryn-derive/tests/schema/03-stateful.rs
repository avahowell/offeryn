use offeryn_derive::tool;
use offeryn_types::*;
use std::sync::atomic::{AtomicI64, Ordering};

/// A counter demonstrating stateful tools
#[derive(Default)]
struct Counter {
    count: AtomicI64,
}

#[tool]
impl Counter {
    /// Get the current count
    async fn get(&self) -> Result<i64, String> {
        Ok(self.count.load(Ordering::SeqCst))
    }

    /// Increment the counter by a value
    ///
    /// # Parameters
    /// * `by` - Amount to increment by
    async fn increment(&self, by: i64) -> Result<i64, String> {
        let new_value = self.count.fetch_add(by, Ordering::SeqCst) + by;
        Ok(new_value)
    }
}

#[tokio::main]
async fn main() {
    let counter = Counter::default();
    let tools = counter.tools();

    // Test get tool
    let get_tool = &tools[0];
    let get_schema = get_tool.input_schema();
    let get_schema_str = serde_json::to_string_pretty(&get_schema).unwrap();
    println!("Get Schema: {}", get_schema_str);

    // Verify get schema has no required parameters
    let schema: serde_json::Value = serde_json::from_str(&get_schema_str).unwrap();
    assert_eq!(schema["type"], "object");
    assert!(schema["required"].as_array().unwrap().is_empty());

    // Test increment tool
    let increment_tool = &tools[1];
    let increment_schema = increment_tool.input_schema();
    let increment_schema_str = serde_json::to_string_pretty(&increment_schema).unwrap();
    println!("Increment Schema: {}", increment_schema_str);

    // Verify increment schema
    let schema: serde_json::Value = serde_json::from_str(&increment_schema_str).unwrap();
    assert_eq!(schema["type"], "object");

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 1);

    let by_prop = &properties["by"];
    assert_eq!(by_prop["type"], "integer");
    assert_eq!(by_prop["format"], "int64");
    assert_eq!(by_prop["description"], "Amount to increment by");

    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert!(required.contains(&serde_json::json!("by")));

    // Test actual execution
    let args = serde_json::json!({});
    let result = get_tool.execute(args).await.unwrap();
    assert_eq!(result.content[0].text, "0");

    let args = serde_json::json!({
        "by": 5
    });
    let result = increment_tool.execute(args).await.unwrap();
    assert_eq!(result.content[0].text, "5");

    let args = serde_json::json!({});
    let result = get_tool.execute(args).await.unwrap();
    assert_eq!(result.content[0].text, "5");
}
