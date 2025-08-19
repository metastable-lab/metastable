use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericArgument, PathArguments, Type};

use super::parse::{LlmTool, LlmToolField};

pub fn generate_llm_tool_impl(parsed_tool: &LlmTool) -> TokenStream {
    let tool_ident = &parsed_tool.ident;
    let tool_name = parsed_tool.name.clone().unwrap_or_else(|| tool_ident.to_string());
    let tool_description = parsed_tool.description.clone().unwrap_or_default();

    let fields = parsed_tool.data.as_ref().take_struct().expect("LlmTool can only be derived for structs").fields;

    let schema_impl = generate_schema_impl(&fields);
    let from_tool_call_impl = generate_from_tool_call_impl(tool_ident, &tool_name, &fields);
    let into_tool_call_impl = generate_into_tool_call_impl(&tool_name, &fields);
    
    quote! {
        impl ::metastable_runtime::ToolCall for #tool_ident {
            fn schema() -> serde_json::Value {
                #schema_impl
            }

            fn to_function_object() -> async_openai::types::FunctionObject {
                async_openai::types::FunctionObject {
                    name: #tool_name.to_string(),
                    description: Some(#tool_description.to_string()),
                    parameters: Some(Self::schema()),
                    strict: Some(true),
                }
            }
            
            #from_tool_call_impl

            #into_tool_call_impl
        }
    }
}

fn generate_schema_impl(fields: &[&LlmToolField]) -> TokenStream {
    let props = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let description = f.description.clone().unwrap_or_default();
        let schema_call = get_schema_for_type(&f.ty, f.enum_lang.as_deref(), f.is_enum);

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

    let required_fields: Vec<proc_macro2::TokenStream> = fields.iter()
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
    
    quote! {
        let mut properties = serde_json::Map::new();
        #(#props)*
        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": [#(#required_fields),*]
        })
    }
}

fn generate_from_tool_call_impl(_tool_ident: &syn::Ident, tool_name: &str, fields: &[&LlmToolField]) -> TokenStream {
    let field_parsers = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();

        let (is_option, base_ty) = unwrap_option(&f.ty);
        let (is_vec, inner_ty) = unwrap_vec(&base_ty);
        
        let parser = if f.enum_lang.is_some() {
            // TextPromptCodec enum
            if is_vec {
                let inner_ty = inner_ty.as_ref().unwrap_or(&base_ty);
                quote! {
                    let arr = value.as_array().ok_or_else(|| Error::custom(format!("Field {} is not an array", #field_name_str)))?;
                    arr.iter().map(|v| {
                        let obj = v.as_object().ok_or_else(|| Error::custom("Invalid object in array"))?;
                        let type_str = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| Error::custom("Missing type in object"))?;
                        let content_str = obj.get("content").and_then(|v| v.as_str()).ok_or_else(|| Error::custom("Missing content in object"))?;
                        <#inner_ty as ::metastable_database::TextPromptCodec>::parse_with_type_and_content(type_str, content_str)
                            .map_err(|e| Error::custom(e.to_string()))
                    }).collect::<Result<Vec<_>, _>>()?
                }
            } else {
                 quote! {
                    if let Some(s) = value.as_str() {
                        <#base_ty as ::metastable_database::TextPromptCodec>::parse_any_lang(s)
                            .map_err(|e| Error::custom(e.to_string()))?
                    } else if let Some(obj) = value.as_object() {
                        let type_str = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| Error::custom("Missing type in object"))?;
                        let content_str = obj.get("content").and_then(|v| v.as_str()).ok_or_else(|| Error::custom("Missing content in object"))?;
                        <#base_ty as ::metastable_database::TextPromptCodec>::parse_with_type_and_content(type_str, content_str)
                            .map_err(|e| Error::custom(e.to_string()))?
                    } else {
                        return Err(Error::custom(format!("Invalid value for field {}", #field_name_str)));
                    }
                }
            }
        } else if f.is_enum {
            // Standard enum
            if is_vec {
                let inner_ty = inner_ty.as_ref().unwrap_or(&base_ty);
                quote! {
                    let arr = value.as_array().ok_or_else(|| Error::custom(format!("Field {} is not an array", #field_name_str)))?;
                    arr.iter().map(|v| {
                        let s = v.as_str().ok_or_else(|| Error::custom("Invalid string in array"))?;
                        s.parse::<#inner_ty>().map_err(|e| Error::custom(e.to_string()))
                    }).collect::<Result<Vec<_>, _>>()?
                }
            } else {
                quote! {
                    value.as_str().ok_or_else(|| Error::custom(format!("Invalid string for field {}", #field_name_str)))?.parse().map_err(|e| Error::custom(format!("Failed to parse enum: {}", e)))?
                }
            }
        } else {
            // Regular type
            quote! { serde_json::from_value(value.clone())? }
        };

        if is_option {
            quote! {
                #field_name: {
                    use serde::de::Error;
                    match tool_call_args.get(#field_name_str) {
                        None => None,
                        Some(value) if value.is_null() => None,
                        Some(value) => Some({ #parser })
                    }
                }
            }
        } else {
            quote! {
                #field_name: {
                    use serde::de::Error;
                    let value = tool_call_args.get(#field_name_str).ok_or_else(|| Error::custom(format!("Missing field: {}", #field_name_str)))?;
                    { #parser }
                }
            }
        }
    });

    quote! {
        fn try_from_tool_call(tool_call: &async_openai::types::FunctionCall) -> Result<Self, serde_json::Error> {
            use serde::de::Error;
            if tool_call.name != #tool_name {
                return Err(Error::custom(format!("Unexpected tool name: got '{}', expected '{}'", tool_call.name, #tool_name)));
            }
            let tool_call_args: serde_json::Value = serde_json::from_str(&tool_call.arguments)?;
            let tool_call_args = tool_call_args.as_object().ok_or_else(|| serde_json::Error::custom("Invalid tool call arguments"))?;
            Ok(Self {
                #(#field_parsers,)*
            })
        }
    }
}

fn generate_into_tool_call_impl(tool_name: &str, fields: &[&LlmToolField]) -> TokenStream {
    let field_serializers = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        
        let (is_option, base_ty) = unwrap_option(&f.ty);
        let (is_vec, _) = unwrap_vec(&base_ty);

        let serializer = if f.is_enum {
            if is_vec {
                quote! {
                    let mut items = Vec::new();
                    for item in value {
                        items.push(serde_json::to_value(item.to_string())?);
                    }
                    args.insert(#field_name_str.to_string(), serde_json::Value::Array(items));
                }
            } else {
                quote! {
                    args.insert(#field_name_str.to_string(), serde_json::to_value(value.to_string())?);
                }
            }
        } else if let Some(enum_lang) = f.enum_lang.as_deref() {
            if is_vec {
                quote! {
                    let mut items = Vec::new();
                    for item in value {
                        let (ty, content) = ::metastable_database::TextPromptCodec::to_lang_parts(item, #enum_lang);
                        items.push(serde_json::json!({
                            "type": ty,
                            "content": content
                        }));
                    }
                    args.insert(#field_name_str.to_string(), serde_json::Value::Array(items));
                }
            } else {
                quote! {
                    args.insert(#field_name_str.to_string(), serde_json::to_value(::metastable_database::TextPromptCodec::to_lang(value, #enum_lang))?);
                }
            }
        } else {
            quote! {
                args.insert(#field_name_str.to_string(), serde_json::to_value(value)?);
            }
        };

        if is_option {
            quote! {
                if let Some(value) = &self.#field_name {
                    #serializer
                }
            }
        } else {
            quote! {
                let value = &self.#field_name;
                #serializer
            }
        }
    });

    quote! {
        fn into_tool_call(&self) -> Result<async_openai::types::FunctionCall, serde_json::Error> {
            let mut args = serde_json::Map::new();
            #(#field_serializers)*
            
            let arguments = if args.is_empty() {
                "{}".to_string()
            } else {
                serde_json::to_string(&serde_json::Value::Object(args)).unwrap()
            };

            Ok(async_openai::types::FunctionCall {
                name: #tool_name.to_string(),
                arguments,
            })
        }
    }
}

fn get_schema_for_type(ty: &Type, enum_lang: Option<&str>, is_enum: bool) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = &segment.ident;
            if type_name == "String" {
                return quote! { serde_json::json!({ "type": "string" }) };
            } else if type_name == "i64" || type_name == "u64" || type_name == "f64" || type_name == "isize" || type_name == "usize" {
                return quote! { serde_json::json!({ "type": "number" }) };
            } else if type_name == "bool" {
                return quote! { serde_json::json!({ "type": "boolean" }) };
            } else if type_name == "Uuid" {
                return quote! { serde_json::json!({ "type": "string", "format": "uuid" }) };
            } else if type_name == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        if enum_lang.is_some() {
                             let lang_opt = match enum_lang {
                                Some(lang) => quote! { Some(#lang) },
                                None => quote! { None },
                            };
                            let type_schema = quote! { <#inner_ty as ::metastable_database::TextPromptCodec>::schema(#lang_opt) };
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

                        let inner_schema = get_schema_for_type(inner_ty, None, is_enum);
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
                        return get_schema_for_type(inner_ty, enum_lang, is_enum);
                    }
                }
            }
        }
    }

    if is_enum || enum_lang.is_some() {
        let lang_opt = match enum_lang {
            Some(lang) => quote! { Some(#lang) },
            None => quote! { None },
        };
        quote! { <#ty as ::metastable_database::TextPromptCodec>::schema(#lang_opt) }
    } else {
        quote! { <#ty as ::metastable_runtime::ToolCall>::schema() }
    }
}

fn unwrap_option(ty: &Type) -> (bool, Type) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return (true, inner_ty.clone());
                    }
                }
            }
        }
    }
    (false, ty.clone())
}

fn unwrap_vec(ty: &Type) -> (bool, Option<Type>) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return (true, Some(inner_ty.clone()));
                    }
                }
            }
        }
    }
    (false, None)
}
