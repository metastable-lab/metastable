use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::parse::{TextEnumCodec, TextEnumVariant, VariantKind};

pub fn generate_text_enum_impl(parsed_enum: &TextEnumCodec) -> TokenStream {
    let enum_ident = &parsed_enum.ident;

    let text_enum_codec_impl = generate_text_enum_codec_trait_impl(parsed_enum);
    let default_impl = generate_default_impl(enum_ident, &parsed_enum.variants);
    let display_impl = generate_display_impl(enum_ident);
    let from_str_impl = generate_from_str_impl(enum_ident);
    let serialize_impl = generate_serialize_impl(enum_ident, &parsed_enum.variants);
    let deserialize_impl = generate_deserialize_impl(enum_ident, &parsed_enum.variants);
    let sqlx_impls = generate_sqlx_impls(enum_ident);

    quote! {
        #text_enum_codec_impl
        #display_impl
        #from_str_impl
        #default_impl
        #serialize_impl
        #deserialize_impl
        #sqlx_impls
    }
}

fn generate_text_enum_codec_trait_impl(parsed_enum: &TextEnumCodec) -> TokenStream {
    let enum_ident = &parsed_enum.ident;
    let type_lang = &parsed_enum.type_lang;
    let schema_lang = &parsed_enum.schema_lang;

    let to_prompt_text_impl = generate_to_prompt_text_impl(type_lang, &parsed_enum.variants);
    let schema_impl = generate_schema_impl(parsed_enum);

    quote! {
        impl ::metastable_database::TextEnumCodec for #enum_ident {
            fn to_prompt_text(&self, lang: &str) -> String {
                #to_prompt_text_impl
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

fn generate_sqlx_impls(enum_ident: &Ident) -> TokenStream {
    quote! {
        impl ::sqlx::Type<::sqlx::Postgres> for #enum_ident {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                ::sqlx::postgres::PgTypeInfo::with_name("JSONB")
            }
        }

        impl<'q> ::sqlx::Encode<'q, ::sqlx::Postgres> for #enum_ident {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> Result<::sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                let json_value = serde_json::to_value(self)?;
                <serde_json::Value as ::sqlx::Encode<::sqlx::Postgres>>::encode_by_ref(&json_value, buf)
            }
        }

        impl<'r> ::sqlx::Decode<'r, ::sqlx::Postgres> for #enum_ident {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
                let json_value = <serde_json::Value as ::sqlx::Decode<::sqlx::Postgres>>::decode(value)?;
                serde_json::from_value(json_value).map_err(Into::into)
            }
        }
    }
}

fn generate_to_prompt_text_impl(type_lang: &str, variants: &[TextEnumVariant]) -> TokenStream {
    let arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let default_type = v.ident.to_string();

        // Get the type name for this language, fallback to default
        let type_name = v.prefixes.get(type_lang).cloned().unwrap_or(default_type);

        match v.kind {
            VariantKind::Unit => {
                quote! { Self::#variant_ident => #type_name.to_string() }
            },
            VariantKind::String => {
                if v.is_catch_all {
                    quote! { Self::#variant_ident(content) => content.clone() }
                } else {
                    quote! { Self::#variant_ident(content) => format!("{}: {}", #type_name, content) }
                }
            },
            VariantKind::VecString => {
                quote! { Self::#variant_ident(items) => format!("{}: {}", #type_name, items.join(",")) }
            },
            VariantKind::Uuid => {
                quote! { Self::#variant_ident(id) => format!("{}: {}", #type_name, id) }
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

fn generate_serialize_impl(enum_ident: &Ident, variants: &[TextEnumVariant]) -> TokenStream {
    let ser_repr_ident = format_ident!("{}SerRepr", enum_ident);
    let ser_struct_ident = format_ident!("{}SerStruct", enum_ident);

    let from_arms = variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let default_type = v.ident.to_string();

        // For serialization, we'll just use the default type name.
        // The multi-language support was primarily for prompt generation.
        let type_name = default_type;

        match v.kind {
            VariantKind::Unit => {
                quote! { Self::#variant_ident => #ser_repr_ident::Unit(#type_name.into()) }
            },
            VariantKind::String => {
                if v.is_catch_all && !v.catch_all_include_prefix {
                    quote! { Self::#variant_ident(content) => #ser_repr_ident::Unit(content.clone()) }
                } else {
                    quote! {
                        Self::#variant_ident(content) => #ser_repr_ident::Content(#ser_struct_ident {
                            typ: #type_name.into(),
                            content: serde_json::json!(content),
                        })
                    }
                }
            },
            VariantKind::VecString => {
                quote! {
                    Self::#variant_ident(items) => #ser_repr_ident::Content(#ser_struct_ident {
                        typ: #type_name.into(),
                        content: serde_json::json!(items.join(",")),
                    })
                }
            },
            VariantKind::Uuid => {
                quote! {
                    Self::#variant_ident(id) => #ser_repr_ident::Content(#ser_struct_ident {
                        typ: #type_name.into(),
                        content: serde_json::json!(id.to_string()),
                    })
                }
            },
            VariantKind::Unsupported => quote! { Self::#variant_ident => unreachable!() },
        }
    });

    quote! {
        impl serde::Serialize for #enum_ident {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                #[derive(serde::Serialize)]
                #[serde(untagged)]
                enum #ser_repr_ident {
                    Unit(String),
                    Content(#ser_struct_ident),
                }

                #[derive(serde::Serialize)]
                struct #ser_struct_ident {
                    #[serde(rename = "type")]
                    typ: String,
                    content: serde_json::Value,
                }

                let repr = match self {
                    #(#from_arms,)*
                };
                repr.serialize(serializer)
            }
        }
    }
}

fn generate_deserialize_impl(enum_ident: &Ident, variants: &[TextEnumVariant]) -> TokenStream {
    let de_repr_ident = format_ident!("{}DeRepr", enum_ident);
    let de_struct_ident = format_ident!("{}DeStruct", enum_ident);

    let unit_arms = variants.iter().filter(|v| v.kind == VariantKind::Unit).map(|v| {
        let variant_ident = &v.ident;
        let mut type_names = vec![v.ident.to_string()];
        type_names.extend(v.prefixes.values().cloned());
        quote! {
            #( #type_names => Ok(Self::#variant_ident), )*
        }
    });

    let content_arms = variants.iter().filter(|v| v.kind != VariantKind::Unit && (!v.is_catch_all || v.catch_all_include_prefix)).map(|v| {
        let variant_ident = &v.ident;
        let mut type_names = vec![v.ident.to_string()];
        type_names.extend(v.prefixes.values().cloned());

        let content_parsing = match v.kind {
            VariantKind::String => quote! { content.as_str().map(|s| Self::#variant_ident(s.to_string())) },
            VariantKind::VecString => quote! {
                content.as_str().map(|s| {
                    let items = if s.trim().is_empty() { vec![] } else { s.split(',').map(|i| i.trim().to_string()).collect() };
                    Self::#variant_ident(items)
                })
            },
            VariantKind::Uuid => quote! {
                content.as_str().and_then(|s| s.parse().ok()).map(Self::#variant_ident)
            },
            _ => quote! { None },
        };

        quote! {
            #( #type_names => #content_parsing.ok_or_else(|| anyhow::anyhow!("Failed to parse content for type '{}'", typ)), )*
        }
    });

    let catch_all_arm = if let Some(catch_all) = variants.iter().find(|v| v.is_catch_all) {
        let variant_ident = &catch_all.ident;
        quote! {
            _ => Ok(Self::#variant_ident(s.to_string())),
        }
    } else {
        quote! {
            _ => Err(anyhow::anyhow!("Unknown unit variant string: {}", s)),
        }
    };

    quote! {
        impl<'de> serde::Deserialize<'de> for #enum_ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                #[derive(serde::Deserialize)]
                #[serde(untagged)]
                enum #de_repr_ident {
                    Unit(String),
                    Content(#de_struct_ident),
                }

                #[derive(serde::Deserialize)]
                struct #de_struct_ident {
                    #[serde(rename = "type")]
                    typ: String,
                    content: serde_json::Value,
                }

                let repr = #de_repr_ident::deserialize(deserializer)?;
                let result: anyhow::Result<Self> = match repr {
                    #de_repr_ident::Unit(s) => {
                        match s.as_str() {
                            #(#unit_arms)*
                            #catch_all_arm
                        }
                    },
                    #de_repr_ident::Content(#de_struct_ident { typ, content }) => {
                        match typ.as_str() {
                            #(#content_arms)*
                            _ => Err(anyhow::anyhow!("Unknown content variant type: {}", typ)),
                        }
                    },
                };

                result.map_err(serde::de::Error::custom)
            }
        }
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
                let s = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
                write!(f, "{}", s)
            }
        }
    }
}

fn generate_from_str_impl(enum_ident: &Ident) -> TokenStream {
    quote! {
        impl ::std::str::FromStr for #enum_ident {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let json_value = serde_json::from_str(s)
                    .unwrap_or_else(|_| serde_json::Value::String(s.to_string()));
                serde_json::from_value(json_value).map_err(anyhow::Error::from)
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
