extern crate proc_macro;

use darling::{FromDeriveInput, FromField};
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type,
};

#[derive(FromDeriveInput, Default)]
#[darling(default, attributes(llm_tool))]
struct ToolOpts {
    name: Option<String>,
    description: Option<String>,
}

#[derive(FromField, Default)]
#[darling(default, attributes(llm_tool))]
struct FieldOpts {
    description: Option<String>,
    enum_lang: Option<String>,
}

#[proc_macro_derive(LlmTool, attributes(llm_tool))]
pub fn derive_llm_tool(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let opts = ToolOpts::from_derive_input(&input).expect("Wrong options");
    let ident = &input.ident;

    let tool_name = opts.name.unwrap_or_else(|| ident.to_string());
    let tool_description = opts.description.unwrap_or_default();

    let (struct_fields, from_tool_call_impl) = match &input.data {
        Data::Struct(s) => {
            if let Fields::Named(fields) = &s.fields {
                let props = fields.named.iter().map(|f| {
                    let field_opts = FieldOpts::from_field(f).expect("Wrong field options");
                    let field_name = f.ident.as_ref().unwrap();
                    let field_name_str = field_name.to_string();
                    let field_ty = &f.ty;
                    let description = field_opts.description.unwrap_or_default();
                    let schema_call =
                        get_schema_for_type(field_ty, field_opts.enum_lang.as_deref());
                    quote! {
                        let mut schema = #schema_call;
                        if let Some(obj) = schema.as_object_mut() {
                             if !#description.is_empty() {
                                obj.insert("description".to_string(), serde_json::json!(#description));
                            }
                        }
                        properties.insert(#field_name_str.to_string(), schema);
                    }
                });

                let required_fields: Vec<proc_macro2::TokenStream> = fields
                    .named
                    .iter()
                    .filter(|f| {
                        if let Type::Path(type_path) = &f.ty {
                            if let Some(segment) = type_path.path.segments.last() {
                                return segment.ident != "Option";
                            }
                        }
                        true
                    })
                    .map(|f| {
                        let field_name = f.ident.as_ref().unwrap().to_string();
                        quote! { #field_name }
                    })
                    .collect();

                let struct_fields = quote! {
                    fn schema() -> serde_json::Value {
                        let mut properties = serde_json::Map::new();
                        #(#props)*
                        serde_json::json!({
                            "type": "object",
                            "properties": properties,
                            "required": [#(#required_fields),*]
                        })
                    }
                };

                let field_parsers = fields.named.iter().map(|f| {
                    let field_opts = FieldOpts::from_field(f).expect("Wrong field options");
                    let field_name = f.ident.as_ref().unwrap();
                    let field_name_str = field_name.to_string();

                    if field_opts.enum_lang.is_some() {
                        if let Type::Path(type_path) = &f.ty {
                            if let Some(segment) = type_path.path.segments.last() {
                                if segment.ident == "Vec" {
                                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                        if let Some(GenericArgument::Type(inner_ty)) =
                                            args.args.first()
                                        {
                                            return quote! {
                                                #field_name: {
                                                     let value = tool_call_args.get(#field_name_str).ok_or_else(|| ::serde::de::Error::custom(format!("Missing field: {}", #field_name_str)))?;
                                                     let arr = value.as_array().ok_or_else(|| ::serde::de::Error::custom(format!("Field {} is not an array", #field_name_str)))?;
                                                     arr.iter().map(|v| {
                                                        let obj = v.as_object().ok_or_else(|| serde_json::Error::custom("Invalid object in array"))?;
                                                        let type_str = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| serde_json::Error::custom("Missing type in object"))?;
                                                        let content_str = obj.get("content").and_then(|v| v.as_str()).ok_or_else(|| serde_json::Error::custom("Missing content in object"))?;
                                                        <#inner_ty as ::metastable_database::TextPromptCodec>::parse_with_type_and_content(type_str, content_str).map_err(|e| serde_json::Error::custom(e.to_string()))
                                                     }).collect::<Result<Vec<_>, _>>()?
                                                }
                                            };
                                        }
                                    }
                                }
                            }
                        }
                        quote! {
                            #field_name: {
                                let value = tool_call_args.get(#field_name_str).ok_or_else(|| ::serde::de::Error::custom(format!("Missing field: {}", #field_name_str)))?;
                                <#f.ty as ::metastable_database::TextPromptCodec>::parse_any_lang(&value.to_string()).map_err(|e| serde_json::Error::custom(e.to_string()))?
                            }
                        }
                    } else {
                        quote! {
                            #field_name: {
                                let value = tool_call_args.get(#field_name_str).ok_or_else(|| ::serde::de::Error::custom(format!("Missing field: {}", #field_name_str)))?;
                                serde_json::from_value(value.clone())?
                            }
                        }
                    }
                });

                let from_tool_call_impl = quote! {
                    fn try_from_tool_call(
                        tool_call: &async_openai::types::FunctionCall,
                    ) -> Result<Self, serde_json::Error>
                    where
                        Self: Sized,
                    {
                        use serde::de::Error;
                        let tool_call_args: serde_json::Value = serde_json::from_str(&tool_call.arguments)?;
                        let tool_call_args = tool_call_args.as_object().ok_or_else(|| serde_json::Error::custom("Invalid tool call arguments"))?;
                        Ok(Self {
                            #(#field_parsers,)*
                        })
                    }
                };

                (struct_fields, from_tool_call_impl)
            } else {
                (quote! {}, quote! {})
            }
        }
        Data::Enum(e) => {
            let variants = e.variants.iter().map(|v| {
                let ty = &v.fields.iter().next().unwrap().ty;
                get_schema_for_type(ty, None)
            });
            let schema_impl = quote! {
                pub fn schema() -> serde_json::Value {
                     serde_json::json!({
                        "oneOf": [#(#variants),*]
                    })
                }
            };
            (schema_impl, quote! {})
        }
        _ => (quote! {}, quote! {}),
    };

    let expanded = quote! {
        impl ::metastable_runtime::ToolCall for #ident {
            #struct_fields

            fn to_function_object() -> async_openai::types::FunctionObject {
                async_openai::types::FunctionObject {
                    name: #tool_name.to_string(),
                    description: Some(#tool_description.to_string()),
                    parameters: Some(Self::schema()),
                    strict: Some(true),
                }
            }

            #from_tool_call_impl
        }
    };

    TokenStream::from(expanded)
}

fn get_schema_for_type(ty: &Type, enum_lang: Option<&str>) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = &segment.ident;
            if type_name == "String" {
                return quote! {
                    serde_json::json!({ "type": "string" })
                };
            } else if type_name == "i64"
                || type_name == "u64"
                || type_name == "f64"
                || type_name == "isize"
                || type_name == "usize"
            {
                return quote! {
                    serde_json::json!({ "type": "number" })
                };
            } else if type_name == "bool" {
                return quote! {
                    serde_json::json!({ "type": "boolean" })
                };
            } else if type_name == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        let inner_schema = get_schema_for_type(inner_ty, None);
                        if enum_lang.is_some() {
                            let type_schema = get_schema_for_type(inner_ty, enum_lang);
                            return quote! {
                                serde_json::json!({
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "type": #type_schema,
                                            "content": { "type": "string" }
                                        },
                                        "required": ["type", "content"]
                                    }
                                })
                            };
                        }

                        return quote! {
                            serde_json::json!({
                                "type": "array",
                                "items": #inner_schema
                            })
                        };
                    }
                }
            } else if type_name == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return get_schema_for_type(inner_ty, enum_lang);
                    }
                }
            }
        }
    }
    
    if enum_lang.is_some() {
        let lang_opt = match enum_lang {
            Some(lang) => quote! { Some(#lang) },
            None => quote! { None },
        };
        quote! { <#ty>::schema(#lang_opt) }
    } else {
        quote! { <#ty>::schema(None) }
    }
}
