use mcp_derive::mcp_tool;
use mcp_types::*;
use async_trait::async_trait;

/// A calculator trait
#[mcp_tool]
#[async_trait]
trait Calculator {
    /// Add two numbers
    /// 
    /// # Parameters
    /// * `a` - First operand
    /// * `b` - Second operand
    async fn add(&self, a: i64, b: i64) -> Result<i64, String> {
        Ok(a + b)
    }

    /// Multiply two numbers
    /// 
    /// # Parameters
    /// * `a` - First number
    /// * `b` - Second number
    async fn multiply(&self, a: i64, b: i64) -> Result<i64, String> {
        Ok(a * b)
    }
}

#[tokio::main]
async fn main() {
    let calc = CalculatorImpl::default();
    let tools = calc.into_tools();
    
    // Test add tool
    let add_tool = &tools.1[0];
    let add_schema = add_tool.input_schema();
    let add_schema_str = serde_json::to_string_pretty(&add_schema).unwrap();
    println!("Add Schema: {}", add_schema_str);
    
    // Verify add schema structure
    let schema: serde_json::Value = serde_json::from_str(&add_schema_str).unwrap();
    
    assert_eq!(schema["type"], "object");
    
    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 2);
    
    let a_prop = &properties["a"];
    assert_eq!(a_prop["type"], "integer");
    assert_eq!(a_prop["format"], "int64");
    assert_eq!(a_prop["description"], "First operand");
    
    let b_prop = &properties["b"]; 
    assert_eq!(b_prop["type"], "integer");
    assert_eq!(b_prop["format"], "int64");
    assert_eq!(b_prop["description"], "Second operand");
    
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&serde_json::json!("a")));
    assert!(required.contains(&serde_json::json!("b")));

    // Test multiply tool
    let multiply_tool = &tools.1[1];
    let multiply_schema = multiply_tool.input_schema();
    let multiply_schema_str = serde_json::to_string_pretty(&multiply_schema).unwrap();
    println!("Multiply Schema: {}", multiply_schema_str);
    
    // Verify multiply schema structure
    let schema: serde_json::Value = serde_json::from_str(&multiply_schema_str).unwrap();
    
    assert_eq!(schema["type"], "object");
    
    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 2);
    
    let a_prop = &properties["a"];
    assert_eq!(a_prop["type"], "integer"); 
    assert_eq!(a_prop["format"], "int64");
    assert_eq!(a_prop["description"], "First number");
    
    let b_prop = &properties["b"];
    assert_eq!(b_prop["type"], "integer");
    assert_eq!(b_prop["format"], "int64"); 
    assert_eq!(b_prop["description"], "Second number");
    
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 2);
    assert!(required.contains(&serde_json::json!("a")));
    assert!(required.contains(&serde_json::json!("b")));
}