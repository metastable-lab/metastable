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
/// Simple types are Rust primitives, String, chrono types, Uuid, CryptoHash, Json<T>.
fn is_simple_type(ty: &Type) -> bool {
    let type_str = get_fully_qualified_type_string(ty);
    matches!(type_str.as_str(),
        "String" | "i32" | "u32" | "i64" | "u64" | "isize" | "usize" |
        "f32" | "f64" | "bool" | "Vec<u8>" |
        "Uuid" | "::sqlx::types::Uuid" | "sqlx::types::Uuid" |
        "CryptoHash" | "::voda_common::CryptoHash" | "voda_common::CryptoHash" |
        "DateTime<Utc>" | "::chrono::DateTime<::chrono::Utc>" | "chrono::DateTime<chrono::Utc>" |
        "NaiveDateTime" | "::chrono::NaiveDateTime" | "chrono::NaiveDateTime" |
        "NaiveDate" | "::chrono::NaiveDate" | "chrono::NaiveDate" |
        "NaiveTime" | "::chrono::NaiveTime" | "chrono::NaiveTime"
    ) || type_str.starts_with("Json<") || type_str.starts_with("::sqlx::types::Json<") || type_str.starts_with("sqlx::types::Json<")
}

// Helper to map Rust types to SQL types for PostgreSQL
fn map_rust_type_to_sql(ty: &Type, _is_pk: bool, processing_array_inner: bool) -> String {
    let type_str = get_fully_qualified_type_string(ty);

    if type_str == "Vec<u8>" {
        return "BYTEA".to_string();
    }
    if type_str == "Vec<::voda_common::CryptoHash>" || type_str == "Vec<voda_common::CryptoHash>" || type_str == "Vec<CryptoHash>" {
        return "BYTEA[]".to_string();
    }

    if !processing_array_inner {
        if let Some(inner_ty) = get_vec_inner_type(ty) {
            let inner_type_sql = map_rust_type_to_sql(&inner_ty, false, true);
            if inner_type_sql.ends_with("[]") && !(get_fully_qualified_type_string(&inner_ty).contains("CryptoHash")) {
                panic!("Multi-dimensional arrays (Vec<Vec<T>>) are not currently supported for SQL mapping beyond Vec<CryptoHash>.");
            }
            if inner_type_sql == "JSONB" || (inner_type_sql == "BYTEA" && !get_fully_qualified_type_string(&inner_ty).contains("CryptoHash") ) {
                 panic!("Vec<{}> mapped to SQL type {} cannot be directly made into an SQL array. Consider Json<Vec<{}>> or a different structure.", quote!(#inner_ty), inner_type_sql, quote!(#inner_ty));
            }
            // If inner_ty is TEXT-mappable enum, map_rust_type_to_sql will return TEXT for it.
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
        "CryptoHash" | "::voda_common::CryptoHash" | "voda_common::CryptoHash" => "BYTEA".to_string(),
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

fn has_sqlx_skip_column_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("sqlx_skip_column"))
}

#[proc_macro_derive(SqlxObject, attributes(table_name, foreign_key, foreign_key_many, sqlx_skip_column))]
pub fn sqlx_object_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;
    let row_struct_name = format_ident!("{}RowSqlx", struct_name);

    let mut custom_table_name: Option<String> = None;
    for attr in &input_ast.attrs {
        if attr.path.is_ident("table_name") {
            match attr.parse_meta() {
                Ok(Meta::NameValue(mnv)) => {
                    if let Lit::Str(lit_str) = &mnv.lit { custom_table_name = Some(lit_str.value()); } 
                    else { return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal for Meta::NameValue"); }); }
                }
                _ => return TokenStream::from(quote! { compile_error!("table_name attribute must be in the format #[table_name = \"my_table\"]"); }),
            }
        }
    }
    let table_name_str = custom_table_name.unwrap_or_else(|| format!("{}s", struct_name.to_string().to_lowercase()));

    let all_fields_in_struct = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => &fields_named.named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] is only supported for structs with named fields."); }),
    };

    let active_fields_for_sql: Vec<&Field> = all_fields_in_struct.iter()
        .filter(|field| !has_sqlx_skip_column_attr(field))
        .collect();
    
    if active_fields_for_sql.is_empty() {
        return TokenStream::from(quote! { compile_error!("After skipping #[sqlx_skip_column] fields, no fields remain for SQL mapping."); });
    }

    match active_fields_for_sql.iter().find(|f| f.ident.as_ref().map_or(false, |i| i == "id")) {
        Some(field) => {
            if !matches!(field.vis, syn::Visibility::Public(_)) {
                return syn::Error::new_spanned(field.ident.as_ref().unwrap(), "#[derive(SqlxObject)] requires the 'id' field to be public and not skipped.").to_compile_error().into();
            }
            let type_str = get_fully_qualified_type_string(&field.ty);
            if type_str != "CryptoHash" && type_str != "::voda_common::CryptoHash" && type_str != "voda_common::CryptoHash" {
                 return syn::Error::new_spanned(&field.ty, format!("#[derive(SqlxObject)] requires the 'id' field to be of type 'CryptoHash' or '::voda_common::CryptoHash', found '{}'.", type_str)).to_compile_error().into();
            }
        }
        None => {
            return syn::Error::new_spanned(struct_name, "#[derive(SqlxObject)] requires a public field 'id: CryptoHash' that is not marked with #[sqlx_skip_column].").to_compile_error().into();
        }
    }

    let row_struct_fields_defs: Vec<proc_macro2::TokenStream> = active_fields_for_sql.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let fq_field_type_str = get_fully_qualified_type_string(field_ty);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        let is_original_leaf_crypto_hash = fq_type_str_for_analysis.contains("CryptoHash") && !is_json_type_for_analysis;
        let field_is_option = is_option_type(field_ty);
        
        let mut row_field_ty = field_ty.clone();

        if is_original_leaf_crypto_hash {
            let ch_vec_u8_type: Type = parse_quote!(Vec<u8>);
            row_field_ty = if field_is_option { parse_quote!(Option<#ch_vec_u8_type>) } else { ch_vec_u8_type };
        } else if fq_field_type_str == "Vec<CryptoHash>" || fq_field_type_str == "Vec<::voda_common::CryptoHash>" || fq_field_type_str == "Vec<voda_common::CryptoHash>" {
            row_field_ty = parse_quote!(Vec<Vec<u8>>);
        } else if !is_simple_type(&type_for_analysis) && !is_json_type_for_analysis && !fq_type_str_for_analysis.starts_with("Option<") && !fq_type_str_for_analysis.starts_with("Vec<") {
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
        let fq_field_type_str = get_fully_qualified_type_string(field_ty);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        let is_original_leaf_crypto_hash = fq_type_str_for_analysis.contains("CryptoHash") && !is_json_type_for_analysis;
        let field_is_option = is_option_type(field_ty);
        let row_field_name = field_ident;

        if is_original_leaf_crypto_hash {
            if field_is_option {
                quote! { #field_ident: row.#row_field_name.map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect("Failed to convert Option<Vec<u8>> to [u8;32] for CryptoHash"))) }
            } else {
                quote! { #field_ident: ::voda_common::CryptoHash::new(row.#row_field_name.try_into().expect("Failed to convert Vec<u8> to [u8;32] for CryptoHash")) }
            }
        } else if fq_field_type_str == "Vec<CryptoHash>" || fq_field_type_str == "Vec<::voda_common::CryptoHash>" || fq_field_type_str == "Vec<voda_common::CryptoHash>" {
            quote! { #field_ident: row.#row_field_name.into_iter().map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect("Failed to convert Vec<u8> to [u8;32] for CryptoHash"))).collect() }
        } else if !is_simple_type(&type_for_analysis) && !is_json_type_for_analysis && !fq_type_str_for_analysis.starts_with("Option<") && !fq_type_str_for_analysis.starts_with("Vec<") {
            if field_is_option {
                 quote! { #field_ident: row.#row_field_name.map(|s| s.parse().unwrap_or_else(|_| <#type_for_analysis>::default())) }
            } else {
                 quote! { #field_ident: row.#row_field_name.parse().unwrap_or_else(|_| <#type_for_analysis>::default()) }
            }
        } else if let Some(vec_inner_ty) = get_vec_inner_type(field_ty) { 
            let is_inner_text_mappable_enum = !is_simple_type(&vec_inner_ty) && 
                                              !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Option<") && 
                                              !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Vec<");
            if is_inner_text_mappable_enum {
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

    for field in active_fields_for_sql.iter() {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let sql_column_name = field_ident.to_string();
        
        all_sql_column_names_str_lits.push(LitStr::new(&sql_column_name, proc_macro2::Span::call_site()));
        insert_col_sql_names.push(format!("\"{}\"", sql_column_name));

        let field_is_option = is_option_type(field_ty);
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone()); // This is the Option-unwrapped type or original type.
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let fq_field_type_str = get_fully_qualified_type_string(field_ty); // This is the original field type (could be Option<T> or Vec<T> or T)

        // Determine if type_for_analysis is a direct TEXT-mappable type (e.g., an enum like UserRole)
        let is_direct_text_mappable = 
            !is_simple_type(&type_for_analysis) &&
            !get_fully_qualified_type_string(&type_for_analysis).starts_with("Vec<");
            // No Option check as type_for_analysis is already unwrapped.
            // No Json check as !is_simple_type implies not Json.

        // Validation Logic
        let is_valid_type = if is_simple_type(&type_for_analysis) {
            true // Simple types (primitives, Json<T>, Uuid, DateTime, CryptoHash) are valid for type_for_analysis
        } else if is_direct_text_mappable {
            true // Standalone enums/custom types mappable to TEXT are valid for type_for_analysis
        } else if fq_field_type_str.contains("Vec<CryptoHash>") || fq_field_type_str.contains("Vec<::voda_common::CryptoHash>") {
            true // Vec<CryptoHash> is valid for the original field_ty
        } else if let Some(vec_inner_ty) = get_vec_inner_type(field_ty) {
            // For Vec<T>, check T (which is vec_inner_ty)
            is_simple_type(&vec_inner_ty) || // T is simple (includes Json<U>)
            ( // OR T is a TEXT-mappable enum
                !is_simple_type(&vec_inner_ty) &&
                !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Option<") &&
                !get_fully_qualified_type_string(&vec_inner_ty).starts_with("Vec<")
            )
        } else {
            false // Type is complex and not covered by above rules
        };

        if !is_valid_type {
            return syn::Error::new_spanned(field_ty, 
                format!("Field '{}': Type '{}' (analyzed as '{}') is not automatically mappable to SQL. Use simple types, Option<Simple>, Json<T>, TEXT-mappable enums, Vec<Simple>, or Vec<TEXT-mappable Enum>.",
                        field_ident, fq_field_type_str, fq_type_str_for_analysis)
            ).to_compile_error().into();
        }
        
        let actual_type_for_sql_map = if field_is_option { type_for_analysis.clone() } else { field_ty.clone() };
        let sql_type_str = map_rust_type_to_sql(&actual_type_for_sql_map, sql_column_name == "id", false);
        
        let mut col_def_parts = vec![format!("\"{}\"", sql_column_name), sql_type_str.clone()];
        let is_pk = sql_column_name == "id";
        if is_pk { col_def_parts.push("PRIMARY KEY".to_string()); }
        else if !field_is_option { col_def_parts.push("NOT NULL".to_string()); }
        create_table_column_defs.push(col_def_parts.join(" "));

        if let Some(fk_info) = parse_foreign_key_attr(field) {
            foreign_key_clauses_for_create_table.push(format!(
                "FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"id\") ON DELETE SET NULL ON UPDATE CASCADE",
                sql_column_name, fk_info.referenced_table
            ));
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_info.related_rust_type;
            let self_field_access = quote!{ self.#field_ident };
            if field_is_option {
                fetch_helper_methods.push(quote! {
                    pub async fn #fetch_method_name(&self, pool: &sqlx::PgPool) -> Result<Option<#related_type>, sqlx::Error> {
                        if let Some(id_value) = &#self_field_access {
                            ::voda_database::sqlx_postgres::SqlxCrud::find_by_id(id_value.hash().to_vec(), pool).await
                        } else { Ok(None) }
                    }
                });
            } else {
                fetch_helper_methods.push(quote! {
                    pub async fn #fetch_method_name(&self, pool: &sqlx::PgPool) -> Result<Option<#related_type>, sqlx::Error> {
                        ::voda_database::sqlx_postgres::SqlxCrud::find_by_id(#self_field_access.hash().to_vec(), pool).await
                    }
                });
            }
        } 
        else if let Some(fk_many_info) = parse_foreign_key_many_attr(field) {
            if !(fq_field_type_str.contains("Vec<CryptoHash>") || fq_field_type_str.contains("Vec<::voda_common::CryptoHash>")) {
                return syn::Error::new_spanned(field_ty, "foreign_key_many attribute can only be used on fields of type Vec<CryptoHash> or Vec<::voda_common::CryptoHash>.").to_compile_error().into();
            }
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_many_info.related_rust_type;
            let referenced_table_str = &fk_many_info.referenced_table;
            
            fetch_helper_methods.push(quote! {
                pub async fn #fetch_method_name(&self, pool: &sqlx::PgPool) -> Result<Vec<#related_type>, sqlx::Error> {
                    if self.#field_ident.is_empty() {
                        return Ok(Vec::new());
                    }
                    let ids_as_vec_u8: Vec<Vec<u8>> = self.#field_ident.iter().map(|ch| ch.hash().to_vec()).collect();
                    let sql = format!("SELECT * FROM \"{}\" WHERE \"id\" = ANY($1)", #referenced_table_str);
                    
                    let related_rows = sqlx::query_as::<_, <#related_type as ::voda_database::sqlx_postgres::SqlxSchema>::Row>(&sql)
                        .bind(ids_as_vec_u8)
                        .fetch_all(pool)
                        .await?;
                    
                    Ok(related_rows.into_iter().map(<#related_type as ::voda_database::sqlx_postgres::SqlxSchema>::from_row).collect())
                }
            });
        }
        let field_access_path = quote!{ self.#field_ident };
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        let is_original_leaf_crypto_hash = fq_type_str_for_analysis.contains("CryptoHash") && !is_json_type_for_analysis;
        let is_vec_crypto_hash = fq_field_type_str.contains("Vec<CryptoHash>") || fq_field_type_str.contains("Vec<::voda_common::CryptoHash>");
        let is_vec_text_mappable_enum = get_vec_inner_type(field_ty).map_or(false, |vt| 
            !is_simple_type(&vt) && 
            !get_fully_qualified_type_string(&vt).starts_with("Option<") && 
            !get_fully_qualified_type_string(&vt).starts_with("Vec<")
        );

        let insert_bind = if is_original_leaf_crypto_hash {
            if field_is_option {
                quote! { .bind(#field_access_path.as_ref().map(|ch| ch.hash().to_vec())) }
            } else {
                quote! { .bind(#field_access_path.hash().to_vec()) }
            }
        } else if is_vec_crypto_hash { 
             quote! { .bind(#field_access_path.iter().map(|ch| ch.hash().to_vec()).collect::<Vec<Vec<u8>>>()) }
        } else if is_direct_text_mappable { 
            if field_is_option {
                 quote! { .bind(#field_access_path.as_ref().map(|v| v.to_string())) }
            } else {
                 quote! { .bind(#field_access_path.to_string()) }
            }
        } else if is_vec_text_mappable_enum { 
            quote! { .bind(#field_access_path.iter().map(|v| v.to_string()).collect::<Vec<String>>()) }
        }
        else { quote! { .bind(#field_access_path.clone()) } }; 
        insert_bindings_streams.push(insert_bind);

        if !is_pk {
            update_set_clauses_sql.push(format!("\"{}\" = ${}", sql_column_name, update_placeholder_idx));
            update_placeholder_idx += 1;
            let update_bind = if is_original_leaf_crypto_hash {
                if field_is_option {
                    quote! { .bind(#field_access_path.as_ref().map(|ch| ch.hash().to_vec())) }
                } else {
                    quote! { .bind(#field_access_path.hash().to_vec()) }
                }
            } else if is_vec_crypto_hash {
                 quote! { .bind(#field_access_path.iter().map(|ch| ch.hash().to_vec()).collect::<Vec<Vec<u8>>>()) }
            } else if is_direct_text_mappable { 
                if field_is_option {
                    quote! { .bind(#field_access_path.as_ref().map(|v| v.to_string())) }
                } else {
                    quote! { .bind(#field_access_path.to_string()) }
                }
            } else if is_vec_text_mappable_enum { 
                 quote! { .bind(#field_access_path.iter().map(|v| v.to_string()).collect::<Vec<String>>()) }
            }
            else { quote! { .bind(#field_access_path.clone()) } };
            update_bindings_streams.push(update_bind);
        }
    }

    let mut create_table_parts = create_table_column_defs;
    if !foreign_key_clauses_for_create_table.is_empty() {
        create_table_parts.extend(foreign_key_clauses_for_create_table);
    }
    let create_table_sql_query = format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name_str, create_table_parts.join(", "));
    let drop_table_sql_query = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table_name_str);

    let all_sql_columns_joined_str = all_sql_column_names_str_lits.iter().map(|s| format!("\"{}\"", s.value())).collect::<Vec<String>>().join(", ");
    let select_all_sql_query = format!("SELECT {} FROM \"{}\"", all_sql_columns_joined_str, table_name_str);
    let select_by_id_sql_query = format!("SELECT {} FROM \"{}\" WHERE \"id\" = $1", all_sql_columns_joined_str, table_name_str);
    let delete_by_id_sql_query = format!("DELETE FROM \"{}\" WHERE \"id\" = $1", table_name_str);
    
    let insert_column_names_joined_sql = insert_col_sql_names.join(", ");
    let insert_bind_placeholders_sql = (1..=insert_col_sql_names.len()).map(|i| format!("${}", i)).collect::<Vec<String>>().join(", ");
    let insert_sql_query = format!("INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}", table_name_str, insert_column_names_joined_sql, insert_bind_placeholders_sql, all_sql_columns_joined_str);

    let pk_binding_for_update = quote! { .bind(self.id.hash().to_vec()) };
    let update_set_str_sql = update_set_clauses_sql.join(", ");
    
    let update_by_id_sql_query_is_select = update_set_clauses_sql.is_empty();
    let update_by_id_sql_query = if update_by_id_sql_query_is_select {
        format!("SELECT {} FROM \"{}\" WHERE \"id\" = $1", all_sql_columns_joined_str, table_name_str) 
    } else {
        format!("UPDATE \"{}\" SET {} WHERE \"id\" = ${} RETURNING {}", table_name_str, update_set_str_sql, update_placeholder_idx, all_sql_columns_joined_str)
    };
    
    let final_pk_id_trait_type: Type = parse_quote!(Vec<u8>);
    let get_id_value_impl = quote! { self.id.hash().to_vec() };
    
    let expanded = quote! {
        #[derive(::sqlx::FromRow, Debug, Clone)]
        #[automatically_derived]
        pub struct #row_struct_name {
            #(#row_struct_fields_defs),*
        }

        #[automatically_derived]
        impl ::voda_database::sqlx_postgres::SqlxSchema for #struct_name {
            type Id = #final_pk_id_trait_type;
            type Row = #row_struct_name;

            const TABLE_NAME: &'static str = #table_name_str;
            const ID_COLUMN_NAME: &'static str = "id";
            const COLUMNS: &'static [&'static str] = &[#( #all_sql_column_names_str_lits ),*];

            fn get_id_value(&self) -> Self::Id { #get_id_value_impl }

            fn from_row(row: Self::Row) -> Self {
                Self {
                    #(#all_from_row_assignments),*
                }
            }

            fn select_all_sql() -> String { #select_all_sql_query.to_string() }
            fn select_by_id_sql() -> String { #select_by_id_sql_query.to_string() }
            fn insert_sql() -> String { #insert_sql_query.to_string() }
            fn update_by_id_sql() -> String { #update_by_id_sql_query.to_string() }
            fn delete_by_id_sql() -> String { #delete_by_id_sql_query.to_string() }
            fn create_table_sql() -> String { #create_table_sql_query.to_string() }
            fn drop_table_sql() -> String { #drop_table_sql_query.to_string() }
        }

        #[automatically_derived]
        #[::async_trait::async_trait]
        impl ::voda_database::sqlx_postgres::SqlxCrud for #struct_name {
            fn bind_insert<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                query #(#insert_bindings_streams)*
            }

            fn bind_update<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                if #update_by_id_sql_query_is_select {
                    query #pk_binding_for_update
                } else {
                    query #(#update_bindings_streams)* #pk_binding_for_update
                }
            }

            async fn find_by_id<'e, A>(id: Self::Id, acquirer: A) -> Result<Option<Self>, ::sqlx::Error>
            where
                A: ::sqlx::Acquire<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let mut conn = acquirer.acquire().await?;
                let sql = <Self as ::voda_database::sqlx_postgres::SqlxSchema>::select_by_id_sql();
                ::sqlx::query_as::<_, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row>(&sql)
                    .bind(id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map(|opt_row| opt_row.map(<Self as ::voda_database::sqlx_postgres::SqlxSchema>::from_row))
            }

            async fn find_all<'e, A>(acquirer: A) -> Result<Vec<Self>, ::sqlx::Error>
            where
                A: ::sqlx::Acquire<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let mut conn = acquirer.acquire().await?;
                let sql = <Self as ::voda_database::sqlx_postgres::SqlxSchema>::select_all_sql();
                ::sqlx::query_as::<_, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row>(&sql)
                    .fetch_all(&mut *conn)
                    .await
                    .map(|rows| rows.into_iter().map(<Self as ::voda_database::sqlx_postgres::SqlxSchema>::from_row).collect())
            }

            async fn create<'e, A>(mut self, acquirer: A) -> Result<Self, ::sqlx::Error>
            where
                A: ::sqlx::Acquire<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                self.sql_populate_id();
                let mut conn = acquirer.acquire().await?;
                let sql = <Self as ::voda_database::sqlx_postgres::SqlxSchema>::insert_sql();
                self.bind_insert(::sqlx::query_as::<_, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row>(&sql))
                    .fetch_one(&mut *conn)
                    .await
                    .map(<Self as ::voda_database::sqlx_postgres::SqlxSchema>::from_row)
            }

            async fn update<'e, A>(self, acquirer: A) -> Result<Self, ::sqlx::Error>
            where
                A: ::sqlx::Acquire<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let mut conn = acquirer.acquire().await?;
                let sql = <Self as ::voda_database::sqlx_postgres::SqlxSchema>::update_by_id_sql();
                self.bind_update(::sqlx::query_as::<_, <Self as ::voda_database::sqlx_postgres::SqlxSchema>::Row>(&sql))
                    .fetch_one(&mut *conn)
                    .await
                    .map(<Self as ::voda_database::sqlx_postgres::SqlxSchema>::from_row)
            }

            async fn delete<'e, A>(self, acquirer: A) -> Result<u64, ::sqlx::Error>
            where
                A: ::sqlx::Acquire<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let mut conn = acquirer.acquire().await?;
                let sql = <Self as ::voda_database::sqlx_postgres::SqlxSchema>::delete_by_id_sql();
                ::sqlx::query(&sql)
                    .bind(<Self as ::voda_database::sqlx_postgres::SqlxSchema>::get_id_value(&self))
                    .execute(&mut *conn)
                    .await
                    .map(|done| done.rows_affected())
            }
        }

        #[automatically_derived]
        impl #struct_name {
            #(#fetch_helper_methods)*
        }
    };

    TokenStream::from(expanded)
}
