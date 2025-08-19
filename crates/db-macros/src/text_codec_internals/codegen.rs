use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::parse::{TextCodecEnum, TextCodecVariant, VariantKind, FormatKind};

pub fn generate_text_codec_impl(parsed_enum: &TextCodecEnum) -> TokenStream {
    let enum_ident = &parsed_enum.ident;

    let display_impl = generate_display_impl(enum_ident, &parsed_enum.storage_lang, &parsed_enum.format, &parsed_enum.variants);
    let from_str_impl = generate_from_str_impl(enum_ident, &parsed_enum.storage_lang, &parsed_enum.format, &parsed_enum.variants);
    let text_prompt_codec_impl = generate_text_prompt_codec_impl(parsed_enum);
    let default_impl = generate_default_impl(enum_ident, &parsed_enum.variants);
    
    quote! {
        #display_impl
        #from_str_impl
        #text_prompt_codec_impl
        #default_impl
    }
}

fn generate_text_prompt_codec_impl(parsed_enum: &TextCodecEnum) -> TokenStream {
    let enum_ident = &parsed_enum.ident;
    let to_lang_impl = generate_to_lang_impl(enum_ident, &parsed_enum.storage_lang, &parsed_enum.format, &parsed_enum.variants);
    let to_lang_parts_impl = generate_to_lang_parts_impl(enum_ident, &parsed_enum.storage_lang, &parsed_enum.format, &parsed_enum.variants);
    let parse_any_lang_impl = generate_parse_any_lang_impl(enum_ident, &parsed_enum.format, &parsed_enum.variants);
    let parse_with_type_and_content_impl = generate_parse_with_type_and_content_impl(enum_ident, &parsed_enum.variants);
    let schema_impl = generate_schema_impl(parsed_enum);

    quote! {
        impl ::metastable_database::TextPromptCodec for #enum_ident {
            fn to_lang(&self, lang: &str) -> String {
                #to_lang_impl
            }
            
            fn to_lang_parts(&self, lang: &str) -> (String, String) {
                #to_lang_parts_impl
            }

            fn parse_any_lang(s: &str) -> anyhow::Result<Self> {
                #parse_any_lang_impl
            }

            fn parse_with_type_and_content(type_str: &str, content_str: &str) -> anyhow::Result<Self> {
                #parse_with_type_and_content_impl
            }
            
            fn schema(lang: Option<&str>) -> serde_json::Value {
                #schema_impl
            }
        }
    }
}

fn generate_to_lang_parts_impl(_enum_ident: &Ident, _storage_lang: &str, _format: &FormatKind, variants: &[TextCodecVariant]) -> TokenStream {
    let arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let default_prefix = v.ident.to_string();

        let prefix_map: Vec<_> = v.prefixes.iter().map(|(lang, prefix)| {
            quote! { #lang => #prefix, }
        }).collect();

        let prefix_logic = quote! {
            let prefix = match lang {
                #(#prefix_map)*
                _ => #default_prefix,
            };
        };

        match v.kind {
            VariantKind::Unit => quote! { Self::#variant_ident => { #prefix_logic; (prefix.to_string(), "".to_string()) } },
            VariantKind::String => {
                quote! { Self::#variant_ident(inner) => { #prefix_logic; (prefix.to_string(), inner.clone()) } }
            },
            VariantKind::VecString => {
                 quote! { Self::#variant_ident(vec) => { #prefix_logic; (prefix.to_string(), vec.join(",")) } }
            },
            _ => quote! {},
        }
    });
    
    quote! {
        match self {
            #(#arms,)*
        }
    }
}

fn generate_display_impl(enum_ident: &Ident, storage_lang: &str, format: &FormatKind, variants: &[TextCodecVariant]) -> TokenStream {
    let arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let prefix = v.prefixes.get(storage_lang).cloned().unwrap_or_else(|| v.ident.to_string());
        
        match v.kind {
            VariantKind::Unit => quote! { Self::#variant_ident => write!(f, "{}", #prefix) },
            VariantKind::String => {
                match format {
                    FormatKind::Paren => quote! { Self::#variant_ident(inner) => write!(f, "{}({})", #prefix, inner) },
                    FormatKind::Colon(c) => quote! { Self::#variant_ident(inner) => write!(f, "{}{} {}", #prefix, #c, inner) },
                }
            },
            VariantKind::VecString => {
                let open = "[";
                let sep = ",";
                let close = "]";
                match format {
                    FormatKind::Paren => quote! { Self::#variant_ident(vec) => write!(f, "{}({}{}{})", #prefix, #open, vec.join(#sep), #close) },
                    FormatKind::Colon(c) => quote! { Self::#variant_ident(vec) => write!(f, "{}{} {}{}{}", #prefix, #c, #open, vec.join(#sep), #close) },
                }
            },
            VariantKind::Unsupported => quote! {},
        }
    });

    quote! {
        impl ::std::fmt::Display for #enum_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}

fn generate_from_str_impl(enum_ident: &Ident, storage_lang: &str, format: &FormatKind, variants: &[TextCodecVariant]) -> TokenStream {
    let catch_all_variant = variants.iter().find(|v| v.is_catch_all);

    let parse_arms = variants.iter().filter(|v|!v.is_catch_all).map(|v| {
        let variant_ident = &v.ident;
        let prefix = v.prefixes.get(storage_lang).cloned().unwrap_or_else(|| v.ident.to_string());

        match v.kind {
            VariantKind::Unit => quote! { if s == #prefix { return Ok(Self::#variant_ident); } },
            VariantKind::String => {
                match format {
                    FormatKind::Paren => quote! {
                        if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                            return Ok(Self::#variant_ident(inner.to_string()));
                        }
                    },
                    FormatKind::Colon(c) => quote! {
                        if let Some(inner) = s.strip_prefix(&format!("{}{}", #prefix, #c)) {
                            return Ok(Self::#variant_ident(inner.trim().to_string()));
                        }
                    },
                }
            },
            VariantKind::VecString => {
                 let open = "[";
                let sep = ",";
                let close = "]";
                match format {
                    FormatKind::Paren => quote! {
                        if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                            if let Some(inner) = inner.strip_prefix(#open).and_then(|t| t.strip_suffix(#close)) {
                                let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep).map(|s| s.trim().to_string()).collect() };
                                return Ok(Self::#variant_ident(vec));
                            }
                        }
                    },
                    FormatKind::Colon(c) => quote! {
                        if let Some(inner) = s.strip_prefix(&format!("{}{}", #prefix, #c)) {
                            if let Some(inner) = inner.strip_prefix(#open).and_then(|t| t.strip_suffix(#close)) {
                                let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep).map(|s| s.trim().to_string()).collect() };
                                return Ok(Self::#variant_ident(vec));
                            }
                        }
                    },
                }
            },
            VariantKind::Unsupported => quote!{},
        }
    });

    let catch_all_logic = if let Some(variant) = catch_all_variant {
        let variant_ident = &variant.ident;
        match variant.kind {
            VariantKind::String => quote! { return Ok(Self::#variant_ident(s.to_string())); },
            _ => quote! { anyhow::bail!("Catch-all variant is not of type String"); }
        }
    } else {
        quote! { anyhow::bail!("Failed to parse string into {}", stringify!(#enum_ident)); }
    };
    
    quote! {
        impl ::std::str::FromStr for #enum_ident {
            type Err = anyhow::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let s = s.trim();
                #(#parse_arms)*
                #catch_all_logic
            }
        }
    }
}

fn generate_to_lang_impl(_enum_ident: &Ident, _storage_lang: &str, format: &FormatKind, variants: &[TextCodecVariant]) -> TokenStream {
    let arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let default_prefix = v.ident.to_string();

        let prefix_map: Vec<_> = v.prefixes.iter().map(|(lang, prefix)| {
            quote! { #lang => #prefix, }
        }).collect();

        let prefix_logic = quote! {
            let prefix = match lang {
                #(#prefix_map)*
                _ => #default_prefix,
            };
        };

        match v.kind {
            VariantKind::Unit => quote! { Self::#variant_ident => { #prefix_logic; prefix.to_string() } },
            VariantKind::String => {
                match format {
                    FormatKind::Paren => quote! { Self::#variant_ident(inner) => { #prefix_logic; format!("{}({})", prefix, inner) } },
                    FormatKind::Colon(c) => quote! { Self::#variant_ident(inner) => { #prefix_logic; format!("{}{} {}", prefix, #c, inner) } },
                }
            },
            VariantKind::VecString => {
                let open = "[";
                let sep = ",";
                let close = "]";
                match format {
                    FormatKind::Paren => quote! { Self::#variant_ident(vec) => { #prefix_logic; format!("{}({}{}{})", prefix, #open, vec.join(#sep), #close) } },
                    FormatKind::Colon(c) => quote! { Self::#variant_ident(vec) => { #prefix_logic; format!("{}{} {}{}{}", prefix, #c, #open, vec.join(#sep), #close) } },
                }
            },
            _ => quote! {},
        }
    });
    
    quote! {
        match self {
            #(#arms,)*
        }
    }
}


fn generate_parse_any_lang_impl(enum_ident: &Ident, format: &FormatKind, variants: &[TextCodecVariant]) -> TokenStream {
    let mut all_prefixes = std::collections::HashMap::<String, &Ident>::new();
    for v in variants.iter() {
        if !v.is_catch_all {
            for prefix in v.prefixes.values() {
                all_prefixes.insert(prefix.clone(), &v.ident);
            }
        }
    }
    
    let parse_arms = variants.iter().flat_map(|v| {
        if v.is_catch_all {
            return vec![];
        }
        v.prefixes.values().map(|prefix| {
            let variant_ident = &v.ident;
            match v.kind {
                VariantKind::Unit => quote! { if s == #prefix { return Ok(Self::#variant_ident); } },
                VariantKind::String => {
                    match format {
                        FormatKind::Paren => quote! {
                            if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                                return Ok(Self::#variant_ident(inner.to_string()));
                            }
                        },
                        FormatKind::Colon(c) => quote! {
                            if let Some(inner) = s.strip_prefix(&format!("{}{}", #prefix, #c)) {
                                return Ok(Self::#variant_ident(inner.trim().to_string()));
                            }
                        },
                    }
                },
                VariantKind::VecString => {
                    let open = "[";
                    let sep = ",";
                    let close = "]";
                    match format {
                        FormatKind::Paren => quote! {
                            if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                                if let Some(inner) = inner.strip_prefix(#open).and_then(|t| t.strip_suffix(#close)) {
                                    let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep).map(|s| s.trim().to_string()).collect() };
                                    return Ok(Self::#variant_ident(vec));
                                }
                            }
                        },
                        FormatKind::Colon(c) => quote! {
                            if let Some(inner) = s.strip_prefix(&format!("{}{}", #prefix, #c)) {
                                if let Some(inner) = inner.strip_prefix(#open).and_then(|t| t.strip_suffix(#close)) {
                                    let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep).map(|s| s.trim().to_string()).collect() };
                                    return Ok(Self::#variant_ident(vec));
                                }
                            }
                        },
                    }
                },
                VariantKind::Unsupported => quote! {},
            }
        }).collect::<Vec<_>>()
    });

    let catch_all_variant = variants.iter().find(|v| v.is_catch_all);
    let catch_all_logic = if let Some(variant) = catch_all_variant {
        let variant_ident = &variant.ident;
        match variant.kind {
            VariantKind::String => quote! { return Ok(Self::#variant_ident(s.to_string())); },
            _ => quote! { anyhow::bail!("Catch-all variant is not of type String"); }
        }
    } else {
        quote! { anyhow::bail!("Failed to parse string into {}: {}", stringify!(#enum_ident), s); }
    };

    quote! {
        let s = s.trim();
        #(#parse_arms)*
        #catch_all_logic
    }
}

fn generate_parse_with_type_and_content_impl(enum_ident: &Ident, variants: &[TextCodecVariant]) -> TokenStream {
    let mut type_map = std::collections::HashMap::<String, &TextCodecVariant>::new();
    for v in variants {
        for prefix in v.prefixes.values() {
            type_map.insert(prefix.clone(), v);
        }
    }
    
    let arms = type_map.iter().map(|(prefix, variant)| {
        let variant_ident = &variant.ident;
        match variant.kind {
            VariantKind::Unit => quote! {
                if type_str == #prefix {
                    return Ok(Self::#variant_ident);
                }
            },
            VariantKind::String => quote! {
                if type_str == #prefix {
                    return Ok(Self::#variant_ident(content_str.to_string()));
                }
            },
            VariantKind::VecString => quote! {
                if type_str == #prefix {
                    let vec = if content_str.trim().is_empty() { Vec::new() } else { content_str.split(',').map(|s| s.trim().to_string()).collect() };
                    return Ok(Self::#variant_ident(vec));
                }
            },
            VariantKind::Unsupported => quote!{},
        }
    });

    let catch_all_variant = variants.iter().find(|v| v.is_catch_all);
    let catch_all_logic = if let Some(variant) = catch_all_variant {
        let variant_ident = &variant.ident;
        quote! { return Ok(Self::#variant_ident(format!("{}: {}", type_str, content_str))); }
    } else {
        quote! { anyhow::bail!("Invalid type or content for {}", stringify!(#enum_ident)); }
    };
    
    quote! {
        #(#arms)*
        #catch_all_logic
    }
}

fn generate_schema_impl(parsed_enum: &TextCodecEnum) -> TokenStream {
    let mut all_langs = parsed_enum.variants.iter()
        .flat_map(|v| v.prefixes.keys())
        .collect::<std::collections::HashSet<_>>();
    all_langs.insert(&parsed_enum.storage_lang);

    let arms = all_langs.iter().map(|lang| {
        let variant_names: Vec<String> = parsed_enum.variants.iter()
            .map(|v| {
                v.prefixes.get(*lang).cloned().unwrap_or_else(|| v.ident.to_string())
            })
            .collect();
        quote! {
            Some(#lang) => serde_json::json!({
                "type": "string",
                "enum": [#(#variant_names),*]
            }),
        }
    });
    
    let default_variant_names: Vec<String> = parsed_enum.variants.iter()
        .map(|v| v.ident.to_string())
        .collect();

    quote! {
        match lang {
            #(#arms)*
            _ => serde_json::json!({
                "type": "string",
                "enum": [#(#default_variant_names),*]
            }),
        }
    }
}

fn generate_default_impl(enum_ident: &Ident, variants: &[TextCodecVariant]) -> TokenStream {
    if let Some(catch_all) = variants.iter().find(|v| v.is_catch_all) {
        let variant_ident = &catch_all.ident;
        match catch_all.kind {
            VariantKind::String => {
                quote! {
                    impl Default for #enum_ident {
                        fn default() -> Self {
                            Self::#variant_ident(String::new())
                        }
                    }
                }
            },
            _ => quote!{},
        }
    } else {
        quote! {}
    }
}
