use mcp_derive::mcp_tool;
use mcp_types::*;

/// A test struct with doc comments
#[derive(Default)]
struct TestStruct {}

#[mcp_tool]
impl TestStruct {
    /// This is a test method
    /// 
    /// # Parameters
    /// * `value` - A test parameter
    async fn test_method(&self, value: String) -> Result<String, String> {
        Ok(value)
    }

    #[doc = "Another test method"]
    #[doc = "with multiple doc attributes"]
    /// 
    /// # Parameters
    /// * `x` - First value
    /// * `y` - Second value
    async fn another_method(&self, x: i32, y: i32) -> Result<i32, String> {
        Ok(x + y)
    }

    /// A method with an optional parameter
    /// 
    /// # Parameters
    /// * `required` - A required parameter
    /// * `optional` - An optional parameter
    async fn optional_method(&self, required: String, optional: Option<i32>) -> Result<String, String> {
        match optional {
            Some(val) => Ok(format!("{} with {}", required, val)),
            None => Ok(required)
        }
    }
}

#[tokio::main]
async fn main() {
    let test = TestStruct::default();
    let tools = test.tools();
    
    // Test first method
    let test_tool = &tools[0];
    let test_schema = test_tool.input_schema();
    let test_schema_str = serde_json::to_string_pretty(&test_schema).unwrap();
    println!("Test Schema: {}", test_schema_str);
    
    // Verify test schema
    let schema: serde_json::Value = serde_json::from_str(&test_schema_str).unwrap();
    assert_eq!(schema["type"], "object");
    
    let properties = schema["properties"].as_object().unwrap();
    let value_prop = &properties["value"];
    assert_eq!(value_prop["type"], "string");
    assert_eq!(value_prop["description"], "A test parameter");
    
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert!(required.contains(&serde_json::json!("value")));
    
    // Test second method
    let another_tool = &tools[1];
    let another_schema = another_tool.input_schema();
    let another_schema_str = serde_json::to_string_pretty(&another_schema).unwrap();
    println!("Another Schema: {}", another_schema_str);
    
    // Verify another schema
    let schema: serde_json::Value = serde_json::from_str(&another_schema_str).unwrap();
    assert_eq!(schema["type"], "object");
    
    let properties = schema["properties"].as_object().unwrap();
    let x_prop = &properties["x"];
    assert_eq!(x_prop["type"], "integer");
    assert_eq!(x_prop["description"], "First value");
    
    let y_prop = &properties["y"];
    assert_eq!(y_prop["type"], "integer");
    assert_eq!(y_prop["description"], "Second value");
    
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&serde_json::json!("x")));
    assert!(required.contains(&serde_json::json!("y")));

    // Test optional method
    let optional_tool = &tools[2];
    let optional_schema = optional_tool.input_schema();
    let optional_schema_str = serde_json::to_string_pretty(&optional_schema).unwrap();
    println!("Optional Schema: {}", optional_schema_str);
    
    // Verify optional schema
    let schema: serde_json::Value = serde_json::from_str(&optional_schema_str).unwrap();
    assert_eq!(schema["type"], "object");
    
    let properties = schema["properties"].as_object().unwrap();
    
    // Check required parameter
    let required_prop = &properties["required"];
    assert_eq!(required_prop["type"], "string");
    assert_eq!(required_prop["description"], "A required parameter");
    
    // Check optional parameter
    let optional_prop = &properties["optional"];
    assert_eq!(optional_prop["description"], "An optional parameter");
    assert_eq!(optional_prop["type"], serde_json::json!(["integer", "null"]));
    assert_eq!(optional_prop["format"], "int32");
    
    // Verify required array only contains non-optional parameters
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert!(required.contains(&serde_json::json!("required")));
    assert!(!required.contains(&serde_json::json!("optional")));

    // Test execution
    let args = serde_json::json!({
        "required": "test"
    });
    let result = optional_tool.execute(args).await.unwrap();
    assert_eq!(result.content[0].text, "test");

    let args = serde_json::json!({
        "required": "test",
        "optional": 42
    });
    let result = optional_tool.execute(args).await.unwrap();
    assert_eq!(result.content[0].text, "test with 42");
} 