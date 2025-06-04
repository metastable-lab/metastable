use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident, Lit,
    LitStr, Meta, Type, Expr, GenericArgument, PathArguments, parse_quote, Field
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
    // Check for Vec<CryptoHash> explicitly for array mapping if needed, but current logic is for SQL types
    if type_str == "Vec<::voda_common::CryptoHash>" || type_str == "Vec<voda_common::CryptoHash>" || type_str == "Vec<CryptoHash>" {
        return "BYTEA[]".to_string(); // PostgreSQL array of BYTEA
    }

    if !processing_array_inner {
        if let Some(inner_ty) = get_vec_inner_type(ty) {
            let inner_type_sql = map_rust_type_to_sql(&inner_ty, false, true);
            if inner_type_sql.ends_with("[]") {
                panic!("Multi-dimensional arrays (Vec<Vec<T>>) are not currently supported for SQL mapping beyond Vec<CryptoHash>.");
            }
            if inner_type_sql == "JSONB" || (inner_type_sql == "BYTEA" && get_fully_qualified_type_string(&inner_ty) != "::voda_common::CryptoHash" && get_fully_qualified_type_string(&inner_ty) != "voda_common::CryptoHash" && get_fully_qualified_type_string(&inner_ty) != "CryptoHash" ) {
                 panic!("Vec<{}> mapped to SQL type {} cannot be directly made into an SQL array. Consider Json<Vec<{}>> or a different structure.", quote!(#inner_ty), inner_type_sql, quote!(#inner_ty));
            }
            return format!("{}[]", inner_type_sql);
        }
    }

    match type_str.as_str() {
        "String" | "std::string::String" => "TEXT".to_string(),
        "i32" => "INTEGER".to_string(),
        "u32" => "BIGINT".to_string(), // Or INTEGER if u32 fits, but BIGINT is safer for PG
        "i64" | "isize" => "BIGINT".to_string(),
        "u64" | "usize" => "BIGINT".to_string(), // No direct u64 in SQL, map to BIGINT and rely on Rust's range or custom types if needed
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
        _ => {
            panic!("Unsupported Rust type for SQL mapping: {} Please use a supported type or specify SQL type via an attribute if complex, or ensure it's a struct that can be flattened.", type_str)
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

#[proc_macro_derive(SqlxObject, attributes(table_name, foreign_key, foreign_key_many))]
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

    let top_level_fields = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => &fields_named.named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] is only supported for structs with named fields."); }),
    };

    // ID field validation (must be public id: CryptoHash)
    match top_level_fields.iter().find(|f| f.ident.as_ref().map_or(false, |i| i == "id")) {
        Some(field) => {
            if !matches!(field.vis, syn::Visibility::Public(_)) {
                return syn::Error::new_spanned(field.ident.as_ref().unwrap(), "#[derive(SqlxObject)] requires the 'id' field to be public.").to_compile_error().into();
            }
            let type_str = get_fully_qualified_type_string(&field.ty);
            if type_str != "CryptoHash" && type_str != "::voda_common::CryptoHash" && type_str != "voda_common::CryptoHash" {
                 return syn::Error::new_spanned(&field.ty, format!("#[derive(SqlxObject)] requires the 'id' field to be of type 'CryptoHash' or '::voda_common::CryptoHash', found '{}'.", type_str)).to_compile_error().into();
            }
        }
        None => {
            return syn::Error::new_spanned(struct_name, "#[derive(SqlxObject)] requires a public field 'id: CryptoHash'.").to_compile_error().into();
        }
    }
    if top_level_fields.is_empty() { return TokenStream::from(quote! { compile_error!("SqlxObject cannot be derived for a struct with no fields."); }); }

    let row_struct_fields_defs: Vec<proc_macro2::TokenStream> = top_level_fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let is_json_type = get_fully_qualified_type_string(&type_for_analysis).starts_with("Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("::sqlx::types::Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("sqlx::types::Json<");
        let is_original_leaf_crypto_hash = get_fully_qualified_type_string(&type_for_analysis).contains("CryptoHash") && !is_json_type;
        let field_is_option = is_option_type(field_ty);
        let row_field_type = if is_original_leaf_crypto_hash {
            let ch_vec_u8_type: Type = parse_quote!(Vec<u8>);
            if field_is_option { parse_quote!(Option<#ch_vec_u8_type>) } else { ch_vec_u8_type }
        } else if get_fully_qualified_type_string(field_ty) == "Vec<CryptoHash>" || get_fully_qualified_type_string(field_ty) == "Vec<::voda_common::CryptoHash>" || get_fully_qualified_type_string(field_ty) == "Vec<voda_common::CryptoHash>" {
             parse_quote!(Vec<Vec<u8>>)
        }
        else { field_ty.clone() };
        quote! { pub #field_ident: #row_field_type }
    }).collect();

    let from_row_assignments: Vec<proc_macro2::TokenStream> = top_level_fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let is_json_type = get_fully_qualified_type_string(&type_for_analysis).starts_with("Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("::sqlx::types::Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("sqlx::types::Json<");
        let is_original_leaf_crypto_hash = get_fully_qualified_type_string(&type_for_analysis).contains("CryptoHash") && !is_json_type;
        let field_is_option = is_option_type(field_ty);
        let row_field_name = field_ident;

        if is_original_leaf_crypto_hash {
            if field_is_option {
                quote! { #field_ident: row.#row_field_name.map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect("Failed to convert Option<Vec<u8>> to [u8;32] for CryptoHash"))) }
            } else {
                quote! { #field_ident: ::voda_common::CryptoHash::new(row.#row_field_name.try_into().expect("Failed to convert Vec<u8> to [u8;32] for CryptoHash")) }
            }
        } else if get_fully_qualified_type_string(field_ty) == "Vec<CryptoHash>" || get_fully_qualified_type_string(field_ty) == "Vec<::voda_common::CryptoHash>" || get_fully_qualified_type_string(field_ty) == "Vec<voda_common::CryptoHash>" {
            quote! { #field_ident: row.#row_field_name.into_iter().map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect("Failed to convert Vec<u8> to [u8;32] for CryptoHash"))).collect() }
        }
        else { quote! { #field_ident: row.#row_field_name } }
    }).collect::<Vec<_>>();

    let mut all_sql_column_names_str_lits: Vec<LitStr> = Vec::new();
    let mut create_table_column_defs: Vec<String> = Vec::new();
    let mut foreign_key_clauses_for_create_table: Vec<String> = Vec::new();
    let mut insert_col_sql_names: Vec<String> = Vec::new();
    let mut insert_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_set_clauses_sql: Vec<String> = Vec::new();
    let mut update_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_placeholder_idx = 1;
    let mut fetch_helper_methods: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in top_level_fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let sql_column_name = field_ident.to_string();
        
        all_sql_column_names_str_lits.push(LitStr::new(&sql_column_name, proc_macro2::Span::call_site()));
        insert_col_sql_names.push(format!("\"{}\"", sql_column_name));

        let field_is_option = is_option_type(field_ty);
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let field_is_vec_of_something = get_vec_inner_type(&type_for_analysis).is_some() || get_vec_inner_type(field_ty).is_some();
        let is_json_type = get_fully_qualified_type_string(&type_for_analysis).starts_with("Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("::sqlx::types::Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("sqlx::types::Json<");
        let actual_type_for_sql_map = if field_is_option { get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone()) } else { field_ty.clone() };

        if !is_simple_type(&type_for_analysis) && 
           !is_json_type && 
           !(field_is_vec_of_something && 
             (get_vec_inner_type(&actual_type_for_sql_map).map_or(false, |vt| is_simple_type(&vt) || get_fully_qualified_type_string(&vt).contains("CryptoHash")))
            )
        {
             let type_str_check = get_fully_qualified_type_string(&actual_type_for_sql_map);
             if !(type_str_check.contains("Vec<CryptoHash>") || type_str_check.contains("Vec<::voda_common::CryptoHash>") || type_str_check.contains("Vec<voda_common::CryptoHash>")) {
                return syn::Error::new_spanned(field_ty,
                    format!("Field '{}': Type '{}' is complex. Wrap with Json<T>, use simple types, Option<Simple>, Vec<Simple>, or Vec<CryptoHash>.",
                            field_ident, quote!(#type_for_analysis))
                ).to_compile_error().into();
             }
        }
        
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
            let field_type_str = get_fully_qualified_type_string(field_ty);
            if !(field_type_str == "Vec<CryptoHash>" || field_type_str == "Vec<::voda_common::CryptoHash>" || field_type_str == "Vec<voda_common::CryptoHash>") {
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
        let is_original_leaf_crypto_hash = get_fully_qualified_type_string(&type_for_analysis).contains("CryptoHash") && !is_json_type;
        let is_vec_crypto_hash = get_fully_qualified_type_string(field_ty).contains("Vec<CryptoHash>");

        let insert_bind = if is_original_leaf_crypto_hash && !field_is_vec_of_something { 
            if field_is_option {
                quote! { .bind(#field_access_path.as_ref().map(|ch| ch.hash().to_vec())) }
            } else {
                quote! { .bind(#field_access_path.hash().to_vec()) }
            }
        } else if is_vec_crypto_hash { 
             quote! { .bind(#field_access_path.iter().map(|ch| ch.hash().to_vec()).collect::<Vec<Vec<u8>>>()) }
        }
        else { quote! { .bind(#field_access_path.clone()) } };
        insert_bindings_streams.push(insert_bind);

        if !is_pk {
            update_set_clauses_sql.push(format!("\"{}\" = ${}", sql_column_name, update_placeholder_idx));
            update_placeholder_idx += 1;
            let update_bind = if is_original_leaf_crypto_hash && !field_is_vec_of_something {
                if field_is_option {
                    quote! { .bind(#field_access_path.as_ref().map(|ch| ch.hash().to_vec())) }
                } else {
                    quote! { .bind(#field_access_path.hash().to_vec()) }
                }
            } else if is_vec_crypto_hash {
                 quote! { .bind(#field_access_path.iter().map(|ch| ch.hash().to_vec()).collect::<Vec<Vec<u8>>>()) }
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
    let update_by_id_sql_query = format!("UPDATE \"{}\" SET {} WHERE \"id\" = ${} RETURNING {}", table_name_str, update_set_str_sql, update_placeholder_idx, all_sql_columns_joined_str);
    
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
                    #(#from_row_assignments),*
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
            fn bind_insert<'q>(&self, query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments>)
                -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments> {
                query #(#insert_bindings_streams)*
            }

            fn bind_update<'q>(&self, query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments>)
                -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments> {
                query #(#update_bindings_streams)* #pk_binding_for_update
            }
        }

        #[automatically_derived]
        impl #struct_name {
            #(#fetch_helper_methods)*
        }
    };

    TokenStream::from(expanded)
}
