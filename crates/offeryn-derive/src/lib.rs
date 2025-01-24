use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Expr, FnArg, ImplItem, ItemImpl, Lit, Meta, Pat, ReturnType, Type,
};

fn extract_doc_string(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(expr_lit) => match &expr_lit.lit {
                    Lit::Str(lit_str) => Some(lit_str.value().trim().to_string()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_type_schema(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(type_path) => {
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
        _ => {}
    }
    quote! {
        <#ty as schemars::JsonSchema>::json_schema(&mut schemars::gen::SchemaGenerator::default())
    }
}

fn is_optional_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path) if type_path.path.segments.last()
        .map_or(false, |segment| segment.ident == "Option"))
}

fn extract_param_doc(docs: &str, param_name: &str) -> String {
    docs.lines()
        .find(|line| {
            line.contains(&format!("`{}`", param_name))
                || line.contains(&format!("* {}", param_name))
                || line.contains(&format!("- {}", param_name))
        })
        .map(|line| {
            let line = line.trim().trim_start_matches(['*', '-']).trim();
            let line = if let Some(idx) = line.find(&format!("`{}`", param_name)) {
                &line[idx..]
            } else if let Some(idx) = line.find(param_name) {
                &line[idx..]
            } else {
                line
            };
            line.trim_start_matches('`')
                .trim_start_matches(param_name)
                .trim_start_matches('`')
                .trim_start_matches('-')
                .trim_start_matches(" - ")
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

fn generate_param_schema(
    param_type: &Type,
    param_name: &str,
    param_doc: &str,
    is_optional: bool,
) -> proc_macro2::TokenStream {
    let schema = get_type_schema(param_type);
    let param_name_str = param_name.to_string();

    quote! {
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
        if !#is_optional {
            required.push(#param_name_str.to_string());
        }
    }
}

fn generate_param_deserialization(param_name: &str, is_optional: bool) -> proc_macro2::TokenStream {
    let name_str = param_name.to_string();
    if is_optional {
        quote! {
            match args.get(#name_str) {
                Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| e.to_string())?),
                None => None
            }
        }
    } else {
        quote! {
            serde_json::from_value(
                args.get(#name_str)
                    .ok_or_else(|| format!("Missing required parameter: {}", #name_str))?
                    .clone()
            ).map_err(|e| e.to_string())?
        }
    }
}

#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    let ty = &*input.self_ty;

    let type_name = if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .expect("Expected a type with at least one segment")
    } else {
        panic!("Expected a path type")
    };

    let mut tool_impls = Vec::new();
    let mut tool_names = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            let method_name = &method.sig.ident;
            let tool_name = format!("{}_{}", type_name.to_case(Case::Snake), method_name);
            let tool_struct_name = format_ident!(
                "{}{}Tool",
                type_name.to_case(Case::Pascal),
                method_name.to_string().to_case(Case::Pascal)
            );
            tool_names.push(tool_struct_name.clone());

            let docs = extract_doc_string(&method.attrs);

            let mut param_schemas = Vec::new();
            let mut param_desers = Vec::new();

            for param in &method.sig.inputs {
                if let FnArg::Typed(pat_type) = param {
                    if let Pat::Ident(param_name) = &*pat_type.pat {
                        if param_name.ident != "self" {
                            let param_type = &*pat_type.ty;
                            let name_str = param_name.ident.to_string();
                            let is_optional = is_optional_type(param_type);
                            let param_doc = extract_param_doc(&docs, &name_str);

                            param_schemas.push(generate_param_schema(
                                param_type,
                                &name_str,
                                &param_doc,
                                is_optional,
                            ));
                            param_desers
                                .push(generate_param_deserialization(&name_str, is_optional));
                        }
                    }
                }
            }

            let is_result = matches!(&method.sig.output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(p) if p.path.segments.last().map_or(false, |s| s.ident == "Result")));

            let result_handling = if is_result {
                quote! {
                    match result {
                        Ok(result) => Ok(offeryn_types::ToolResult {
                            content: vec![offeryn_types::ToolContent {
                                r#type: "text".to_string(),
                                text: result.to_string(),
                            }],
                            is_error: false,
                        }),
                        Err(e) => Ok(offeryn_types::ToolResult {
                            content: vec![offeryn_types::ToolContent {
                                r#type: "text".to_string(),
                                text: e.to_string(),
                            }],
                            is_error: true,
                        })
                    }
                }
            } else {
                quote! {
                    Ok(offeryn_types::ToolResult {
                        content: vec![offeryn_types::ToolContent {
                            r#type: "text".to_string(),
                            text: result.to_string(),
                        }],
                        is_error: false,
                    })
                }
            };

            let execute_impl = if param_desers.is_empty() {
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
                    let result = self.inner.#method_name(#(#param_desers),*).await;
                    #result_handling
                }
            };

            let schema_impl = if param_schemas.is_empty() {
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
                #[doc(hidden)]
                pub struct #tool_struct_name {
                    inner: std::sync::Arc<#ty>,
                }

                impl #tool_struct_name {
                    pub fn new(inner: std::sync::Arc<#ty>) -> Self {
                        Self { inner }
                    }
                }

                #[async_trait::async_trait]
                impl offeryn_types::McpTool for #tool_struct_name {
                    fn name(&self) -> &str { #tool_name }
                    fn description(&self) -> &str { #docs }
                    fn input_schema(&self) -> serde_json::Value { #schema_impl }
                    async fn execute(&self, args: serde_json::Value) -> Result<offeryn_types::ToolResult, String> {
                        #execute_impl
                    }
                }
            };

            tool_impls.push(tool_impl);
        }
    }

    TokenStream::from(quote! {
        #input

        impl offeryn_types::HasTools for #ty {
            type Tools = Vec<Box<dyn offeryn_types::McpTool>>;
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
