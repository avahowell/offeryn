use proc_macro::TokenStream;
use quote::{quote, format_ident, ToTokens};
use syn::{
    parse_macro_input,
    ItemImpl,
    Type,
    ReturnType,
    Pat,
    Attribute,
    Meta,
    FnArg,
    ImplItem,
    Expr,
    Lit,
};
use convert_case::{Case, Casing};

fn extract_doc_string(attrs: &[Attribute]) -> String {
    attrs.iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            if let Meta::NameValue(meta) = &attr.meta {
                if let Expr::Lit(expr_lit) = &meta.value {
                    if let Lit::Str(lit_str) = &expr_lit.lit {
                        Some(lit_str.value().trim().to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_type_schema(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(ok_type)) = args.args.first() {
                        return get_type_schema(ok_type);
                    }
                }
            }
        }
    }
    quote! {
        <#ty as schemars::JsonSchema>::json_schema(&mut schemars::gen::SchemaGenerator::default())
    }
}

#[proc_macro_attribute]
pub fn mcp_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let ty = &input.self_ty;
    let vis = quote!(pub);  // impl blocks don't have visibility
    
    let mut tool_impls = Vec::new();
    let mut tool_names = Vec::new();
    
    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            let method_name = &method.sig.ident;
            let tool_name = format!("{}_{}", ty.to_token_stream().to_string().to_case(Case::Snake), method_name);
            let tool_struct_name = format_ident!("{}{}Tool", ty.to_token_stream().to_string().to_case(Case::Pascal), method_name.to_string().to_case(Case::Pascal));
            tool_names.push(tool_struct_name.clone());
            
            println!("Processing method: {}", method_name);
            println!("Method attributes: {:#?}", method.attrs);
            
            let docs = extract_doc_string(&method.attrs);
            println!("Final docs for {}: {:?}", method_name, docs);
            
            let mut param_schemas = Vec::new();
            let mut param_names = Vec::new();
            
            for param in &method.sig.inputs {
                if let FnArg::Typed(pat_type) = param {
                    if let Pat::Ident(param_name) = &*pat_type.pat {
                        if param_name.ident != "self" {
                            let param_type = &*pat_type.ty;
                            let param_doc = docs.lines()
                                .find(|line| line.contains(&format!("* `{}`", param_name.ident)))
                                .unwrap_or("")
                                .trim()
                                .trim_start_matches(&format!("* `{}`", param_name.ident))
                                .trim_start_matches('-')
                                .trim_start_matches(" - ")
                                .trim();
                            
                            let param_name_str = param_name.ident.to_string();
                            param_names.push(param_name.ident.clone());
                            
                            let schema = get_type_schema(param_type);
                            param_schemas.push(quote! {
                                let schema = #schema;
                                if let schemars::schema::Schema::Object(mut obj) = schema {
                                    if let Some(meta) = &mut obj.metadata {
                                        meta.description = Some(#param_doc.to_string());
                                    } else {
                                        obj.metadata = Some(Box::new(schemars::schema::Metadata {
                                            description: Some(#param_doc.to_string()),
                                            ..Default::default()
                                        }));
                                    }
                                    properties.insert(#param_name_str.to_string(), schemars::schema::Schema::Object(obj));
                                } else {
                                    properties.insert(#param_name_str.to_string(), schema);
                                }
                                required.push(#param_name_str.to_string());
                            });
                        }
                    }
                }
            }
            
            let param_deserialization = param_names.iter().map(|name| {
                let name_str = name.to_string();
                quote! { serde_json::from_value(args[#name_str].clone()).map_err(|e| e.to_string())? }
            });
            
            let is_result = matches!(&method.sig.output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(p) if p.path.segments.last().map_or(false, |s| s.ident == "Result")));
            
            let result_handling = if is_result {
                quote! {
                    match result {
                        Ok(result) => Ok(mcp_types::ToolResult {
                            content: vec![mcp_types::ToolContent {
                                r#type: "text".to_string(),
                                text: result.to_string(),
                            }],
                            is_error: false,
                        }),
                        Err(e) => Ok(mcp_types::ToolResult {
                            content: vec![mcp_types::ToolContent {
                                r#type: "text".to_string(),
                                text: e.to_string(),
                            }],
                            is_error: true,
                        })
                    }
                }
            } else {
                quote! {
                    Ok(mcp_types::ToolResult {
                        content: vec![mcp_types::ToolContent {
                            r#type: "text".to_string(),
                            text: result.to_string(),
                        }],
                        is_error: false,
                    })
                }
            };
            
            let execute_impl = if param_names.is_empty() {
                quote! {
                    let args = args.as_object().ok_or("Expected object")?;
                    if !args.is_empty() {
                        return Err("Expected no arguments".to_string());
                    }
                    let result = self.inner.#method_name().await;
                    #result_handling
                }
            } else {
                quote! {
                    let args = args.as_object().ok_or("Expected object")?;
                    let result = self.inner.#method_name(#(#param_deserialization),*).await;
                    #result_handling
                }
            };
            
            let schema_impl = if param_names.is_empty() {
                quote! {
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    })
                }
            } else {
                quote! {
                    {
                        use std::collections::HashMap;
                        let mut properties = HashMap::new();
                        let mut required = Vec::new();
                        #(#param_schemas)*
                        serde_json::json!({
                            "type": "object",
                            "properties": properties,
                            "required": required
                        })
                    }
                }
            };
            
            let tool_impl = quote! {
                #vis struct #tool_struct_name {
                    inner: std::sync::Arc<#ty>,
                }
                
                impl #tool_struct_name {
                    #vis fn new(inner: std::sync::Arc<#ty>) -> Self {
                        Self { inner }
                    }
                }
                
                #[async_trait::async_trait]
                impl mcp_types::McpTool for #tool_struct_name {
                    fn name(&self) -> &str { #tool_name }
                    fn description(&self) -> &str { #docs }
                    fn input_schema(&self) -> serde_json::Value { #schema_impl }
                    async fn execute(&self, args: serde_json::Value) -> Result<mcp_types::ToolResult, String> {
                        #execute_impl
                    }
                }
            };
            
            tool_impls.push(tool_impl);
        }
    }
    
    TokenStream::from(quote! {
        #input
        
        impl mcp_types::HasTools for #ty {
            type Tools = Vec<Box<dyn mcp_types::McpTool>>;
            fn tools(self) -> Self::Tools {
                let this = std::sync::Arc::new(self);
                vec![
                    #(Box::new(#tool_names::new(this.clone()))),*
                ]
            }
        }
        
        #(#tool_impls)*
    })
}
