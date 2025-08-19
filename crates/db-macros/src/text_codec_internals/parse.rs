use syn::{DeriveInput, Lit, Meta, NestedMeta, Ident, Data, Fields, Variant};
use quote::quote;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatKind {
    Paren,
    Colon(String),
}

#[derive(Debug)]
pub struct TextCodecEnum {
    pub ident: Ident,
    pub format: FormatKind,
    pub storage_lang: String,
    pub variants: Vec<TextCodecVariant>,
}

#[derive(Debug)]
pub struct TextCodecVariant {
    pub ident: Ident,
    pub kind: VariantKind,
    pub prefixes: HashMap<String, String>,
    pub is_catch_all: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VariantKind {
    Unit,
    String,
    VecString,
    Unsupported,
}

pub fn parse_text_codec_enum(input: &DeriveInput) -> Result<TextCodecEnum, syn::Error> {
    let mut format = FormatKind::Paren;
    let mut storage_lang = "en".to_string();

    for attr in &input.attrs {
        if attr.path.is_ident("text_codec") {
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                for nested in list.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                        if nv.path.is_ident("format") {
                            if let Lit::Str(s) = &nv.lit {
                                if s.value() == "paren" {
                                    format = FormatKind::Paren;
                                }
                            }
                        } else if nv.path.is_ident("colon_char") {
                            if let Lit::Str(s) = &nv.lit {
                                format = FormatKind::Colon(s.value());
                            }
                        } else if nv.path.is_ident("storage_lang") {
                            if let Lit::Str(s) = &nv.lit {
                                storage_lang = s.value();
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
        return Err(syn::Error::new_spanned(&input.ident, "TextCodecEnum can only be derived for enums"));
    };

    Ok(TextCodecEnum {
        ident: input.ident.clone(),
        format,
        storage_lang,
        variants,
    })
}

fn parse_variant(variant: &Variant) -> Result<TextCodecVariant, syn::Error> {
    let mut prefixes = HashMap::new();
    let mut is_catch_all = false;

    for attr in &variant.attrs {
        if attr.path.is_ident("prefix") {
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                let (lang, content) = parse_prefix_attribute(&list)?;
                prefixes.insert(lang, content);
            }
        } else if attr.path.is_ident("catch_all") {
            is_catch_all = true;
        }
    }

    let kind = match &variant.fields {
        Fields::Unit => VariantKind::Unit,
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed[0].ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str == "String" || ty_str == "std::string::String" {
                VariantKind::String
            } else if ty_str == "Vec<String>" || ty_str == "std::string::String" {
                VariantKind::VecString
            } else {
                VariantKind::Unsupported
            }
        }
        _ => VariantKind::Unsupported,
    };

    Ok(TextCodecVariant {
        ident: variant.ident.clone(),
        kind,
        prefixes,
        is_catch_all,
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
            } else if nv.path.is_ident("content") {
                if let Lit::Str(s) = &nv.lit {
                    content = Some(s.value());
                }
            }
        }
    }
    let lang = lang.ok_or_else(|| syn::Error::new_spanned(list, "Missing `lang` in `prefix` attribute"))?;
    let content = content.ok_or_else(|| syn::Error::new_spanned(list, "Missing `content` in `prefix` attribute"))?;
    Ok((lang, content))
}
