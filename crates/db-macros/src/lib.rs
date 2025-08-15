use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, format_ident};
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Lit, Meta};

mod internals;
use internals::{
    codegen::{generate_migrate_fn, generate_row_struct, generate_sqlx_schema_impl, generate_sqlx_crud_impl, generate_sqlx_filter_query_impl, generate_fetch_helpers},
    parse::get_fields_data,
};

#[proc_macro_derive(SqlxObject, attributes(table_name, foreign_key, foreign_key_many, sqlx_skip_column, unique, vector_dimension, indexed, allow_column_dropping))]
pub fn sqlx_object_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;

    // --- Parsing and Validation ---
    let mut custom_table_name_opt: Option<String> = None;
    for attr in &input_ast.attrs {
        if attr.path.is_ident("table_name") {
            if let Ok(Meta::NameValue(mnv)) = attr.parse_meta() {
                if let Lit::Str(lit_str) = mnv.lit {
                    custom_table_name_opt = Some(lit_str.value());
                    break;
                }
            }
        }
    }

    let table_name_str = match custom_table_name_opt {
        Some(name) => name,
        None => {
            return syn::Error::new_spanned(struct_name, "#[derive(SqlxObject)] requires `#[table_name = \"...\"]` attribute.")
                .to_compile_error()
                .into();
        }
    };

    let all_fields_in_struct = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => &fields_named.named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] only supports structs with named fields."); }),
    };

    let fields_data = get_fields_data(all_fields_in_struct);

    // --- Code Generation ---
    let row_struct_name = format_ident!("{}RowSqlx", struct_name);
    let allow_column_dropping = input_ast.attrs.iter().any(|attr| attr.path.is_ident("allow_column_dropping"));
    
    let row_struct_def = generate_row_struct(&row_struct_name, &fields_data);
    let sqlx_schema_impl = generate_sqlx_schema_impl(struct_name, &row_struct_name, &table_name_str, &fields_data);
    let sqlx_crud_impl = generate_sqlx_crud_impl(struct_name, &table_name_str, &fields_data);
    let sqlx_filter_query_impl = generate_sqlx_filter_query_impl(struct_name, &row_struct_name);
    
    let fetch_helpers = generate_fetch_helpers(&fields_data);
    let migrate_impl = generate_migrate_fn(struct_name, &table_name_str, &fields_data, allow_column_dropping);

    let expanded = quote! {
        use ::metastable_database::{SqlxSchema, SqlxCrud, SqlxFilterQuery, QueryCriteria};

        #row_struct_def
        #sqlx_schema_impl
        #sqlx_crud_impl
        #sqlx_filter_query_impl
        
        #[automatically_derived]
        impl #struct_name {
            #fetch_helpers
        }

        #migrate_impl
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(TextCodecEnum, attributes(text_codec, prefix, catch_all, collection))]
pub fn text_codec_enum_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let enum_ident = &input_ast.ident;

    let mut format_kind: Option<String> = None; // "paren" | "colon"
    let mut storage_lang: String = "en".to_string();
    let mut colon_char: String = ":".to_string();

    for attr in &input_ast.attrs {
        if attr.path.is_ident("text_codec") {
            if let Ok(Meta::List(list)) = attr.parse_meta() {
                for nested in list.nested.iter() {
                    if let syn::NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                        if nv.path.is_ident("format") {
                            if let Lit::Str(s) = &nv.lit { format_kind = Some(s.value()); }
                        } else if nv.path.is_ident("storage_lang") {
                            if let Lit::Str(s) = &nv.lit { storage_lang = s.value(); }
                        } else if nv.path.is_ident("colon_char") {
                            if let Lit::Str(s) = &nv.lit { colon_char = s.value(); }
                        }
                    }
                }
            }
        }
    }

    let format_kind = format_kind.unwrap_or_else(|| "paren".to_string());
    let is_paren = format_kind == "paren";
    let colon_char_val = colon_char.clone();

    // Collect variant data: name, tuple inner shape, prefixes per lang, is_catch_all, collection formatting
    let enum_data = match &input_ast.data {
        Data::Enum(data_enum) => data_enum,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(TextCodecEnum)] only supports enums"); }),
    };

    struct VariantInfo {
        ident: syn::Ident,
        is_tuple_string: bool,
        is_tuple_vec_string: bool,
        is_unit: bool,
        is_catch_all: bool,
        prefixes: Vec<(String, String)>, // (lang, content)
    }

    let mut variants: Vec<VariantInfo> = Vec::new();
    let mut has_catch_all = false;

    for v in &enum_data.variants {
        let mut info = VariantInfo {
            ident: v.ident.clone(),
            is_tuple_string: false,
            is_tuple_vec_string: false,
            is_unit: matches!(v.fields, syn::Fields::Unit),
            is_catch_all: false,
            prefixes: Vec::new(),
        };

        match &v.fields {
            syn::Fields::Unnamed(unnamed) => {
                if unnamed.unnamed.len() == 1 {
                    let ty = &unnamed.unnamed[0].ty;
                    let ty_str = quote!(#ty).to_string().replace(' ', "");
                    if ty_str == "String" || ty_str == "std::string::String" { info.is_tuple_string = true; }
                    if ty_str == "Vec<String>" || ty_str == "Vec<std::string::String>" { info.is_tuple_vec_string = true; }
                }
            }
            _ => {}
        }

        for attr in &v.attrs {
            if attr.path.is_ident("prefix") {
                if let Ok(Meta::List(list)) = attr.parse_meta() {
                    let mut lang: Option<String> = None;
                    let mut content: Option<String> = None;
                    for nested in list.nested.iter() {
                        if let syn::NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                            if nv.path.is_ident("lang") { if let Lit::Str(s) = &nv.lit { lang = Some(s.value()); } }
                            if nv.path.is_ident("content") { if let Lit::Str(s) = &nv.lit { content = Some(s.value()); } }
                        }
                    }
                    if let (Some(l), Some(c)) = (lang, content) { info.prefixes.push((l, c)); }
                }
            } else if attr.path.is_ident("catch_all") {
                if let Ok(Meta::List(list)) = attr.parse_meta() {
                    for nested in list.nested.iter() {
                        if let syn::NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                            if nv.path.is_ident("no_prefix") { info.is_catch_all = true; }
                        }
                    }
                }
            }
        }

        if info.is_catch_all { has_catch_all = true; }
        variants.push(info);
    }

    // Build prefix maps
    let mut all_langs: Vec<String> = Vec::new();
    for v in &variants { for (l, _) in &v.prefixes { if !all_langs.contains(l) { all_langs.push(l.clone()); } } }
    if !all_langs.contains(&storage_lang) { all_langs.push(storage_lang.clone()); }

    let schema_impl = {
        let mut arms = quote! {};
        for lang in &all_langs {
            let variant_names: Vec<String> = variants
                .iter()
                .map(|v| {
                    v.prefixes
                        .iter()
                        .find(|(l, _)| l == lang)
                        .map(|(_, c)| c.clone())
                        .unwrap_or_else(|| v.ident.to_string())
                })
                .collect();
            let lang_lit = lang.as_str();
            arms.extend(quote! {
                Some(#lang_lit) => serde_json::json!({
                    "type": "string",
                    "enum": [#(#variant_names),*]
                }),
            });
        }

        let default_variant_names: Vec<String> = variants
            .iter()
            .map(|v| v.ident.to_string())
            .collect();

        quote! {
            impl #enum_ident {
                pub fn schema(lang: Option<&str>) -> serde_json::Value {
                    match lang {
                        #arms
                        _ => serde_json::json!({
                            "type": "string",
                            "enum": [#(#default_variant_names),*]
                        }),
                    }
                }
            }
        }
    };

    // Display and to_lang arms
    let display_match_arms: Vec<_> = variants.iter().map(|vi| {
        let ident = &vi.ident;
        if vi.is_unit {
            let prefix = vi.prefixes.iter().find(|(l, _)| l == &storage_lang).map(|(_, c)| c.clone()).unwrap_or_else(|| ident.to_string());
            quote! { Self::#ident => write!(f, "{}", #prefix) }
        } else if vi.is_tuple_string {
            let prefix = vi.prefixes.iter().find(|(l, _)| l == &storage_lang).map(|(_, c)| c.clone()).unwrap_or_else(|| ident.to_string());
            if is_paren { quote! { Self::#ident(inner) => write!(f, "{}({})", #prefix, inner) } }
            else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); quote! { Self::#ident(inner) => write!(f, "{}{} {}", #prefix, #cc, inner) } }
        } else if vi.is_tuple_vec_string {
            let open = "[".to_string();
            let sep = ",".to_string();
            let close = "]".to_string();
            let prefix = vi.prefixes.iter().find(|(l, _)| l == &storage_lang).map(|(_, c)| c.clone()).unwrap_or_else(|| ident.to_string());
            if is_paren { quote! { Self::#ident(vecv) => write!(f, "{}({}{}{})", #prefix, #open, vecv.join(#sep), #close) } }
            else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); quote! { Self::#ident(vecv) => write!(f, "{}{} {}{}{}", #prefix, #cc, #open, vecv.join(#sep), #close) } }
        } else { quote!{} }
    }).collect();

    let to_lang_match_arms: Vec<_> = variants.iter().map(|vi| {
        let ident = &vi.ident;
        let lang_match_arms: Vec<_> = vi.prefixes.iter().map(|(l, c)| {
            let l_lit = syn::LitStr::new(l, Span::call_site());
            let c_lit = syn::LitStr::new(c, Span::call_site());
            quote! { #l_lit => #c_lit }
        }).collect();
        let ident_lit = syn::LitStr::new(&ident.to_string(), Span::call_site());
        if vi.is_unit {
            quote! { Self::#ident => {
                let prefix = match lang {
                    #(#lang_match_arms,)*
                    _ => #ident_lit,
                };
                prefix.to_string()
            } }
        } else if vi.is_tuple_string {
            if is_paren { quote! { Self::#ident(inner) => {
                let prefix = match lang {
                    #(#lang_match_arms,)*
                    _ => #ident_lit,
                };
                format!("{}({})", prefix, inner)
            } } } else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); quote! { Self::#ident(inner) => {
                let prefix = match lang {
                    #(#lang_match_arms,)*
                    _ => #ident_lit,
                };
                format!("{}{} {}", prefix, #cc, inner)
            } } }
        } else if vi.is_tuple_vec_string {
            let open = "[".to_string();
            let sep = ",".to_string();
            let close = "]".to_string();
            if is_paren { quote! { Self::#ident(vecv) => {
                let prefix = match lang {
                    #(#lang_match_arms,)*
                    _ => #ident_lit,
                };
                format!("{}({}{}{})", prefix, #open, vecv.join(#sep), #close)
            } } } else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); quote! { Self::#ident(vecv) => {
                let prefix = match lang {
                    #(#lang_match_arms,)*
                    _ => #ident_lit,
                };
                format!("{}{} {}{}{}", prefix, #cc, #open, vecv.join(#sep), #close)
            } } }
        } else { quote!{} }
    }).collect();

    // FromStr / parse_any_lang generation: try storage_lang first, then all langs; match both formats; fall back to catchall
    let parse_match_arms_storage: Vec<_> = variants.iter().map(|vi| {
        let ident = &vi.ident;
        let prefix = vi.prefixes.iter().find(|(l, _)| l == &storage_lang).map(|(_, c)| c.clone()).unwrap_or_else(|| ident.to_string());
        if vi.is_unit {
            quote! {
                if s == #prefix { return Ok(Self::#ident); }
            }
        } else if vi.is_tuple_string {
            if is_paren {
                quote! {
                    if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                        return Ok(Self::#ident(inner.to_string()));
                    }
                }
            } else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); quote! {
                    if let Some(inner) = s.strip_prefix(&format!("{}{} ", #prefix, #cc)) {
                        return Ok(Self::#ident(inner.to_string()));
                    }
                } }
        } else if vi.is_tuple_vec_string {
            if is_paren {
                let open_lit = syn::LitStr::new("[", Span::call_site());
                let sep_lit = syn::LitStr::new(",", Span::call_site());
                let close_lit = syn::LitStr::new("]", Span::call_site());
                quote! {
                    if let Some(inner) = s.strip_prefix(&format!("{}(", #prefix)).and_then(|t| t.strip_suffix(")")) {
                        if let Some(inner) = inner.strip_prefix(#open_lit).and_then(|t| t.strip_suffix(#close_lit)) {
                            let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep_lit).map(|s| s.trim().to_string()).collect() };
                            return Ok(Self::#ident(vec));
                        }
                    }
                }
            } else { let cc = syn::LitStr::new(&colon_char_val, Span::call_site()); let open_lit = syn::LitStr::new("[", Span::call_site()); let sep_lit = syn::LitStr::new(",", Span::call_site()); let close_lit = syn::LitStr::new("]", Span::call_site()); quote! {
                    if let Some(inner) = s.strip_prefix(&format!("{}{} ", #prefix, #cc)) {
                        if let Some(inner) = inner.strip_prefix(#open_lit).and_then(|t| t.strip_suffix(#close_lit)) {
                            let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep_lit).map(|s| s.trim().to_string()).collect() };
                            return Ok(Self::#ident(vec));
                        }
                    }
                } }
        } else { quote! {} }
    }).collect();

    // parse_any_lang: try every language prefix
    let parse_any_lang_arms: Vec<_> = variants.iter().flat_map(|vi| {
        let ident = &vi.ident;
        let cc_lit_variant = syn::LitStr::new(&colon_char_val, Span::call_site());
        vi.prefixes.iter().map(move |(_l, c)| {
            let c_lit = syn::LitStr::new(c, Span::call_site());
            if vi.is_unit {
                quote! { if s == #c_lit { return Ok(Self::#ident); } }
            } else if vi.is_tuple_string {
                if is_paren {
                    quote! { if let Some(inner) = s.strip_prefix(&format!("{}(", #c_lit)).and_then(|t| t.strip_suffix(")")) { return Ok(Self::#ident(inner.to_string())); } }
                } else {
                    let cc = cc_lit_variant.clone();
                    quote! { if let Some(inner) = s.strip_prefix(&format!("{}{} ", #c_lit, #cc)) { return Ok(Self::#ident(inner.to_string())); } }
                }
            } else if vi.is_tuple_vec_string {
                if is_paren {
                    let open_lit = syn::LitStr::new("[", Span::call_site());
                    let sep_lit = syn::LitStr::new(",", Span::call_site());
                    let close_lit = syn::LitStr::new("]", Span::call_site());
                    quote! { if let Some(inner) = s.strip_prefix(&format!("{}(", #c_lit)).and_then(|t| t.strip_suffix(")")) { if let Some(inner) = inner.strip_prefix(#open_lit).and_then(|t| t.strip_suffix(#close_lit)) { let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep_lit).map(|s| s.trim().to_string()).collect() }; return Ok(Self::#ident(vec)); } } }
                } else {
                    let cc = cc_lit_variant.clone();
                    let open_lit = syn::LitStr::new("[", Span::call_site());
                    let sep_lit = syn::LitStr::new(",", Span::call_site());
                    let close_lit = syn::LitStr::new("]", Span::call_site());
                    quote! { if let Some(inner) = s.strip_prefix(&format!("{}{} ", #c_lit, #cc)) { if let Some(inner) = inner.strip_prefix(#open_lit).and_then(|t| t.strip_suffix(#close_lit)) { let vec = if inner.trim().is_empty() { Vec::new() } else { inner.split(#sep_lit).map(|s| s.trim().to_string()).collect() }; return Ok(Self::#ident(vec)); } } }
                }
            } else { quote! {} }
        })
    }).collect();

    let parse_with_type_and_content_arms: Vec<_> = variants.iter().flat_map(|vi| {
        let ident = &vi.ident;
        let content_str_ident = quote! { content_str };
        vi.prefixes.iter().map(move |(_l, c)| {
            let c_lit = syn::LitStr::new(c, Span::call_site());
            if vi.is_unit {
                quote! { if type_str == #c_lit { return Ok(Self::#ident); } }
            } else if vi.is_tuple_string {
                quote! { if type_str == #c_lit { return Ok(Self::#ident(#content_str_ident.to_string())); } }
            } else if vi.is_tuple_vec_string {
                quote! { if type_str == #c_lit { let vec = if #content_str_ident.trim().is_empty() { Vec::new() } else { #content_str_ident.split(',').map(|s| s.trim().to_string()).collect() }; return Ok(Self::#ident(vec)); } }
            } else { quote! {} }
        })
    }).collect();

    // Default impl: if there is a catch-all variant with String, return it on unknown/unprefixed
    let catch_all_ctor_s = if has_catch_all {
        let ca_ident = variants.iter().find(|v| v.is_catch_all && v.is_tuple_string).map(|v| v.ident.clone());
        if let Some(id) = ca_ident { quote! { return Ok(Self::#id(s.to_string())); } } else { quote! {} }
    } else { quote! {} };

    let catch_all_ctor_type_content = if has_catch_all {
        let ca_ident = variants.iter().find(|v| v.is_catch_all && v.is_tuple_string).map(|v| v.ident.clone());
        if let Some(id) = ca_ident {
            if is_paren {
                quote! { return Ok(Self::#id(format!("{}({})", type_str, content_str))); }
            } else {
                let cc = syn::LitStr::new(&colon_char_val, Span::call_site());
                quote! { return Ok(Self::#id(format!("{}{} {}", type_str, #cc, content_str))); }
            }
        } else { quote! {} }
    } else { quote! {} };

    // Default impl for enums with catchall -> empty string; otherwise, derive Default is expected elsewhere
    let default_impl = if has_catch_all {
        let ca_ident = variants.iter().find(|v| v.is_catch_all && v.is_tuple_string).map(|v| v.ident.clone());
        if let Some(id) = ca_ident { quote! { impl Default for #enum_ident { fn default() -> Self { Self::#id(String::new()) } } } } else { quote! {} }
    } else { quote! {} };

    let bail_stmt = if has_catch_all {
        quote!{}
    } else {
        quote!{ anyhow::bail!("Invalid {} string: {}", stringify!(#enum_ident), s) }
    };

    let bail_stmt_type_content = if has_catch_all {
        quote!{}
    } else {
        quote!{ anyhow::bail!("Invalid {} type: {} or content: {}", stringify!(#enum_ident), type_str, content_str) }
    };

    let expanded = quote! {
        impl ::std::fmt::Display for #enum_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#display_match_arms,)*
                }
            }
        }

        impl ::std::str::FromStr for #enum_ident {
            type Err = anyhow::Error;
            fn from_str(s: &str) -> anyhow::Result<Self> {
                let s = s.trim();
                { #(#parse_match_arms_storage)* }
                { #(#parse_any_lang_arms)* }
                #catch_all_ctor_s
                #bail_stmt
            }
        }

        impl ::metastable_database::TextPromptCodec for #enum_ident {
            fn to_lang(&self, lang: &str) -> String {
                match self {
                    #(#to_lang_match_arms,)*
                }
            }

            fn parse_any_lang(s: &str) -> anyhow::Result<Self> {
                let s = s.trim();
                { #(#parse_any_lang_arms)* }
                #catch_all_ctor_s
                #bail_stmt
            }

            fn parse_with_type_and_content(type_str: &str, content_str: &str) -> anyhow::Result<Self> {
                { #(#parse_with_type_and_content_arms)* }
                #catch_all_ctor_type_content
                #bail_stmt_type_content
            }
        }

        #default_impl
    };

    let final_expanded = quote! {
        #expanded
        #schema_impl
    };

    TokenStream::from(final_expanded)
}