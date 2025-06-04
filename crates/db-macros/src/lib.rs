use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, Lit,
    LitStr, Meta, Type, Expr, GenericArgument, PathArguments, parse_quote
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

    if !processing_array_inner {
        if let Some(inner_ty) = get_vec_inner_type(ty) {
            let inner_type_sql = map_rust_type_to_sql(&inner_ty, false, true);
            if inner_type_sql.ends_with("[]") {
                panic!("Multi-dimensional arrays (Vec<Vec<T>>) are not currently supported for SQL mapping.");
            }
            if inner_type_sql == "JSONB" || inner_type_sql == "BYTEA" {
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
        "CryptoHash" | "::voda_common::CryptoHash" | "voda_common::CryptoHash" => "BYTEA".to_string(),
        s if s.starts_with("Json<") || s.starts_with("::sqlx::types::Json<") || s.starts_with("sqlx::types::Json<") => "JSONB".to_string(),
        "DateTime<Utc>" | "::chrono::DateTime<::chrono::Utc>" | "chrono::DateTime<chrono::Utc>" => "TIMESTAMPTZ".to_string(),
        "NaiveDateTime" | "::chrono::NaiveDateTime" | "chrono::NaiveDateTime" => "TIMESTAMP".to_string(),
        "NaiveDate" | "::chrono::NaiveDate" | "chrono::NaiveDate" => "DATE".to_string(),
        "NaiveTime" | "::chrono::NaiveTime" | "chrono::NaiveTime" => "TIME".to_string(),
        _ => {
            panic!("Unsupported Rust type for SQL mapping: {} Please use a supported type or specify SQL type via #[column_type(...)] attribute, or ensure it's a struct that can be flattened.", type_str)
        }
    }
}

#[proc_macro_derive(SqlxObject, attributes(table_name))]
pub fn sqlx_object_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;
    let row_struct_name = format_ident!("{}RowSqlx", struct_name);

    let mut custom_table_name: Option<String> = None;

    for attr in &input_ast.attrs {
        if attr.path().is_ident("table_name") {
            match &attr.meta {
                Meta::NameValue(mnv) => {
                    if let Expr::Lit(expr_lit) = &mnv.value {
                        if let Lit::Str(lit_str) = &expr_lit.lit {
                            custom_table_name = Some(lit_str.value());
                        } else { return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal"); }); }
                    } else { return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal"); }); }
                }
                _ => return TokenStream::from(quote! { compile_error!("table_name attribute must be a name-value pair like #[table_name = \"my_table\"]"); }),
            }
        }
    }
    
    let table_name_str = custom_table_name.unwrap_or_else(|| format!("{}s", struct_name.to_string().to_lowercase()));

    let top_level_fields = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => &fields_named.named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] is only supported for structs with named fields."); }),
    };

    let id_field = top_level_fields.iter().find(|f| f.ident.as_ref().map_or(false, |i| i == "id"));
    match id_field {
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

    if top_level_fields.is_empty() {
        return TokenStream::from(quote! { compile_error!("SqlxObject cannot be derived for a struct with no fields."); });
    }

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
        } else {
            field_ty.clone()
        };
        quote! { pub #field_ident: #row_field_type }
    }).collect();

    let from_row_assignments: Vec<proc_macro2::TokenStream> = top_level_fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty; // Original field type

        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let is_json_type = get_fully_qualified_type_string(&type_for_analysis).starts_with("Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("::sqlx::types::Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("sqlx::types::Json<");

        let is_original_leaf_crypto_hash = get_fully_qualified_type_string(&type_for_analysis).contains("CryptoHash") && !is_json_type;
        let field_is_option = is_option_type(field_ty);
        
        let row_field_name = field_ident; // Row struct field names match original struct field names

        if is_original_leaf_crypto_hash {
            if field_is_option {
                quote! { #field_ident: row.#row_field_name.map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect("Failed to convert Option<Vec<u8>> to [u8;32] for CryptoHash"))) }
            } else {
                quote! { #field_ident: ::voda_common::CryptoHash::new(row.#row_field_name.try_into().expect("Failed to convert Vec<u8> to [u8;32] for CryptoHash")) }
            }
        } else {
             quote! { #field_ident: row.#row_field_name }
        }
    }).collect::<Vec<_>>();

    let mut all_sql_column_names_str_lits: Vec<LitStr> = Vec::new();
    let mut create_table_column_defs: Vec<String> = Vec::new();
    let mut insert_col_sql_names: Vec<String> = Vec::new();
    let mut insert_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_set_clauses_sql: Vec<String> = Vec::new();
    let mut update_bindings_streams: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut update_placeholder_idx = 1;

    for field in top_level_fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let sql_column_name = field_ident.to_string();
        
        all_sql_column_names_str_lits.push(LitStr::new(&sql_column_name, proc_macro2::Span::call_site()));
        insert_col_sql_names.push(format!("\"{}\"", sql_column_name));

        let field_is_option = is_option_type(field_ty);
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let field_is_vec = get_vec_inner_type(&type_for_analysis).is_some();
        let is_json_type = get_fully_qualified_type_string(&type_for_analysis).starts_with("Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("::sqlx::types::Json<") ||
                           get_fully_qualified_type_string(&type_for_analysis).starts_with("sqlx::types::Json<");

        if !is_simple_type(&type_for_analysis) && 
           !is_json_type && 
           !(field_is_vec && get_vec_inner_type(&type_for_analysis).map_or(false, |vt| is_simple_type(&vt) || get_fully_qualified_type_string(&vt) == "u8"))
        {
             return syn::Error::new_spanned(field_ty,
                format!("Field '{}': Type '{}' is a nested struct or unsupported Vec type. Please wrap complex types with Json<T> (e.g., Json<{}>) or ensure it is a simple type, Option<Simple>, or Vec<Simple> (like Vec<String> or Vec<i32>).",
                        field_ident, quote!(#type_for_analysis), quote!(#type_for_analysis))
            ).to_compile_error().into();
        }
        
        let type_for_sql_mapping = if is_json_type || field_is_vec {
            type_for_analysis.clone()
        } else if field_is_option { 
             get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone())
        } else {
             field_ty.clone()
        };
        let sql_type_str = map_rust_type_to_sql(&type_for_sql_mapping, sql_column_name == "id", false);

        let mut col_def_parts = vec![format!("\"{}\"", sql_column_name), sql_type_str.clone()];
        let is_pk = sql_column_name == "id";
        if is_pk { col_def_parts.push("PRIMARY KEY".to_string()); }
        else if !field_is_option { col_def_parts.push("NOT NULL".to_string()); }
        create_table_column_defs.push(col_def_parts.join(" "));

        let field_access_path = quote!{ self.#field_ident };
        let is_original_leaf_crypto_hash = get_fully_qualified_type_string(&type_for_analysis).contains("CryptoHash") && !is_json_type;

        let insert_bind = if is_original_leaf_crypto_hash {
            if field_is_option {
                quote! { .bind(#field_access_path.as_ref().map(|ch| ch.hash().to_vec())) }
            } else {
                quote! { .bind(#field_access_path.hash().to_vec()) }
            }
        } else { 
             quote! { .bind(#field_access_path.clone()) }
        };
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
            } else {
                quote! { .bind(#field_access_path.clone()) }
            };
            update_bindings_streams.push(update_bind);
        }
    }

    let all_sql_columns_joined_str = all_sql_column_names_str_lits.iter().map(|s| format!("\"{}\"", s.value())).collect::<Vec<String>>().join(", ");
    let create_table_sql_query = format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name_str, create_table_column_defs.join(", "));
    let drop_table_sql_query = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table_name_str);

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
    let populate_id_trait_method_impl = quote! { <Self as ::voda_database::sqlx_postgres::SqlxPopulateId>::sql_populate_id(self); };
    
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

            fn populate_id(&mut self) {
                #populate_id_trait_method_impl
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
    };

    TokenStream::from(expanded)
}
