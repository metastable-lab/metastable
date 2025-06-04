use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, Data, DataStruct, DeriveInput, Fields, Ident, Lit,
    LitStr, Meta, Type, Expr, Token, GenericArgument, PathArguments,
    punctuated::Punctuated, parse_quote
};

// Helper to check if a type is an Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 && type_path.path.segments[0].ident == "Option" {
            return true;
        }
    }
    false
}

// Helper to get the inner type from Option<T>
fn get_option_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 && type_path.path.segments[0].ident == "Option" {
            if let PathArguments::AngleBracketed(angle_args) = &type_path.path.segments[0].arguments {
                if angle_args.args.len() == 1 {
                    if let GenericArgument::Type(inner_ty) = &angle_args.args[0] {
                        return Some(inner_ty);
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

// Helper to map Rust types to SQL types for PostgreSQL
fn map_rust_type_to_sql(ty: &Type, is_pk: bool, auto_generated_pk: bool) -> String {
    let type_str = get_fully_qualified_type_string(ty);

    // Handle primary key types that might map to SERIAL/BIGSERIAL if auto-generated
    if is_pk && auto_generated_pk {
        match type_str.as_str() {
            "i32" => return "SERIAL".to_string(),
            "i64" => return "BIGSERIAL".to_string(),
            // Add other integer types if they can be auto-generated PKs
            _ => {} // Fall through for non-auto-generated or non-integer PKs
        }
    }

    match type_str.as_str() {
        "String" => "TEXT".to_string(),
        "i32" => "INTEGER".to_string(),
        "u32" => "BIGINT".to_string(), // Map u32 to BIGINT to avoid overflow, as PG INTEGER is signed.
        "i64" | "isize" => "BIGINT".to_string(),
        "u64" | "usize" => "DECIMAL".to_string(), // PostgreSQL doesn't have u64 directly. DECIMAL is a safe choice for large unsigned numbers.
        "f32" => "REAL".to_string(),
        "f64" => "DOUBLE PRECISION".to_string(),
        "bool" => "BOOLEAN".to_string(),
        "Vec<u8>" => "BYTEA".to_string(),
        "Uuid" | "::sqlx::types::Uuid" | "sqlx::types::Uuid" => "UUID".to_string(),
        "CryptoHash" | "::voda_common::CryptoHash" | "voda_common::CryptoHash" => "BYTEA".to_string(),
        s if s.starts_with("Json<") || s.starts_with("::sqlx::types::Json<") || s.starts_with("sqlx::types::Json<") => "JSONB".to_string(),
        "DateTime<Utc>" | "::chrono::DateTime<::chrono::Utc>" | "chrono::DateTime<chrono::Utc>" => "TIMESTAMPTZ".to_string(),
        "NaiveDateTime" | "::chrono::NaiveDateTime" | "chrono::NaiveDateTime" => "TIMESTAMP".to_string(),
        "NaiveDate" | "::chrono::NaiveDate" | "chrono::NaiveDate" => "DATE".to_string(),
        "NaiveTime" | "::chrono::NaiveTime" | "chrono::NaiveTime" => "TIME".to_string(),
        _ => panic!("Unsupported Rust type for SQL mapping: {} Please use a supported type or specify SQL type via #[column_type(...)] attribute.", type_str),
    }
}

fn meta_list_error(path: &syn::Path, message: &str) -> proc_macro2::TokenStream {
    let option_name = path.get_ident().map_or_else(|| "unknown_option".to_string(), |i| i.to_string());
    let full_message = format!("Invalid primary_key option '{}': {}", option_name, message);
    quote! { compile_error!(#full_message); }
}

#[proc_macro_derive(SqlxObject, attributes(table_name, primary_key, column_type))]
pub fn sqlx_object_derive(input: TokenStream) -> TokenStream {
    let input_ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &input_ast.ident;
    let row_struct_name = format_ident!("{}RowSqlx", struct_name);

    let mut custom_table_name: Option<String> = None;
    let mut pk_name_str: String = "id".to_string();
    let mut pk_auto_generated: bool = true; 
    let mut pk_ty_override_str: Option<String> = None;

    for attr in &input_ast.attrs {
        if attr.path().is_ident("table_name") {
            match &attr.meta {
                Meta::NameValue(mnv) => {
                    if let Expr::Lit(expr_lit) = &mnv.value {
                        if let Lit::Str(lit_str) = &expr_lit.lit {
                            custom_table_name = Some(lit_str.value());
                        } else {
                            return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal"); });
                        }
                    } else {
                         return TokenStream::from(quote! { compile_error!("table_name attribute value must be a string literal"); });
                    }
                }
                _ => return TokenStream::from(quote! { compile_error!("table_name attribute must be a name-value pair like #[table_name = \"my_table\"]"); }),
            }
        } else if attr.path().is_ident("primary_key") {
            match &attr.meta {
                Meta::List(meta_list) => {
                    match meta_list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
                        Ok(nested) => {
                            for meta_item in nested {
                                match meta_item {
                                    Meta::NameValue(nv) => {
                                        if nv.path.is_ident("name") {
                                            if let Expr::Lit(expr_lit) = &nv.value {
                                                if let Lit::Str(lit_str) = &expr_lit.lit {
                                                    pk_name_str = lit_str.value();
                                                } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'name' must be a string")); }
                                            } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'name' must be a string")); }
                                        } else if nv.path.is_ident("auto_generated") {
                                            if let Expr::Lit(expr_lit) = &nv.value {
                                                if let Lit::Bool(lit_bool) = &expr_lit.lit {
                                                    pk_auto_generated = lit_bool.value();
                                                } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'auto_generated' must be a boolean")); }
                                            } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'auto_generated' must be a boolean")); }
                                        } else if nv.path.is_ident("ty") {
                                            if let Expr::Lit(expr_lit) = &nv.value {
                                                if let Lit::Str(lit_str) = &expr_lit.lit {
                                                    pk_ty_override_str = Some(lit_str.value());
                                                } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'ty' must be a string representation of a type")); }
                                            } else { return TokenStream::from(meta_list_error( &nv.path, "primary_key option 'ty' must be a string representation of a type")); }
                                        } else {
                                            let msg = format!("Unknown primary_key option: {}", nv.path.get_ident().map_or_else(|| "unknown".to_string(), |i| i.to_string()));
                                            return TokenStream::from(quote! { compile_error!(#msg); });
                                        }
                                    }
                                    _ => return TokenStream::from(quote! { compile_error!("primary_key options must be name-value pairs like name = \"id\""); }),
                                }
                            }
                        }
                        Err(e) => return e.to_compile_error().into(),
                    }
                }
                _ => return TokenStream::from(quote! { compile_error!("primary_key attribute must be a list like #[primary_key(name = \"id\")]"); }),
            }
        }
        // #[column_type(name = "field_name", sql_type = "VARCHAR(255)")] can be added later
    }
    
    let table_name = custom_table_name
        .unwrap_or_else(|| struct_name.to_string().to_lowercase() + "s");
    let pk_field_ident_original_struct = format_ident!("{}", pk_name_str);

    let fields_named = match &input_ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields_named), .. }) => fields_named,
        _ => return TokenStream::from(quote! { compile_error!("#[derive(SqlxObject)] is only supported for structs with named fields."); }),
    };

    let mut pk_original_struct_field_ty: Option<&Type> = None;
    let mut column_definitions: Vec<String> = Vec::new();
    let mut all_field_idents_for_select: Vec<&Ident> = Vec::new();
    let mut all_column_names_for_select: Vec<String> = Vec::new();
    
    let mut row_struct_fields_defs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut from_row_method_field_assignments: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in &fields_named.named {
        if let Some(ident) = &field.ident {
            all_field_idents_for_select.push(ident); // Used for RETURNING * or SELECT * conceptually
            let col_name_str = ident.to_string();
            all_column_names_for_select.push(format!("\"{}\"", col_name_str));

            let original_field_ty = &field.ty;
            let type_for_sql_mapping = get_option_inner_type(original_field_ty).unwrap_or(original_field_ty);
            let is_nullable_column = is_option_type(original_field_ty);
            let is_current_field_pk = col_name_str == pk_name_str;
            
            let fq_original_type_inner = get_fully_qualified_type_string(type_for_sql_mapping);
            let is_crypto_hash_field = fq_original_type_inner == "::voda_common::CryptoHash" || fq_original_type_inner == "voda_common::CryptoHash" || fq_original_type_inner == "CryptoHash";

            // Determine type for RowSqlx struct
            let row_field_ty = if is_crypto_hash_field {
                if is_nullable_column { parse_quote! { Option<Vec<u8>> } } else { parse_quote! { Vec<u8> } }
            } else {
                original_field_ty.clone() // Keep original type if not CryptoHash
            };
            row_struct_fields_defs.push(quote! { pub #ident: #row_field_ty });

            // Conversion logic for from_row method
            if is_crypto_hash_field {
                if is_nullable_column {
                    from_row_method_field_assignments.push(quote! {
                        #ident: row.#ident.map(|bytes| ::voda_common::CryptoHash::new(bytes.try_into().expect(&format!("Failed to convert Option<Vec<u8>> to [u8;32] for field '{}' (check length)", stringify!(#ident)))))
                    });
                } else {
                    from_row_method_field_assignments.push(quote! {
                        #ident: ::voda_common::CryptoHash::new(row.#ident.try_into().expect(&format!("Failed to convert Vec<u8> to [u8;32] for field '{}' (check length)", stringify!(#ident))))
                    });
                }
            } else {
                from_row_method_field_assignments.push(quote! { #ident: row.#ident });
            }

            if is_current_field_pk {
                pk_original_struct_field_ty = Some(original_field_ty);
            }
            
            let sql_type = map_rust_type_to_sql(type_for_sql_mapping, is_current_field_pk, pk_auto_generated && is_current_field_pk);
            let mut col_def_parts = vec![format!("\"{}\"", col_name_str), sql_type];

            if is_current_field_pk {
                col_def_parts.push("PRIMARY KEY".to_string());
            } else if !is_nullable_column {
                col_def_parts.push("NOT NULL".to_string());
            }
            column_definitions.push(col_def_parts.join(" "));
        }
    }

    let pk_original_ty_unwrapped = pk_original_struct_field_ty.expect(&format!("Primary key field '{}' not found in struct '{}'.", pk_name_str, struct_name));
    let pk_fq_original_inner_type = get_fully_qualified_type_string(get_option_inner_type(pk_original_ty_unwrapped).unwrap_or(pk_original_ty_unwrapped));
    let pk_is_actually_crypto_hash = pk_fq_original_inner_type == "::voda_common::CryptoHash" || pk_fq_original_inner_type == "voda_common::CryptoHash" || pk_fq_original_inner_type == "CryptoHash";
    
    let pk_is_conceptually_crypto_hash = pk_ty_override_str.as_deref().map_or(false, |s| s == "::voda_common::CryptoHash" || s == "voda_common::CryptoHash" || s == "CryptoHash") || pk_is_actually_crypto_hash;

    let final_pk_id_trait_type = if pk_is_conceptually_crypto_hash {
        quote! { Vec<u8> }
    } else if let Some(override_str) = pk_ty_override_str {
        // This path needs to be careful if override is not CryptoHash
        // For now, assume if override_str is present and not CryptoHash, it's a SQLx-compatible type
        let ty: Type = syn::parse_str(&override_str).expect("Failed to parse primary_key ty override string for Id trait");
        quote!{ #ty }
    } else {
        // Use the actual field type from the struct for the Id trait, if not CryptoHash
        quote! { #pk_original_ty_unwrapped }
    };

    let create_table_sql_query = format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name, column_definitions.join(", "));
    let drop_table_sql_query = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table_name);
    let columns_static_array_str: Vec<LitStr> = all_column_names_for_select.iter().map(|s| LitStr::new(&s.replace('"', ""), proc_macro2::Span::call_site())).collect();
    let all_columns_joined_str_for_select = all_column_names_for_select.join(", ");
    
    let select_all_sql_query = format!("SELECT {} FROM \"{}\"", all_columns_joined_str_for_select, table_name);
    let select_by_id_sql_query = format!("SELECT {} FROM \"{}\" WHERE \"{}\" = $1", all_columns_joined_str_for_select, table_name, pk_name_str);
    let delete_by_id_sql_query = format!("DELETE FROM \"{}\" WHERE \"{}\" = $1", table_name, pk_name_str);

    let mut insert_bindings = Vec::new();
    let mut insert_column_names_sql: Vec<String> = Vec::new();
    
    for field in fields_named.named.iter() {
        if let Some(ident) = &field.ident {
            let col_name = ident.to_string();
            if col_name == pk_name_str && pk_auto_generated { continue; }

            insert_column_names_sql.push(format!("\"{}\"", col_name));
            
            let original_field_ty = &field.ty;
            let inner_original_type = get_option_inner_type(original_field_ty).unwrap_or(original_field_ty);
            let fq_inner_original_type = get_fully_qualified_type_string(inner_original_type);
            
            let is_crypto_hash_field = fq_inner_original_type == "::voda_common::CryptoHash" || fq_inner_original_type == "voda_common::CryptoHash" || fq_inner_original_type == "CryptoHash";

            if is_crypto_hash_field {
                if is_option_type(original_field_ty) {
                    insert_bindings.push(quote! { .bind(self.#ident.as_ref().map(|ch| ch.hash().to_vec())) });
                } else {
                    insert_bindings.push(quote! { .bind(self.#ident.hash().to_vec()) });
                }
            } else {
                 if is_option_type(original_field_ty) {
                    insert_bindings.push(quote! { .bind(self.#ident.as_ref()) });
                } else {
                    insert_bindings.push(quote! { .bind(self.#ident.clone()) });
                }
            }
        }
    }
    let insert_column_names_joined_sql = insert_column_names_sql.join(", ");
    let insert_bind_placeholders_sql = (1..=insert_column_names_sql.len()).map(|i| format!("${}", i)).collect::<Vec<String>>().join(", ");
    
    let insert_sql_query = if insert_column_names_sql.is_empty() && pk_auto_generated {
        format!("INSERT INTO \"{}\" DEFAULT VALUES RETURNING {}", table_name, all_columns_joined_str_for_select)
    } else {
        format!("INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}", table_name, insert_column_names_joined_sql, insert_bind_placeholders_sql, all_columns_joined_str_for_select)
    };

    let mut update_bindings = Vec::new();
    let mut update_set_clauses_sql: Vec<String> = Vec::new();
    let mut update_placeholder_idx = 1;

    for field in fields_named.named.iter() {
        if let Some(ident) = &field.ident {
            let col_name = ident.to_string();
            if col_name == pk_name_str { continue; } // PK is handled by pk_binding_for_update

            update_set_clauses_sql.push(format!("\"{}\" = ${}", col_name, update_placeholder_idx));
            update_placeholder_idx += 1;

            let original_field_ty = &field.ty;
            let inner_original_type = get_option_inner_type(original_field_ty).unwrap_or(original_field_ty);
            let fq_inner_original_type = get_fully_qualified_type_string(inner_original_type);
            let is_crypto_hash_field = fq_inner_original_type == "::voda_common::CryptoHash" || fq_inner_original_type == "voda_common::CryptoHash" || fq_inner_original_type == "CryptoHash";

            if is_crypto_hash_field {
                 if is_option_type(original_field_ty) {
                    update_bindings.push(quote! { .bind(self.#ident.as_ref().map(|ch| ch.hash().to_vec())) });
                } else {
                    update_bindings.push(quote! { .bind(self.#ident.hash().to_vec()) });
                }
            } else {
                if is_option_type(original_field_ty) {
                    update_bindings.push(quote! { .bind(self.#ident.as_ref()) });
                } else {
                    update_bindings.push(quote! { .bind(self.#ident.clone()) });
                }
            }
        }
    }
    let update_set_str_sql = update_set_clauses_sql.join(", ");
    let update_by_id_sql_query = format!("UPDATE \"{}\" SET {} WHERE \"{}\" = ${} RETURNING {}", table_name, update_set_str_sql, pk_name_str, update_placeholder_idx, all_columns_joined_str_for_select);
    
    let pk_binding_for_update = if pk_is_conceptually_crypto_hash {
        // self.id is CryptoHash. We need Vec<u8> for binding if SqlxSchema::Id is Vec<u8>
        quote! { .bind(self.#pk_field_ident_original_struct.hash().to_vec()) } 
    } else {
        quote! { .bind(self.#pk_field_ident_original_struct.clone()) }
    };

    let get_id_value_impl = if pk_is_conceptually_crypto_hash {
        // self.id is CryptoHash. SqlxSchema::Id is Vec<u8>. Convert.
        quote! { self.#pk_field_ident_original_struct.hash().to_vec() }
    } else {
        // self.id is some other type, SqlxSchema::Id is that type. Clone.
        quote! { self.#pk_field_ident_original_struct.clone() }
    };
    
    let expanded = quote! {
        #[derive(::sqlx::FromRow, Debug, Clone)]
        #[automatically_derived]
        pub struct #row_struct_name {
            #(#row_struct_fields_defs),*
        }

        #[automatically_derived]
        impl ::voda_database::sqlx_postgres_traits::SqlxSchema for #struct_name {
            type Id = #final_pk_id_trait_type;
            type Row = #row_struct_name;

            const TABLE_NAME: &'static str = #table_name;
            const ID_COLUMN_NAME: &'static str = #pk_name_str;
            const COLUMNS: &'static [&'static str] = &[#( #columns_static_array_str ),*];

            fn get_id_value(&self) -> Self::Id {
                #get_id_value_impl
            }

            fn from_row(row: Self::Row) -> Self {
                Self {
                    #(#from_row_method_field_assignments),*
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
        impl ::voda_database::sqlx_postgres_traits::SqlxCrud for #struct_name {
            fn bind_insert<'q>(&self, query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments>)
                -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments> {
                query
                #( #insert_bindings )*
            }

            fn bind_update<'q>(&self, query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments>)
                -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, Self::Row, ::sqlx::postgres::PgArguments> {
                query
                #( #update_bindings )*
                #pk_binding_for_update // Bind PK last
            }
        }
    };

    TokenStream::from(expanded)
}
