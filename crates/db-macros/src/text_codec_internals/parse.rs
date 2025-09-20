use syn::{DeriveInput, Lit, Meta, NestedMeta, Ident, Data, Fields, Variant};
use quote::quote;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TextEnumCodec {
    pub ident: Ident,
    pub type_lang: String,
    pub schema_lang: String,
    pub variants: Vec<TextEnumVariant>,
}

#[derive(Debug)]
pub struct TextEnumVariant {
    pub ident: Ident,
    pub kind: VariantKind,
    pub prefixes: HashMap<String, String>,
    pub is_catch_all: bool,
    pub catch_all_include_prefix: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VariantKind {
    Unit,           // Like: Scenario
    String,         // Like: Scenario(String)
    VecString,      // Like: Items(Vec<String>)
    Uuid,          // Like: Entity(Uuid)
    Unsupported,   // Other types
}

pub fn parse_text_enum(input: &DeriveInput) -> Result<TextEnumCodec, syn::Error> {
    let mut type_lang = "en".to_string();
    let mut schema_lang = "en".to_string();

    for attr in &input.attrs {
        if attr.path.is_ident("text_enum") {
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                for nested in list.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                        if nv.path.is_ident("type_lang") {
                            if let Lit::Str(s) = &nv.lit {
                                type_lang = s.value();
                            }
                        } else if nv.path.is_ident("schema_lang") {
                            if let Lit::Str(s) = &nv.lit {
                                schema_lang = s.value();
                            }
                        }
                    }
                }
            }
        }
    }

    let variants = if let Data::Enum(data_enum) = &input.data {
        data_enum.variants.iter().map(parse_variant).collect::<Result<Vec<_>,_>>()?
    } else {
        return Err(syn::Error::new_spanned(&input.ident, "TextEnum can only be derived for enums"));
    };

    Ok(TextEnumCodec {
        ident: input.ident.clone(),
        type_lang,
        schema_lang,
        variants,
    })
}

fn parse_variant(variant: &Variant) -> Result<TextEnumVariant, syn::Error> {
    let mut prefixes = HashMap::new();
    let mut is_catch_all = false;
    let mut catch_all_include_prefix = false;

    for attr in &variant.attrs {
        if attr.path.is_ident("prefix") {
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                let (lang, content) = parse_prefix_attribute(&list)?;
                prefixes.insert(lang, content);
            }
        } else if attr.path.is_ident("catch_all") {
            is_catch_all = true;
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                for nested in list.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                        if nv.path.is_ident("include_prefix") {
                            if let Lit::Bool(b) = &nv.lit {
                                catch_all_include_prefix = b.value;
                            }
                        }
                    }
                }
            }
        }
    }

    let kind = match &variant.fields {
        Fields::Unit => VariantKind::Unit,
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed[0].ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str == "String" || ty_str == "std::string::String" {
                VariantKind::String
            } else if ty_str == "Vec<String>" {
                VariantKind::VecString
            } else if ty_str == "Uuid" || ty_str == "sqlx::types::Uuid" {
                VariantKind::Uuid
            } else {
                VariantKind::Unsupported
            }
        }
        _ => VariantKind::Unsupported,
    };

    Ok(TextEnumVariant {
        ident: variant.ident.clone(),
        kind,
        prefixes,
        is_catch_all,
        catch_all_include_prefix,
    })
}

fn parse_prefix_attribute(list: &syn::MetaList) -> Result<(String, String), syn::Error> {
    let mut lang = None;
    let mut content = None;
    for nested in list.nested.iter() {
        if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
            if nv.path.is_ident("lang") {
                if let Lit::Str(s) = &nv.lit {
                    lang = Some(s.value());
                }
            } else if nv.path.is_ident("content")
                && let Lit::Str(s) = &nv.lit {
                content = Some(s.value());
            }
        }
    }
    let lang = lang.ok_or_else(|| syn::Error::new_spanned(list, "Missing `lang` in `prefix` attribute"))?;
    let content = content.ok_or_else(|| syn::Error::new_spanned(list, "Missing `content` in `prefix` attribute"))?;
    Ok((lang, content))
}
