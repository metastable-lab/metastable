use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, Lit,
    LitStr, Meta, Type, GenericArgument, PathArguments, parse_quote, Field
};

// Helper to check if a type is an Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Option" {
                return true;
            }
        }
    }
    false
}

// Helper to get the inner type from Option<T>
fn get_option_inner_type(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Option" {
                if let PathArguments::AngleBracketed(angle_args) = &last_segment.arguments {
                    if angle_args.args.len() == 1 {
                        if let GenericArgument::Type(inner_ty) = &angle_args.args[0] {
                            return Some(inner_ty.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

// Helper to get the inner type from Vec<T>
fn get_vec_inner_type(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Vec" {
                if let PathArguments::AngleBracketed(angle_args) = &last_segment.arguments {
                    if angle_args.args.len() == 1 {
                        if let GenericArgument::Type(inner_ty) = &angle_args.args[0] {
                            return Some(inner_ty.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

fn get_fully_qualified_type_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}

/// Determines if a given type is a "simple" type that should not be recursed into.
/// Simple types are Rust primitives, String, chrono types, Uuid, Json<T>.
fn is_simple_type(ty: &Type) -> bool {
    let type_str = get_fully_qualified_type_string(ty);
    matches!(type_str.as_str(),
        "String" | "i32" | "u32" | "i64" | "u64" | "isize" | "usize" |
        "f32" | "f64" | "bool" | "Vec<u8>" |
        "Uuid" | "::sqlx::types::Uuid" | "sqlx::types::Uuid" |
        "DateTime<Utc>" | "::chrono::DateTime<::chrono::Utc>" | "chrono::DateTime<chrono::Utc>" |
        "NaiveDateTime" | "::chrono::NaiveDateTime" | "chrono::NaiveDateTime" |
        "NaiveDate" | "::chrono::NaiveDate" | "chrono::NaiveDate" |
        "NaiveTime" | "::chrono::NaiveTime" | "chrono::NaiveTime" |
        "Vector" | "::pgvector::Vector" | "pgvector::Vector"
    ) || type_str.starts_with("Json<") || type_str.starts_with("::sqlx::types::Json<") || type_str.starts_with("sqlx::types::Json<")
}

// Helper to map Rust types to SQL types for PostgreSQL
fn map_rust_type_to_sql(ty: &Type, _is_pk: bool, processing_array_inner: bool, vector_dimension: Option<usize>) -> String {
    let type_str = get_fully_qualified_type_string(ty);

    if type_str == "Vec<u8>" {
        return "BYTEA".to_string();
    }
    if type_str == "Vec<::sqlx::types::Uuid>" || type_str == "Vec<sqlx::types::Uuid>" || type_str == "Vec<Uuid>" {
        return "UUID[]".to_string();
    }
    if type_str == "Vec<String>" || type_str == "Vec<std::string::String>" {
        return "TEXT[]".to_string();
    }

    if !processing_array_inner {
        if let Some(inner_ty) = get_vec_inner_type(ty) {
            let inner_type_sql = map_rust_type_to_sql(&inner_ty, false, true, None);
            if inner_type_sql.ends_with("[]") && !(get_fully_qualified_type_string(&inner_ty).contains("Uuid")) {
                panic!("Multi-dimensional arrays (Vec<Vec<T>>) are not currently supported for SQL mapping beyond Vec<Uuid>.");
            }
            if inner_type_sql == "JSONB" || (inner_type_sql == "BYTEA" && !get_fully_qualified_type_string(&inner_ty).contains("Uuid") ) {
                 panic!("Vec<{}> mapped to SQL type {} cannot be directly made into an SQL array. Consider Json<Vec<{}>> or a different structure.", quote!(#inner_ty), inner_type_sql, quote!(#inner_ty));
            }
            return format!("{}[]", inner_type_sql);
        }
    }

    match type_str.as_str() {
        "String" | "std::string::String" => "TEXT".to_string(),
        "i32" => "INTEGER".to_string(),
        "u32" => "BIGINT".to_string(),
        "i64" | "isize" => "BIGINT".to_string(),
        "u64" | "usize" => "BIGINT".to_string(), 
        "f32" => "REAL".to_string(),
        "f64" => "DOUBLE PRECISION".to_string(),
        "bool" => "BOOLEAN".to_string(),
        "Uuid" | "::sqlx::types::Uuid" | "sqlx::types::Uuid" => "UUID".to_string(),
        "Vector" | "::pgvector::Vector" | "pgvector::Vector" => {
            if let Some(dim) = vector_dimension {
                format!("VECTOR({})", dim)
            } else {
                panic!("Internal error in SqlxObject derive: `map_rust_type_to_sql` was called for a Vector type without a dimension. This should have been caught earlier.");
            }
        },
        s if s.starts_with("Json<") || s.starts_with("::sqlx::types::Json<") || s.starts_with("sqlx::types::Json<") => "JSONB".to_string(),
        "DateTime<Utc>" | "::chrono::DateTime<::chrono::Utc>" | "chrono::DateTime<chrono::Utc>" => "TIMESTAMPTZ".to_string(),
        "NaiveDateTime" | "::chrono::NaiveDateTime" | "chrono::NaiveDateTime" => "TIMESTAMP".to_string(),
        "NaiveDate" | "::chrono::NaiveDate" | "chrono::NaiveDate" => "DATE".to_string(),
        "NaiveTime" | "::chrono::NaiveTime" | "chrono::NaiveTime" => "TIME".to_string(),
        _ if !is_simple_type(ty) && !type_str.starts_with("Option<") && !type_str.starts_with("Vec<") => "TEXT".to_string(),
        _ => {
            panic!("Unsupported Rust type for SQL mapping: '{}'. It is not a recognized simple type, Option<Simple>, Vec<Simple>, Json<T>, or a type that can be mapped to TEXT (e.g. an enum deriving Display/ToString, FromStr, Default).", type_str)
        }
    }
}

#[derive(Debug)]
struct ForeignKeyInfo {
    referenced_table: String,
    related_rust_type: syn::Ident,
}

fn parse_foreign_key_attr(field: &Field) -> Option<ForeignKeyInfo> {
    for attr in field.attrs.iter() {
        if attr.path.is_ident("foreign_key") {
            match attr.parse_meta() {
                Ok(syn::Meta::List(meta_list)) => {
                    let mut referenced_table_opt = None;
                    let mut related_rust_type_str_opt = None;
                    for nested in meta_list.nested.iter() {
                        if let syn::NestedMeta::Meta(syn::Meta::NameValue(mnv)) = nested {
                            if mnv.path.is_ident("referenced_table") {
                                if let syn::Lit::Str(lit_str) = &mnv.lit {
                                    referenced_table_opt = Some(lit_str.value());
                                }
                            } else if mnv.path.is_ident("related_rust_type") {
                                if let syn::Lit::Str(lit_str) = &mnv.lit {
                                    related_rust_type_str_opt = Some(lit_str.value());
                                }
                            }
                        }
                    }
                    if let (Some(rt), Some(rrt_str)) = (referenced_table_opt, related_rust_type_str_opt) {
                        return Some(ForeignKeyInfo {
                            referenced_table: rt,
                            related_rust_type: format_ident!("{}", rrt_str),
                        });
                    } else { return None; }
                }
                Ok(_) => { return None; }
                Err(_) => { return None; }
            }
        }
    }
    None
}

#[derive(Debug)]
struct ForeignKeyManyInfo {
    referenced_table: String,
    related_rust_type: syn::Ident,
}

fn parse_foreign_key_many_attr(field: &Field) -> Option<ForeignKeyManyInfo> {
    for attr in field.attrs.iter() {
        if attr.path.is_ident("foreign_key_many") {
            match attr.parse_meta() {
                Ok(syn::Meta::List(meta_list)) => {
                    let mut referenced_table_opt = None;
                    let mut related_rust_type_str_opt = None;
                    for nested in meta_list.nested.iter() {
                        if let syn::NestedMeta::Meta(syn::Meta::NameValue(mnv)) = nested {
                            if mnv.path.is_ident("referenced_table") {
                                if let syn::Lit::Str(lit_str) = &mnv.lit {
                                    referenced_table_opt = Some(lit_str.value());
                                }
                            } else if mnv.path.is_ident("related_rust_type") {
                                if let syn::Lit::Str(lit_str) = &mnv.lit {
                                    related_rust_type_str_opt = Some(lit_str.value());
                                }
                            }
                        }
                    }
                    if let (Some(rt), Some(rrt_str)) = (referenced_table_opt, related_rust_type_str_opt) {
                        return Some(ForeignKeyManyInfo {
                            referenced_table: rt,
                            related_rust_type: format_ident!("{}", rrt_str),
                        });
                    } else { return None; }
                }
                Ok(_) => { return None; }
                Err(_) => { return None; }
            }
        }
    }
    None
}

fn parse_vector_dimension_attr(field: &Field) -> Option<usize> {
    for attr in field.attrs.iter() {
        if attr.path.is_ident("vector_dimension") {
            if let Ok(syn::Meta::List(meta_list)) = attr.parse_meta() {
                if let Some(syn::NestedMeta::Lit(syn::Lit::Int(lit_int))) = meta_list.nested.first() {
                    return lit_int.base10_parse::<usize>().ok();
                }
            }
        }
    }
    None
}

fn has_unique_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("unique"))
}

fn has_indexed_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("indexed"))
}

fn has_sqlx_skip_column_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("sqlx_skip_column"))
}

#[proc_macro_derive(SqlxObject, attributes(table_name, foreign_key, foreign_key_many, sqlx_skip_column, unique, vector_dimension, indexed))]
pub fn sqlx_object_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;
    let row_struct_name = format_ident!("{}RowSqlx", struct_name);

    let mut custom_table_name_opt: Option<String> = None;
    for attr in &input_ast.attrs {
        if attr.path.is_ident("table_name") {
            match attr.parse_meta() {
                Ok(Meta::NameValue(mnv)) => {
                    if let Lit::Str(lit_str) = &mnv.lit {
                        custom_table_name_opt = Some(lit_str.value());
                        break; // Found it, no need to check other attrs for table_name
                    } else {
                        return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal for Meta::NameValue"); });
                    }
                }
                _ => return TokenStream::from(quote! { compile_error!("table_name attribute must be in the format #[table_name = \"my_table\"]"); }),
            }
        }
    }

    let table_name_str = match custom_table_name_opt {
        Some(name) => name,
        None => {
            return syn::Error::new_spanned(
                struct_name, // Span this error on the struct name
                "#[derive(SqlxObject)] requires the `#[table_name = \"...\"]` attribute to be specified."
            ).to_compile_error().into();
        }
    };

    let all_fields_in_struct = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => &fields_named.named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] is only supported for structs with named fields."); }),
    };

    let active_fields_for_sql: Vec<&Field> = all_fields_in_struct.iter()
        .filter(|field| !has_sqlx_skip_column_attr(field))
        .collect();
    
    let has_updated_at = active_fields_for_sql.iter().any(|f| f.ident.as_ref().unwrap() == "updated_at");

    if active_fields_for_sql.is_empty() {
        return TokenStream::from(quote! { compile_error!("After skipping #[sqlx_skip_column] fields, no fields remain for SQL mapping."); });
    }

    match active_fields_for_sql.iter().find(|f| f.ident.as_ref().map_or(false, |i| i == "id")) {
        Some(field) => {
            if !matches!(field.vis, syn::Visibility::Public(_)) {
                return syn::Error::new_spanned(field.ident.as_ref().unwrap(), "#[derive(SqlxObject)] requires the 'id' field to be public and not skipped.").to_compile_error().into();
            }
            let type_str = get_fully_qualified_type_string(&field.ty);
            if type_str != "Uuid" && type_str != "::sqlx::types::Uuid" && type_str != "sqlx::types::Uuid" {
                 return syn::Error::new_spanned(&field.ty, format!("#[derive(SqlxObject)] requires the 'id' field to be of type 'sqlx::types::Uuid', found '{}'.", type_str)).to_compile_error().into();
            }
        }
        None => {
            return syn::Error::new_spanned(struct_name, "#[derive(SqlxObject)] requires a public field 'id: sqlx::types::Uuid' that is not marked with #[sqlx_skip_column].").to_compile_error().into();
        }
    }

    let row_struct_fields_defs: Vec<proc_macro2::TokenStream> = active_fields_for_sql.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        
        let field_is_option = is_option_type(field_ty);
        
        let mut row_field_ty = field_ty.clone();

        if !is_simple_type(&type_for_analysis) && !is_json_type_for_analysis && !fq_type_str_for_analysis.starts_with("Option<") && !fq_type_str_for_analysis.starts_with("Vec<") {
            row_field_ty = if field_is_option { parse_quote!(Option<String>) } else { parse_quote!(String) };
        } else if let Some(vec_inner_ty) = get_vec_inner_type(field_ty) { 
            let is_inner_text_mappable_enum = !is_simple_type(&vec_inner_ty) && 
                                              !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Option<") && 
                                              !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Vec<");
            if is_inner_text_mappable_enum {
                row_field_ty = parse_quote!(Vec<String>);
            }
        }
        quote! { pub #field_ident: #row_field_ty }
    }).collect();

    let from_row_sql_field_assignments: Vec<proc_macro2::TokenStream> = active_fields_for_sql.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        
        let field_is_option = is_option_type(field_ty);
        let row_field_name = field_ident;

        if !is_simple_type(&type_for_analysis) && !is_json_type_for_analysis && !fq_type_str_for_analysis.starts_with("Option<") && !fq_type_str_for_analysis.starts_with("Vec<") {
            if field_is_option {
                 quote! { #field_ident: row.#row_field_name.map(|s| s.parse().unwrap_or_else(|_| <#type_for_analysis>::default())) }
            } else {
                 quote! { #field_ident: row.#row_field_name.parse().unwrap_or_else(|_| <#type_for_analysis>::default()) }
            }
        } else if let Some(vec_inner_ty) = get_vec_inner_type(field_ty) { 
            let is_vec_inner_type_path = matches!(&vec_inner_ty, syn::Type::Path(_));
            let fq_vec_inner_ty_str = get_fully_qualified_type_string(&vec_inner_ty);

            let is_vec_inner_text_mappable_candidate = 
                is_vec_inner_type_path &&
                !is_simple_type(&vec_inner_ty) &&
                !fq_vec_inner_ty_str.starts_with("Json<") && 
                !fq_vec_inner_ty_str.starts_with("::sqlx::types::Json<") && 
                !fq_vec_inner_ty_str.starts_with("sqlx::types::Json<") &&
                !fq_vec_inner_ty_str.starts_with("Option<") && 
                !fq_vec_inner_ty_str.starts_with("Vec<");

            if is_vec_inner_text_mappable_candidate {
                quote! { #field_ident: row.#row_field_name.into_iter().map(|s: String| s.parse().unwrap_or_else(|_| <#vec_inner_ty>::default())).collect() }
            } else { 
                 quote! { #field_ident: row.#row_field_name } 
            }
        }
        else { quote! { #field_ident: row.#row_field_name } }
    }).collect::<Vec<_>>();
    
    let skipped_field_default_assignments: Vec<proc_macro2::TokenStream> = all_fields_in_struct.iter()
        .filter(|field| has_sqlx_skip_column_attr(field))
        .map(|field| {
            let field_ident = field.ident.as_ref().unwrap();
            quote! { #field_ident: Default::default() }
        })
        .collect();

    let mut all_from_row_assignments = from_row_sql_field_assignments;
    all_from_row_assignments.extend(skipped_field_default_assignments);

    let mut all_sql_column_names_str_lits: Vec<LitStr> = Vec::new();
    let mut create_table_column_defs: Vec<String> = Vec::new();
    let mut foreign_key_clauses_for_create_table: Vec<String> = Vec::new();
    let mut insert_col_sql_names: Vec<String> = Vec::new();
    let mut insert_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_set_clauses_sql: Vec<String> = Vec::new();
    let mut update_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_placeholder_idx = 1;
    let mut fetch_helper_methods: Vec<proc_macro2::TokenStream> = Vec::new(); 
    let mut create_index_sqls: Vec<LitStr> = Vec::new();

    // Helper function to generate the .bind() token stream for a field.
    // This avoids duplicating the complex binding logic for insert and update.
    fn get_bind_stream(
        field_access_path: &proc_macro2::TokenStream,
        field_is_option: bool,
        is_standalone_text_mappable_candidate: bool,
        is_vec_text_mappable_enum: bool,
    ) -> proc_macro2::TokenStream {
        if is_standalone_text_mappable_candidate {
            if field_is_option {
                 quote! { .bind(#field_access_path.as_ref().map(|v| v.to_string())) }
            } else {
                 quote! { .bind(#field_access_path.to_string()) }
            }
        } else if is_vec_text_mappable_enum {
            quote! { .bind(#field_access_path.iter().map(|v| v.to_string()).collect::<Vec<String>>()) }
        } else {
            quote! { .bind(#field_access_path.clone()) }
        }
    }

    for field in active_fields_for_sql.iter() {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let sql_column_name = field_ident.to_string();
        
        all_sql_column_names_str_lits.push(LitStr::new(&sql_column_name, proc_macro2::Span::call_site()));
        
        let field_is_option = is_option_type(field_ty);
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone()); 
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let fq_field_type_str = get_fully_qualified_type_string(field_ty); 

        let vector_dimension = parse_vector_dimension_attr(field);
        let is_vector_type = fq_type_str_for_analysis == "Vector" ||
                             fq_type_str_for_analysis == "::pgvector::Vector" ||
                             fq_type_str_for_analysis == "pgvector::Vector";

        if is_vector_type && vector_dimension.is_none() {
             return syn::Error::new_spanned(field.ident.as_ref().unwrap(), "#[derive(SqlxObject)] requires fields of type `pgvector::Vector` to have a `#[vector_dimension(...)]` attribute.").to_compile_error().into();
        }

        // --- Refined Type Classification for Text-Mappability ---
        let is_type_path_for_analysis = matches!(&type_for_analysis, syn::Type::Path(_));

        // Candidate for standalone text mapping (e.g. an enum or custom struct expected to be FromStr/ToString/Default)
        let is_standalone_text_mappable_candidate =
            is_type_path_for_analysis &&
            !is_simple_type(&type_for_analysis) && // Excludes Uuid, chrono, primitives, Vec<u8>
            !fq_type_str_for_analysis.starts_with("Json<") &&
            !fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") &&
            !fq_type_str_for_analysis.starts_with("sqlx::types::Json<") &&
            get_vec_inner_type(&type_for_analysis).is_none(); // Ensure it's not a Vec itself

        // Validation Logic --- uses the refined classification
        let is_valid_type = if is_simple_type(&type_for_analysis) {
            true 
        } else if is_standalone_text_mappable_candidate {
            true 
        } else if fq_field_type_str.contains("Vec<Uuid>") || fq_field_type_str.contains("Vec<::sqlx::types::Uuid>") {
            true 
        } else if let Some(vec_inner_ty) = get_vec_inner_type(field_ty) {
            let is_vec_inner_type_path = matches!(&vec_inner_ty, syn::Type::Path(_));
            let fq_vec_inner_ty_str = get_fully_qualified_type_string(&vec_inner_ty);

            // Candidate for inner type of Vec for text mapping
            let is_vec_inner_text_mappable_candidate = 
                is_vec_inner_type_path &&
                !is_simple_type(&vec_inner_ty) &&
                !fq_vec_inner_ty_str.starts_with("Json<") && 
                !fq_vec_inner_ty_str.starts_with("::sqlx::types::Json<") && 
                !fq_vec_inner_ty_str.starts_with("sqlx::types::Json<") &&
                !fq_vec_inner_ty_str.starts_with("Option<") && // Inner type of Vec shouldn't be Option for this mapping
                !fq_vec_inner_ty_str.starts_with("Vec<"); // No Vec<Vec<enum>> for text mapping

            is_simple_type(&vec_inner_ty) || is_vec_inner_text_mappable_candidate
        } else {
            false 
        };

        if !is_valid_type {
            // Provide more context in the error message about why a type might be rejected
            let type_kind_for_error = match &type_for_analysis {
                syn::Type::Path(_) => "path",
                syn::Type::TraitObject(_) => "trait_object (e.g. dyn Trait)",
                syn::Type::ImplTrait(_) => "impl_trait (e.g. impl Trait)",
                syn::Type::Reference(_) => "reference",
                syn::Type::Array(_) => "array",
                syn::Type::Ptr(_) => "pointer",
                syn::Type::Tuple(_) => "tuple",
                _ => "other_complex_type"
            };
            return syn::Error::new_spanned(field_ty, 
                format!("Field '{}': Type '{}' (analyzed as '{}', kind: '{}') is not automatically mappable to SQL. SqlxObject supports simple types (primitives, Uuid, chrono types, Vec<u8>), Option<Simple>, Json<T>, or concrete structs/enums that implement ToString/FromStr/Default for TEXT mapping (and Vecs of these). Trait objects and 'impl Trait' are not directly supported as text-mappable fields.",
                        field_ident, fq_field_type_str, fq_type_str_for_analysis, type_kind_for_error)
            ).to_compile_error().into();
        }
        
        let actual_type_for_sql_map = if field_is_option { type_for_analysis.clone() } else { field_ty.clone() };
        let sql_type_str = map_rust_type_to_sql(&actual_type_for_sql_map, sql_column_name == "id", false, vector_dimension);
        
        let mut col_def_parts = vec![format!("\"{}\"", sql_column_name), sql_type_str.clone()];
        let is_pk = sql_column_name == "id";
        if is_pk {
            col_def_parts.push("PRIMARY KEY".to_string());
            col_def_parts.push("DEFAULT gen_random_uuid()".to_string());
        }
        else if sql_column_name == "created_at" || sql_column_name == "updated_at" {
            // Handled by default now, but let's ensure the type is what we expect for the DDL
            let idx = col_def_parts.iter().position(|s| s == &sql_type_str).unwrap();
            col_def_parts[idx] = "BIGINT".to_string();
            col_def_parts.push("NOT NULL DEFAULT floor(extract(epoch from now()))".to_string());
        }
        else if !field_is_option { col_def_parts.push("NOT NULL".to_string()); }

        if has_unique_attr(field) {
            col_def_parts.push("UNIQUE".to_string());
        }

        if has_indexed_attr(field) {
            let index_name = format!("idx_{}_{}", table_name_str, sql_column_name);
            let index_sql = format!(
                "CREATE INDEX IF NOT EXISTS \"{}\" ON \"{}\"(\"{}\")",
                index_name, table_name_str, sql_column_name
            );
            create_index_sqls.push(LitStr::new(&index_sql, field.ident.as_ref().unwrap().span()));
        }

        create_table_column_defs.push(col_def_parts.join(" "));

        if let Some(fk_info) = parse_foreign_key_attr(field) {
            foreign_key_clauses_for_create_table.push(format!(
                "FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"id\") ON DELETE SET NULL ON UPDATE CASCADE",
                sql_column_name, fk_info.referenced_table
            ));
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_info.related_rust_type;
            let self_field_access = quote!{ self.#field_ident };
            
            let id_column_name_of_related_type = quote!{ <#related_type as ::voda_database::SqlxSchema>::id_column_name() };

            if field_is_option {
                fetch_helper_methods.push(quote! {
                    pub async fn #fetch_method_name<'exe, E>(
                        &self, 
                        executor: E
                    ) -> Result<Option<#related_type>, ::sqlx::Error>
                    where
                        E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                        #related_type: ::voda_database::SqlxFilterQuery + ::voda_database::SqlxSchema // Ensure related type implements these
                    {
                        if let Some(id_val_ref) = &#self_field_access {
                            let criteria = ::voda_database::QueryCriteria::new()
                                .add_valued_filter(#id_column_name_of_related_type, "=", *id_val_ref)
                                .expect("SqlxObject derive: Failed to build QueryCriteria in fetch helper for Option<ForeignKey>.");
                            <#related_type as ::voda_database::SqlxFilterQuery>::find_one_by_criteria(criteria, executor).await
                        } else {
                            Ok(None)
                        }
                    }
                });
            } else {
                fetch_helper_methods.push(quote! {
                    pub async fn #fetch_method_name<'exe, E>(
                        &self, 
                        executor: E
                    ) -> Result<Option<#related_type>, ::sqlx::Error> // find_one_by_criteria always returns Option<Self>
                    where
                        E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                        #related_type: ::voda_database::SqlxFilterQuery + ::voda_database::SqlxSchema // Ensure related type implements these
                    {
                        let criteria = ::voda_database::QueryCriteria::new()
                            .add_valued_filter(#id_column_name_of_related_type, "=", #self_field_access)
                            .expect("SqlxObject derive: Failed to build QueryCriteria in fetch helper for ForeignKey.");
                        <#related_type as ::voda_database::SqlxFilterQuery>::find_one_by_criteria(criteria, executor).await
                    }
                });
            }
        } 
        else if let Some(fk_many_info) = parse_foreign_key_many_attr(field) {
            if !(fq_field_type_str.contains("Vec<Uuid>") || fq_field_type_str.contains("Vec<::sqlx::types::Uuid>")) {
                return syn::Error::new_spanned(field_ty, "foreign_key_many attribute can only be used on fields of type Vec<Uuid> or Vec<::sqlx::types::Uuid>.").to_compile_error().into();
            }
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_many_info.related_rust_type;
            let referenced_table_str = &fk_many_info.referenced_table;
            
            fetch_helper_methods.push(quote! {
                pub async fn #fetch_method_name<'exe, E>(
                    &self, 
                    executor: E
                ) -> Result<Vec<#related_type>, sqlx::Error>
                where
                    E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                {
                    if self.#field_ident.is_empty() {
                        return Ok(Vec::new());
                    }
                    let ids = &self.#field_ident;
                    let sql = format!("SELECT * FROM \"{}\" WHERE \"id\" = ANY($1)", #referenced_table_str);
                    
                    let related_rows = sqlx::query_as::<_, <#related_type as ::voda_database::SqlxSchema>::Row>(&sql)
                        .bind(ids)
                        .fetch_all(executor)
                        .await?;
                    
                    Ok(related_rows.into_iter().map(<#related_type as ::voda_database::SqlxSchema>::from_row).collect())
                }
            });
        }
        let field_access_path = quote!{ self.#field_ident };

        // Candidate for standalone text mapping (e.g. an enum or custom struct expected to be FromStr/ToString/Default)
        let is_standalone_text_mappable_candidate =
            is_type_path_for_analysis &&
            !is_simple_type(&type_for_analysis) && // Excludes Uuid, chrono, primitives, Vec<u8>
            !fq_type_str_for_analysis.starts_with("Json<") &&
            !fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") &&
            !fq_type_str_for_analysis.starts_with("sqlx::types::Json<") &&
            get_vec_inner_type(&type_for_analysis).is_none(); // Ensure it's not a Vec itself
        
        let is_vec_text_mappable_enum = get_vec_inner_type(field_ty).map_or(false, |vt| 
            !is_simple_type(&vt) && 
            !get_fully_qualified_type_string(&vt).starts_with("Option<") && 
            !get_fully_qualified_type_string(&vt).starts_with("Vec<")
        );
        
        let bind_stream = get_bind_stream(
            &field_access_path,
            field_is_option,
            is_standalone_text_mappable_candidate,
            is_vec_text_mappable_enum
        );
        
        // Exclude timestamp fields and id from binding lists
        if field_ident != "created_at" && field_ident != "updated_at" && !is_pk {
            insert_col_sql_names.push(format!("\"{}\"", sql_column_name));
            insert_bindings_streams.push(bind_stream.clone());

            if !is_pk {
                update_set_clauses_sql.push(format!("\"{}\" = ${}", sql_column_name, update_placeholder_idx));
                update_placeholder_idx += 1;
                update_bindings_streams.push(bind_stream);
            }
        }
    }

    let mut create_table_parts = create_table_column_defs;
    if !foreign_key_clauses_for_create_table.is_empty() {
        create_table_parts.extend(foreign_key_clauses_for_create_table);
    }
    let create_table_sql_query = format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name_str, create_table_parts.join(", "));
    let drop_table_sql_query = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table_name_str);

    let all_sql_columns_joined_str = all_sql_column_names_str_lits.iter().map(|s| format!("\"{}\"", s.value())).collect::<Vec<String>>().join(", ");
    
    let insert_column_names_joined_sql = insert_col_sql_names.join(", ");
    let insert_bind_placeholders_sql = (1..=insert_col_sql_names.len()).map(|i| format!("${}", i)).collect::<Vec<String>>().join(", ");
    let insert_sql_query = format!("INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}", table_name_str, insert_column_names_joined_sql, insert_bind_placeholders_sql, all_sql_columns_joined_str);

    let pk_binding_for_update = quote! { .bind(self.id) };
    let update_set_str_sql = update_set_clauses_sql.join(", ");
    
    let update_by_id_sql_query_is_select = update_set_clauses_sql.is_empty();
    let sql_for_update_instance_by_id = if update_by_id_sql_query_is_select {
        format!("SELECT {} FROM \"{}\" WHERE \"id\" = $1", all_sql_columns_joined_str, table_name_str) 
    } else {
        format!("UPDATE \"{}\" SET {} WHERE \"id\" = ${} RETURNING {}", table_name_str, update_set_str_sql, update_placeholder_idx, all_sql_columns_joined_str)
    };
    
    let sql_for_delete_instance_by_id = format!("DELETE FROM \"{}\" WHERE \"id\" = $1", table_name_str);

    let final_pk_id_trait_type: Type = parse_quote!(::sqlx::types::Uuid);    
    let trigger_sql_impl = if has_updated_at {
        let trigger_name = format!("set_updated_at_{}", table_name_str);
        format!(
            "DROP TRIGGER IF EXISTS {trigger} ON \"{table}\"; CREATE TRIGGER {trigger} BEFORE UPDATE ON \"{table}\" FOR EACH ROW EXECUTE PROCEDURE set_updated_at_unix_timestamp();",
            trigger = trigger_name,
            table = table_name_str
        )
    } else {
        "".to_string()
    };

    let expanded = quote! {
        #[derive(::sqlx::FromRow, Debug, Clone)]
        #[automatically_derived]
        pub struct #row_struct_name {
            #(#row_struct_fields_defs),*
        }

        #[automatically_derived]
        impl ::voda_database::SqlxSchema for #struct_name {
            type Id = #final_pk_id_trait_type;
            type Row = #row_struct_name;

            const TABLE_NAME: &'static str = #table_name_str;
            const ID_COLUMN_NAME: &'static str = "id";
            const COLUMNS: &'static [&'static str] = &[#( #all_sql_column_names_str_lits ),*];
            const INDEXES_SQL: &'static [&'static str] = &[#( #create_index_sqls ),*];

            fn get_id_value(&self) -> Self::Id { self.id }

            fn from_row(row: Self::Row) -> Self {
                Self {
                    #(#all_from_row_assignments),*
                }
            }

            fn insert_sql() -> String { #insert_sql_query.to_string() }
            fn create_table_sql() -> String { #create_table_sql_query.to_string() }
            fn drop_table_sql() -> String { #drop_table_sql_query.to_string() }
            fn trigger_sql() -> String { #trigger_sql_impl.to_string() }
        }

        #[automatically_derived]
        #[::async_trait::async_trait]
        impl ::voda_database::SqlxCrud for #struct_name {
            fn bind_insert<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                query #(#insert_bindings_streams)*
            }

            fn bind_update<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                if #update_by_id_sql_query_is_select { 
                    query #pk_binding_for_update
                } else {
                    query #(#update_bindings_streams)* #pk_binding_for_update
                }
            }

            async fn create<'e, E>(self, executor: E) -> Result<Self, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let sql = <Self as ::voda_database::SqlxSchema>::insert_sql();
                self.bind_insert(::sqlx::query_as::<_, <Self as ::voda_database::SqlxSchema>::Row>(&sql))
                    .fetch_one(executor)
                    .await
                    .map(<Self as ::voda_database::SqlxSchema>::from_row)
            }

            async fn update<'e, E>(self, executor: E) -> Result<Self, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let sql = #sql_for_update_instance_by_id;
                self.bind_update(::sqlx::query_as::<_, <Self as ::voda_database::SqlxSchema>::Row>(&sql))
                    .fetch_one(executor)
                    .await
                    .map(<Self as ::voda_database::SqlxSchema>::from_row)
            }

            async fn delete<'e, E>(self, executor: E) -> Result<u64, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let sql = #sql_for_delete_instance_by_id;
                ::sqlx::query(&sql)
                    .bind(self.id)
                    .execute(executor)
                    .await
                    .map(|done| done.rows_affected())
            }
        }

        #[automatically_derived]
        impl #struct_name {
            #(#fetch_helper_methods)*
        }
        
        #[automatically_derived]
        #[::async_trait::async_trait]
        impl ::voda_database::SqlxFilterQuery for #struct_name {
            async fn find_by_criteria<'exe, E>(
                mut criteria: ::voda_database::QueryCriteria,
                executor: E,
            ) -> Result<Vec<Self>, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                Self: Send,
            {
                let mut sql_query_parts: Vec<String> = Vec::new();
                let mut placeholder_idx = 1;
                let mut select_columns = (<Self as ::voda_database::SqlxSchema>::COLUMNS).join(", ");

                let similarity_search_info = criteria.find_similarity.take();
                let similarity_threshold = criteria.similarity_threshold.take();
                let mut embedding_placeholder_idx: usize = 0;

                if let Some((embedding, as_field)) = &similarity_search_info {
                    use ::sqlx::Arguments;
                    embedding_placeholder_idx = placeholder_idx;
                    criteria.arguments.add(embedding.clone()).map_err(::sqlx::Error::Encode)?;
                    placeholder_idx += 1;
                    select_columns = format!("*, 1 - (embedding <=> ${}) as {}", embedding_placeholder_idx, as_field);
                }

                // Use fully qualified path for schema items
                sql_query_parts.push(format!(
                    "SELECT {} FROM \"{}\"", 
                    select_columns, 
                    <Self as ::voda_database::SqlxSchema>::TABLE_NAME
                ));

                if !criteria.conditions.is_empty() {
                    sql_query_parts.push("WHERE".to_string());
                    let mut first_condition = true;
                    for condition in &criteria.conditions { // Iterate by reference
                        if !first_condition {
                            sql_query_parts.push("AND".to_string());
                        }
                        first_condition = false;
                        
                        let mut current_condition_sql = format!("\"{}\" {}", condition.column, condition.operator);
                        if condition.uses_placeholder {
                            if !condition.operator.contains('$') { // If operator is simple (e.g., "=")
                                current_condition_sql.push_str(&format!(" ${}", placeholder_idx));
                            }
                            // If operator contains '$' (e.g., "= ANY($1)"), we assume it's correctly formatted.
                            // The placeholder_idx still needs to be incremented to account for the argument.
                            placeholder_idx += 1;
                        }
                        sql_query_parts.push(current_condition_sql);
                    }
                }

                if let Some(threshold) = similarity_threshold {
                    if criteria.conditions.is_empty() && similarity_search_info.is_none() {
                        // This case is ambiguous. What if they only provide a threshold?
                        // For now, let's assume it only works with a similarity search.
                    }
                    if let Some(_) = &similarity_search_info {
                        if criteria.conditions.is_empty() {
                            sql_query_parts.push("WHERE".to_string());
                        } else {
                            sql_query_parts.push("AND".to_string());
                        }
                        let condition_sql = format!("1 - (embedding <=> ${}) >= ${}", embedding_placeholder_idx, placeholder_idx);
                        sql_query_parts.push(condition_sql);
                        use ::sqlx::Arguments;
                        criteria.arguments.add(threshold).map_err(::sqlx::Error::Encode)?;
                        placeholder_idx += 1;
                    }
                }

                if !criteria.order_by.is_empty() {
                    sql_query_parts.push("ORDER BY".to_string());
                    let order_clauses: Vec<String> = criteria.order_by.iter().map(|&(col, dir)| {
                        // Allow ordering by aliased similarity field, which isn't a real column
                        if col == similarity_search_info.as_ref().map_or("", |ssi| ssi.1) {
                            format!("{} {}", col, dir.as_sql())
                        } else {
                            format!("\"{}\" {}", col, dir.as_sql())
                        }
                    }).collect();
                    sql_query_parts.push(order_clauses.join(", "));
                }

                if criteria.has_limit {
                    sql_query_parts.push(format!("LIMIT ${}", placeholder_idx));
                    placeholder_idx += 1;
                }

                if criteria.has_offset {
                    sql_query_parts.push(format!("OFFSET ${}", placeholder_idx));
                    // placeholder_idx += 1; // Not strictly needed for the last placeholder
                }

                let final_sql = sql_query_parts.join(" ");
                
                // criteria.arguments already has all necessary values in order (filters, then limit, then offset)
                ::sqlx::query_as_with::<_, <Self as ::voda_database::SqlxSchema>::Row, _>(&final_sql, criteria.arguments)
                    .fetch_all(executor)
                    .await
                    .map(|rows| rows.into_iter().map(<Self as ::voda_database::SqlxSchema>::from_row).collect())
            }

            async fn delete_by_criteria<'exe, E>(
                criteria: ::voda_database::QueryCriteria,
                executor: E,
            ) -> Result<u64, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                Self: Send,
            {
                let mut sql_query_parts: Vec<String> = Vec::new();
                let mut placeholder_idx = 1;
                
                sql_query_parts.push(format!("DELETE FROM \"{}\"", <Self as ::voda_database::SqlxSchema>::TABLE_NAME));

                if !criteria.conditions.is_empty() {
                    sql_query_parts.push("WHERE".to_string());
                    let mut first_condition = true;
                    for condition in &criteria.conditions { 
                        if !first_condition {
                            sql_query_parts.push("AND".to_string());
                        }
                        first_condition = false;
                        
                        let mut current_condition_sql = format!("\"{}\" {}", condition.column, condition.operator);
                        if condition.uses_placeholder {
                            if !condition.operator.contains('$') { // If operator is simple (e.g., "=")
                                current_condition_sql.push_str(&format!(" ${}", placeholder_idx));
                            }
                            // If operator contains '$' (e.g., "= ANY($1)"), we assume it's correctly formatted.
                            // The placeholder_idx still needs to be incremented to account for the argument.
                            placeholder_idx += 1;
                        }
                        sql_query_parts.push(current_condition_sql);
                    }
                } else {
                    // Deleting without a WHERE clause is dangerous.
                    // Consider returning an error or requiring at least one condition.
                    // For now, let it proceed if criteria.conditions is empty, which means deleting all rows.
                    // Production systems should have safeguards.
                }
                
                // LIMIT and OFFSET are not typically used with DELETE in this manner in Postgres for `delete_by_criteria`.
                // If criteria included limit/offset, they would be ignored by this SQL construction for DELETE.
                // Ordering is also irrelevant for DELETE.

                let final_sql = sql_query_parts.join(" ");
                
                ::sqlx::query_with(&final_sql, criteria.arguments)
                    .execute(executor)
                    .await
                    .map(|done| done.rows_affected())
            }
        }
    };
    
    // println!("Generated code for {}: {}", stringify!(#struct_name), expanded.to_string());
    TokenStream::from(expanded)
}