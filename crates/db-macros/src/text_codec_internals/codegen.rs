use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::parse::{TextEnumCodec, TextEnumVariant, VariantKind};

pub fn generate_text_enum_impl(parsed_enum: &TextEnumCodec) -> TokenStream {
    let enum_ident = &parsed_enum.ident;

    let text_enum_codec_impl = generate_text_enum_codec_trait_impl(parsed_enum);
    let default_impl = generate_default_impl(enum_ident, &parsed_enum.variants);
    let display_impl = generate_display_impl(enum_ident);
    let from_str_impl = generate_from_str_impl(enum_ident);

    quote! {
        #text_enum_codec_impl
        #display_impl
        #from_str_impl
        #default_impl
    }
}

fn generate_text_enum_codec_trait_impl(parsed_enum: &TextEnumCodec) -> TokenStream {
    let enum_ident = &parsed_enum.ident;
    let type_lang = &parsed_enum.type_lang;
    let schema_lang = &parsed_enum.schema_lang;

    let to_text_impl = generate_to_text_impl(type_lang, &parsed_enum.variants);
    let from_text_impl = generate_from_text_impl(enum_ident, type_lang, &parsed_enum.variants);
    let schema_impl = generate_schema_impl(parsed_enum);

    quote! {
        impl ::metastable_database::TextEnumCodec for #enum_ident {
            fn to_text(&self, lang: &str) -> String {
                #to_text_impl
            }

            fn from_text(s: &str) -> anyhow::Result<Self> {
                #from_text_impl
            }

            fn schema(lang: Option<&str>) -> serde_json::Value {
                #schema_impl
            }

            fn type_lang() -> &'static str {
                #type_lang
            }

            fn schema_lang() -> &'static str {
                #schema_lang
            }
        }
    }
}

fn generate_to_text_impl(type_lang: &str, variants: &[TextEnumVariant]) -> TokenStream {
    let arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let default_type = v.ident.to_string();

        // Get the type name for this language, fallback to default
        let type_name = v.prefixes.get(type_lang).cloned().unwrap_or(default_type);

        match v.kind {
            VariantKind::Unit => {
                // Unit variants return pure string
                quote! { Self::#variant_ident => #type_name.to_string() }
            },
            VariantKind::String => {
                if v.is_catch_all && !v.catch_all_include_prefix {
                    // Catch-all variants return pure string (even though they have content)
                    quote! { Self::#variant_ident(content) => content.clone() }
                } else {
                    // Content variants return structured JSON
                    quote! {
                        Self::#variant_ident(content) => {
                            serde_json::json!({
                                "content": content,
                                "type": #type_name
                            }).to_string()
                        }
                    }
                }
            },
            VariantKind::VecString => {
                quote! {
                    Self::#variant_ident(items) => {
                        serde_json::json!({
                            "content": items.join(","),
                            "type": #type_name
                        }).to_string()
                    }
                }
            },
            VariantKind::Uuid => {
                quote! {
                    Self::#variant_ident(id) => {
                        serde_json::json!({
                            "content": id.to_string(),
                            "type": #type_name
                        }).to_string()
                    }
                }
            },
            VariantKind::Unsupported => quote! { Self::#variant_ident => unreachable!() },
        }
    });

    quote! {
        match self {
            #(#arms,)*
        }
    }
}

fn generate_from_text_impl(enum_ident: &Ident, type_lang: &str, variants: &[TextEnumVariant]) -> TokenStream {
    let mut type_to_variant = std::collections::HashMap::new();

    // Build mapping from type names to variants
    for variant in variants {
        if !variant.is_catch_all || variant.catch_all_include_prefix {
            // Add default type name
            type_to_variant.insert(variant.ident.to_string(), variant);

            // Add language-specific type names
            for (lang, type_name) in &variant.prefixes {
                if lang == type_lang {
                    type_to_variant.insert(type_name.clone(), variant);
                }
            }
        }
    }

    let parse_arms = type_to_variant.iter().map(|(type_name, variant)| {
        let variant_ident = &variant.ident;
        match variant.kind {
            VariantKind::Unit => {
                quote! {
                    if s.trim() == #type_name {
                        return Ok(Self::#variant_ident);
                    }
                }
            },
            VariantKind::String => {
                quote! {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
                        if let (Some(content), Some(typ)) = (
                            json.get("content").and_then(|v| v.as_str()),
                            json.get("type").and_then(|v| v.as_str())
                        ) {
                            if typ == #type_name {
                                return Ok(Self::#variant_ident(content.to_string()));
                            }
                        }
                    }
                }
            },
            VariantKind::VecString => {
                quote! {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
                        if let (Some(content), Some(typ)) = (
                            json.get("content").and_then(|v| v.as_str()),
                            json.get("type").and_then(|v| v.as_str())
                        ) {
                            if typ == #type_name {
                                let items = if content.trim().is_empty() {
                                    Vec::new()
                                } else {
                                    content.split(',').map(|s| s.trim().to_string()).collect()
                                };
                                return Ok(Self::#variant_ident(items));
                            }
                        }
                    }
                }
            },
            VariantKind::Uuid => {
                quote! {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
                        if let (Some(content), Some(typ)) = (
                            json.get("content").and_then(|v| v.as_str()),
                            json.get("type").and_then(|v| v.as_str())
                        ) {
                            if typ == #type_name {
                                if let Ok(uuid) = content.parse() {
                                    return Ok(Self::#variant_ident(uuid));
                                }
                            }
                        }
                    }
                }
            },
            VariantKind::Unsupported => quote! {},
        }
    });

    // Handle catch-all variant - try pure string parsing first for catch-all
    let catch_all_logic = if let Some(catch_all) = variants.iter().find(|v| v.is_catch_all) {
        let variant_ident = &catch_all.ident;
        match catch_all.kind {
            VariantKind::String => quote! {
                // If no other match, treat as catch-all with the original string
                return Ok(Self::#variant_ident(s.to_string()));
            },
            _ => quote! {
                anyhow::bail!("Catch-all variant must be of type String");
            },
        }
    } else {
        quote! {
            anyhow::bail!("Failed to parse '{}' into {}", s, stringify!(#enum_ident));
        }
    };

    quote! {
        let s = s.trim();
        #(#parse_arms)*
        #catch_all_logic
    }
}

fn generate_schema_impl(parsed_enum: &TextEnumCodec) -> TokenStream {
    // Determine schema structure at compile time
    let has_structured_variants = parsed_enum.variants.iter()
        .any(|v| matches!(v.kind, VariantKind::String | VariantKind::VecString | VariantKind::Uuid) && !v.is_catch_all);
    let has_unit_variants = parsed_enum.variants.iter()
        .any(|v| v.kind == VariantKind::Unit);

    // Pre-compute unique, sorted variant names for all available languages at compile time
    let mut lang_to_variants = std::collections::BTreeMap::<String, Vec<String>>::new();
    for v in &parsed_enum.variants {        
        // Add variant's default name to the default schema language
        let default_lang_variants = lang_to_variants.entry(parsed_enum.schema_lang.clone()).or_default();
        if !default_lang_variants.contains(&v.ident.to_string()) {
            default_lang_variants.push(v.ident.to_string());
        }

        // Add prefixed names for each language
        for (lang, prefix) in &v.prefixes {
            let lang_variants = lang_to_variants.entry(lang.clone()).or_default();
            if !lang_variants.contains(prefix) {
                lang_variants.push(prefix.clone());
            }
        }
    }

    // Generate a match arm for each language
    let lang_arms = lang_to_variants.iter().map(|(lang, variants)| {
        let schema = generate_enum_schema(has_structured_variants, has_unit_variants, variants);
        quote! { #lang => #schema }
    });

    // Generate the schema for the default language to use in the fallback arm
    let default_lang = &parsed_enum.schema_lang;
    let default_variants = lang_to_variants.get(default_lang).cloned().unwrap_or_default();
    let default_schema = generate_enum_schema(has_structured_variants, has_unit_variants, &default_variants);

    // Generate the final function body
    quote! {
        let lang = lang.unwrap_or(#default_lang);
        match lang {
            #(#lang_arms),*,
            _ => #default_schema,
        }
    }
}

fn generate_enum_schema(has_structured_variants: bool, has_unit_variants: bool, variants: &[String]) -> TokenStream {
    if has_structured_variants && has_unit_variants {
        quote! {
            serde_json::json!({"type": "string", "enum": [#(#variants),*]})
        }
    } else if has_structured_variants {
        quote! {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string"},
                    "type": {"type": "string", "enum": [#(#variants),*]}
                },
                "required": ["type", "content"]
            })
        }
    } else {
        quote! {
            serde_json::json!({
                "type": "string",
                "enum": [#(#variants),*]
            })
        }
    }
}

fn generate_display_impl(enum_ident: &Ident) -> TokenStream {
    quote! {
        impl ::std::fmt::Display for #enum_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let text = self.to_text(Self::type_lang());
                write!(f, "{}", text)
            }
        }
    }
}

fn generate_from_str_impl(enum_ident: &Ident) -> TokenStream {
    quote! {
        impl ::std::str::FromStr for #enum_ident {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_text(s)
            }
        }
    }
}

fn generate_default_impl(enum_ident: &Ident, variants: &[TextEnumVariant]) -> TokenStream {
    // Find the first catch-all variant for Default implementation
    if let Some(catch_all) = variants.iter().find(|v| v.is_catch_all) {
        let variant_ident = &catch_all.ident;
        match catch_all.kind {
            VariantKind::String => {
                return quote! {
                    impl ::std::default::Default for #enum_ident {
                        fn default() -> Self {
                            Self::#variant_ident(String::new())
                        }
                    }
                };
            },
            _ => {
                // Catch-all must be String type, this should be caught during parsing
                return quote! {};
            }
        }
    }
    
    // If no suitable default variant found, don't implement Default
    quote! {}
}
