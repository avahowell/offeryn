use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemTrait, TraitItem, Type, ReturnType, Pat, Attribute, Meta};
use convert_case::{Case, Casing};

fn extract_doc_string(attrs: &[Attribute]) -> String {
    attrs.iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            if let Meta::NameValue(meta) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }) = &meta.value {
                    Some(lit.value().trim().to_string())
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
                    if let Some(first_arg) = args.args.first() {
                        if let syn::GenericArgument::Type(ok_type) = first_arg {
                            return quote! {
                                <#ok_type as schemars::JsonSchema>::json_schema(&mut schemars::gen::SchemaGenerator::default())
                            };
                        }
                    }
                }
            }
        }
    }
    quote! {
        <#ty as schemars::JsonSchema>::json_schema(&mut schemars::gen::SchemaGenerator::default())
    }
}

fn is_result_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Result";
        }
    }
    false
}

#[proc_macro_attribute]
pub fn mcp_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemTrait);
    let trait_name = &input.ident;
    let impl_name = format_ident!("{}Impl", trait_name);
    let vis = &input.vis;
    
    let mut tool_impls = Vec::new();
    let mut tool_names = Vec::new();
    let mut trait_items = Vec::new();
    
    for item in &input.items {
        if let TraitItem::Fn(method) = item {
            trait_items.push(quote! { #method });
            
            let method_name = &method.sig.ident;
            let tool_name = format!("{}_{}", trait_name.to_string().to_case(Case::Snake), method_name);
            let tool_struct_name = format_ident!("{}Tool", method_name.to_string().to_case(Case::Pascal));
            tool_names.push(tool_struct_name.clone());
            
            // Extract doc comments for description
            let docs = extract_doc_string(&method.attrs);
            
            // Generate input schema from method parameters
            let mut param_schemas = Vec::new();
            let mut param_names = Vec::new();
            for param in &method.sig.inputs {
                if let syn::FnArg::Typed(pat_type) = param {
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
                quote! {
                    serde_json::from_value(args[#name_str].clone()).map_err(|e| e.to_string())?
                }
            });
            
            let is_result = if let ReturnType::Type(_, ty) = &method.sig.output {
                is_result_type(ty)
            } else {
                false
            };
            
            let execute_impl = if is_result {
                quote! {
                    match impl_instance.#method_name(
                        #(#param_deserialization),*
                    ).await {
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
                    let result = impl_instance.#method_name(
                        #(#param_deserialization),*
                    ).await;
                    
                    Ok(mcp_types::ToolResult {
                        content: vec![mcp_types::ToolContent {
                            r#type: "text".to_string(),
                            text: result.to_string(),
                        }],
                        is_error: false,
                    })
                }
            };
            
            let tool_impl = quote! {
                #[derive(Default)]
                pub struct #tool_struct_name;
                
                #[async_trait::async_trait]
                impl mcp_types::McpTool for #tool_struct_name {
                    fn name(&self) -> &str {
                        #tool_name
                    }
                    
                    fn description(&self) -> &str {
                        #docs
                    }
                    
                    fn input_schema(&self) -> serde_json::Value {
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
                    
                    async fn execute(&self, args: serde_json::Value) -> Result<mcp_types::ToolResult, String> {
                        let impl_instance = #impl_name::default();
                        #execute_impl
                    }
                }
            };
            
            tool_impls.push(tool_impl);
        }
    }
    
    let expanded = quote! {
        #input
        
        #[derive(Default)]
        #vis struct #impl_name;
        
        #(#tool_impls)*
        
        impl mcp_types::IntoTools for #impl_name {
            fn into_tools(self) -> (&'static str, Vec<Box<dyn mcp_types::McpTool>>) {
                (
                    stringify!(#trait_name),
                    vec![
                        #(Box::new(#tool_names::default())),*
                    ]
                )
            }
        }
        
        #[async_trait::async_trait]
        impl #trait_name for #impl_name {
            #(#trait_items)*
        }
    };
    
    TokenStream::from(expanded)
}
