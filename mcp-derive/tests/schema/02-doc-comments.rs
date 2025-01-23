use mcp_derive::mcp_tool;
use mcp_types::*;
use async_trait::async_trait;

/// A test trait with doc comments
#[mcp_tool]
#[async_trait]
trait TestTrait {
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
}

#[tokio::main]
async fn main() {
    let test = TestTraitImpl::default();
    let tools = test.into_tools();
    
    // Test first method
    let test_tool = &tools.1[0];
    let test_schema = test_tool.input_schema();
    let test_schema_str = serde_json::to_string_pretty(&test_schema).unwrap();
    println!("Test Schema: {}", test_schema_str);
    
    // Verify test schema
    assert!(test_schema_str.contains("A test parameter"));
    assert!(test_schema_str.contains(r#""type": "string""#));
    assert!(test_schema_str.contains(r#""required": ["value"]"#));
    
    // Test second method
    let another_tool = &tools.1[1];
    let another_schema = another_tool.input_schema();
    let another_schema_str = serde_json::to_string_pretty(&another_schema).unwrap();
    println!("Another Schema: {}", another_schema_str);
    
    // Verify another schema
    assert!(another_schema_str.contains("First value"));
    assert!(another_schema_str.contains("Second value"));
    assert!(another_schema_str.contains(r#""type": "number""#));
    assert!(another_schema_str.contains(r#""required": ["x", "y"]"#));
} 