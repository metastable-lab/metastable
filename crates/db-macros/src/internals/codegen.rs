use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, LitStr, Type, parse_quote};
use super::types::{FieldData, get_fully_qualified_type_string, get_vec_inner_type, is_simple_type, is_option_type, get_option_inner_type};

pub fn generate_row_struct(row_struct_name: &Ident, fields_data: &[FieldData]) -> TokenStream {
    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();

    let row_struct_fields_defs: Vec<TokenStream> = active_fields.iter().map(|field| {
        let field_ident = format_ident!("{}", field.name);
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

    quote! {
        #[derive(::sqlx::FromRow, Debug, Clone)]
        #[automatically_derived]
        pub struct #row_struct_name {
            #(#row_struct_fields_defs),*
        }
    }
}

pub fn generate_sqlx_schema_impl(struct_name: &Ident, row_struct_name: &Ident, table_name_str: &str, fields_data: &[FieldData]) -> TokenStream {
    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();
    let has_updated_at = active_fields.iter().any(|f| f.name == "updated_at");

    let all_sql_column_names_str_lits: Vec<LitStr> = active_fields.iter()
        .map(|f| LitStr::new(&f.name, proc_macro2::Span::call_site()))
        .collect();

    let from_row_assignments = generate_from_row_assignments(fields_data);

    let (create_table_sql_query, create_index_sqls) = generate_create_table_sql(table_name_str, fields_data);
    let drop_table_sql_query = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", table_name_str);
    let insert_sql_query = generate_insert_sql(table_name_str, &active_fields);
    
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

    let final_pk_id_trait_type: Type = parse_quote!(::sqlx::types::Uuid);

    quote! {
        #[automatically_derived]
        impl ::metastable_database::SqlxSchema for #struct_name {
            type Id = #final_pk_id_trait_type;
            type Row = #row_struct_name;

            const TABLE_NAME: &'static str = #table_name_str;
            const ID_COLUMN_NAME: &'static str = "id";
            const COLUMNS: &'static [&'static str] = &[#( #all_sql_column_names_str_lits ),*];
            const INDEXES_SQL: &'static [&'static str] = &[#( #create_index_sqls ),*];

            fn get_id_value(&self) -> Self::Id { self.id }

            fn from_row(row: Self::Row) -> Self {
                Self {
                    #(#from_row_assignments),*
                }
            }

            fn insert_sql() -> String { #insert_sql_query.to_string() }
            fn create_table_sql() -> String { #create_table_sql_query.to_string() }
            fn drop_table_sql() -> String { #drop_table_sql_query.to_string() }
            fn trigger_sql() -> String { #trigger_sql_impl.to_string() }
        }
    }
}

pub fn generate_sqlx_crud_impl(struct_name: &Ident, table_name_str: &str, fields_data: &[FieldData]) -> TokenStream {
    let (insert_bindings, update_bindings) = generate_bind_streams(fields_data);
    let (update_sql, is_select_only) = generate_update_sql(table_name_str, fields_data);
    let delete_sql = format!("DELETE FROM \"{}\" WHERE \"id\" = $1", table_name_str);
    
    quote! {
        #[automatically_derived]
        #[::async_trait::async_trait]
        impl ::metastable_database::SqlxCrud for #struct_name {
            fn bind_insert<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::metastable_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::metastable_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                query #(#insert_bindings)*
            }

            fn bind_update<'q>(
                &self, 
                query: ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::metastable_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments>
            ) -> ::sqlx::query::QueryAs<'q, ::sqlx::Postgres, <Self as ::metastable_database::SqlxSchema>::Row, ::sqlx::postgres::PgArguments> {
                let query = query #(#update_bindings)*;
                if #is_select_only {
                    query.bind(self.id)
                } else {
                    query.bind(self.id)
                }
            }

            async fn create<'e, E>(self, executor: E) -> Result<Self, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let sql = <Self as ::metastable_database::SqlxSchema>::insert_sql();
                self.bind_insert(::sqlx::query_as::<_, <Self as ::metastable_database::SqlxSchema>::Row>(&sql))
                    .fetch_one(executor)
                    .await
                    .map(<Self as ::metastable_database::SqlxSchema>::from_row)
            }

            async fn update<'e, E>(self, executor: E) -> Result<Self, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                let sql = #update_sql;
                self.bind_update(::sqlx::query_as::<_, <Self as ::metastable_database::SqlxSchema>::Row>(&sql))
                    .fetch_one(executor)
                    .await
                    .map(<Self as ::metastable_database::SqlxSchema>::from_row)
            }

            async fn delete<'e, E>(self, executor: E) -> Result<u64, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Postgres> + Send,
                Self: Send
            {
                ::sqlx::query(#delete_sql)
                    .bind(self.id)
                    .execute(executor)
                    .await
                    .map(|done| done.rows_affected())
            }
        }
    }
}

pub fn generate_sqlx_filter_query_impl(struct_name: &Ident, row_struct_name: &Ident) -> TokenStream {
    quote! {
        #[automatically_derived]
        #[::async_trait::async_trait]
        impl ::metastable_database::SqlxFilterQuery for #struct_name {
            async fn find_by_criteria<'exe, E>(
                criteria: ::metastable_database::QueryCriteria,
                executor: E,
            ) -> Result<Vec<Self>, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                Self: Send,
            {
                let mut sql_query_parts: Vec<String> = Vec::new();
                let mut arguments = ::sqlx::postgres::PgArguments::default();
                let mut placeholder_idx = 1;
                let mut select_columns = (<Self as ::metastable_database::SqlxSchema>::COLUMNS).join(", ");
                let mut where_clauses: Vec<String> = Vec::new();

                if let Some(ss) = &criteria.similarity_search {
                    use ::sqlx::Arguments;
                    arguments.add(ss.vector.clone()).map_err(::sqlx::Error::Encode)?;
                    let vector_placeholder = placeholder_idx;
                    placeholder_idx += 1;
                    select_columns = format!("*, 1 - (embedding <=> ${}) as {}", vector_placeholder, ss.as_field);

                    if let Some(threshold) = ss.threshold {
                        arguments.add(threshold).map_err(::sqlx::Error::Encode)?;
                        let threshold_placeholder = placeholder_idx;
                        placeholder_idx += 1;
                        where_clauses.push(format!("1 - (embedding <=> ${}) >= ${}", vector_placeholder, threshold_placeholder));
                    }
                }

                sql_query_parts.push(format!(
                    "SELECT {} FROM \"{}\"", 
                    select_columns, 
                    <Self as ::metastable_database::SqlxSchema>::TABLE_NAME
                ));

                for condition in &criteria.conditions {
                    let mut current_condition_sql = format!("\"{}\" {}", condition.column, condition.operator);
                    if let Some(value) = &condition.value {
                        value.add_to_args(&mut arguments)?;
                        if !condition.operator.contains('$') {
                            current_condition_sql.push_str(&format!(" ${}", placeholder_idx));
                        }
                        placeholder_idx += 1;
                    }
                    where_clauses.push(current_condition_sql);
                }
                
                if !where_clauses.is_empty() {
                    sql_query_parts.push(format!("WHERE {}", where_clauses.join(" AND ")));
                }


                if !criteria.order_by.is_empty() {
                    sql_query_parts.push("ORDER BY".to_string());
                    let order_clauses: Vec<String> = criteria.order_by.iter().map(|&(col, dir)| {
                        if criteria.similarity_search.as_ref().map_or(false, |ssi| ssi.as_field == col) {
                            format!("{} {}", col, dir.as_sql())
                        } else {
                            format!("\"{}\" {}", col, dir.as_sql())
                        }
                    }).collect();
                    sql_query_parts.push(order_clauses.join(", "));
                }

                if let Some(limit_val) = criteria.limit {
                    use ::sqlx::Arguments;
                    arguments.add(limit_val).map_err(::sqlx::Error::Encode)?;
                    sql_query_parts.push(format!("LIMIT ${}", placeholder_idx));
                    placeholder_idx += 1;
                }

                if let Some(offset_val) = criteria.offset {
                    use ::sqlx::Arguments;
                    arguments.add(offset_val).map_err(::sqlx::Error::Encode)?;
                    sql_query_parts.push(format!("OFFSET ${}", placeholder_idx));
                }

                let final_sql = sql_query_parts.join(" ");
                
                ::sqlx::query_as_with::<_, #row_struct_name, _>(&final_sql, arguments)
                    .fetch_all(executor)
                    .await
                    .map(|rows| rows.into_iter().map(<Self as ::metastable_database::SqlxSchema>::from_row).collect())
            }

            async fn delete_by_criteria<'exe, E>(
                criteria: ::metastable_database::QueryCriteria,
                executor: E,
            ) -> Result<u64, ::sqlx::Error>
            where
                E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                Self: Send,
            {
                let mut sql_query_parts: Vec<String> = Vec::new();
                let mut arguments = ::sqlx::postgres::PgArguments::default();
                let mut placeholder_idx = 1;
                
                sql_query_parts.push(format!("DELETE FROM \"{}\"", <Self as ::metastable_database::SqlxSchema>::TABLE_NAME));

                if !criteria.conditions.is_empty() {
                    sql_query_parts.push("WHERE".to_string());
                    let mut where_clauses = Vec::new();
                    for condition in &criteria.conditions { 
                        let mut current_condition_sql = format!("\"{}\" {}", condition.column, condition.operator);
                        if let Some(value) = &condition.value {
                            value.add_to_args(&mut arguments)?;
                            if !condition.operator.contains('$') {
                                current_condition_sql.push_str(&format!(" ${}", placeholder_idx));
                            }
                            placeholder_idx += 1;
                        }
                        where_clauses.push(current_condition_sql);
                    }
                    sql_query_parts.push(where_clauses.join(" AND "));
                }
                
                let final_sql = sql_query_parts.join(" ");
                
                ::sqlx::query_with(&final_sql, arguments)
                    .execute(executor)
                    .await
                    .map(|done| done.rows_affected())
            }
        }
    }
}

pub fn generate_fetch_helpers(fields_data: &[FieldData]) -> TokenStream {
    let fetch_helper_methods: Vec<TokenStream> = fields_data.iter().filter_map(|field| {
        let field_ident = format_ident!("{}", field.name);
        
        if let Some(fk_info) = &field.foreign_key {
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_info.related_rust_type;
            let self_field_access = quote!{ self.#field_ident };
            
            let id_column_name_of_related_type = quote!{ <#related_type as ::metastable_database::SqlxSchema>::id_column_name() };

            if field.is_option {
                Some(quote! {
                    pub async fn #fetch_method_name<'exe, E>(
                        &self, 
                        executor: E
                    ) -> Result<Option<#related_type>, ::sqlx::Error>
                    where
                        E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                    {
                        if let Some(id_val_ref) = &#self_field_access {
                            let criteria = ::metastable_database::QueryCriteria::new()
                                .add_valued_filter(#id_column_name_of_related_type, "=", *id_val_ref);
                            <#related_type as ::metastable_database::SqlxFilterQuery>::find_one_by_criteria(criteria, executor).await
                        } else {
                            Ok(None)
                        }
                    }
                })
            } else {
                Some(quote! {
                    pub async fn #fetch_method_name<'exe, E>(
                        &self, 
                        executor: E
                    ) -> Result<Option<#related_type>, ::sqlx::Error>
                    where
                        E: ::sqlx::Executor<'exe, Database = ::sqlx::Postgres> + Send,
                    {
                        let criteria = ::metastable_database::QueryCriteria::new()
                            .add_valued_filter(#id_column_name_of_related_type, "=", #self_field_access);
                        <#related_type as ::metastable_database::SqlxFilterQuery>::find_one_by_criteria(criteria, executor).await
                    }
                })
            }
        } else if let Some(fk_many_info) = &field.foreign_key_many {
            let fetch_method_name = format_ident!("fetch_{}", field_ident);
            let related_type = &fk_many_info.related_rust_type;
            let referenced_table_str = &fk_many_info.referenced_table;
            
            Some(quote! {
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
                    
                    let related_rows = sqlx::query_as::<_, <#related_type as ::metastable_database::SqlxSchema>::Row>(&sql)
                        .bind(ids)
                        .fetch_all(executor)
                        .await?;
                    
                    Ok(related_rows.into_iter().map(<#related_type as ::metastable_database::SqlxSchema>::from_row).collect())
                }
            })
        } else {
            None
        }
    }).collect();

    quote! { #(#fetch_helper_methods)* }
}

fn generate_from_row_assignments(fields_data: &[FieldData]) -> Vec<TokenStream> {
    let from_row_sql_field_assignments: Vec<TokenStream> = fields_data.iter()
        .filter(|f| !f.is_skipped)
        .map(|field| {
        let field_ident = format_ident!("{}", field.name);
        let field_ty = &field.ty;
        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");
        
        let field_is_option = is_option_type(field_ty);
        let row_field_name = &field_ident;

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
    
    let skipped_field_default_assignments: Vec<TokenStream> = fields_data.iter()
        .filter(|field| field.is_skipped)
        .map(|field| {
            let field_ident = format_ident!("{}", field.name);
            quote! { #field_ident: Default::default() }
        })
        .collect();

    let mut all_from_row_assignments = from_row_sql_field_assignments;
    all_from_row_assignments.extend(skipped_field_default_assignments);
    all_from_row_assignments
}

fn generate_create_table_sql(table_name_str: &str, fields_data: &[FieldData]) -> (String, Vec<LitStr>) {
    let mut create_table_column_defs: Vec<String> = Vec::new();
    let mut foreign_key_clauses_for_create_table: Vec<String> = Vec::new();
    let mut create_index_sqls: Vec<LitStr> = Vec::new();

    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();

    for field in active_fields {
        let mut col_def_parts = vec![format!("\"{}\"", field.name), field.sql_type.clone()];
        
        if field.is_pk {
            col_def_parts.push("PRIMARY KEY".to_string());
            col_def_parts.push("DEFAULT gen_random_uuid()".to_string());
        }
        else if field.name == "created_at" || field.name == "updated_at" {
            let idx = col_def_parts.iter().position(|s| s == &field.sql_type).unwrap();
            col_def_parts[idx] = "BIGINT".to_string();
            col_def_parts.push("NOT NULL DEFAULT floor(extract(epoch from now()))".to_string());
        }
        else if !field.is_option { col_def_parts.push("NOT NULL".to_string()); }

        if field.unique {
            col_def_parts.push("UNIQUE".to_string());
        }

        if field.indexed {
            let index_name = format!("idx_{}_{}", table_name_str, field.name);
            let index_sql = format!(
                "CREATE INDEX IF NOT EXISTS \"{}\" ON \"{}\"(\"{}\")",
                index_name, table_name_str, field.name
            );
            create_index_sqls.push(LitStr::new(&index_sql, proc_macro2::Span::call_site()));
        }

        create_table_column_defs.push(col_def_parts.join(" "));

        if let Some(fk_info) = &field.foreign_key {
            foreign_key_clauses_for_create_table.push(format!(
                "FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"id\") ON DELETE SET NULL ON UPDATE CASCADE",
                field.name, fk_info.referenced_table
            ));
        }
    }

    let mut create_table_parts = create_table_column_defs;
    if !foreign_key_clauses_for_create_table.is_empty() {
        create_table_parts.extend(foreign_key_clauses_for_create_table);
    }
    let create_table_sql_query = format!("CREATE TABLE IF NOT EXISTS \"{}\" ({})", table_name_str, create_table_parts.join(", "));
    
    (create_table_sql_query, create_index_sqls)
}

fn generate_insert_sql(table_name_str: &str, active_fields: &[&FieldData]) -> String {
    let insert_col_sql_names: Vec<String> = active_fields.iter()
        .filter(|f| f.name != "created_at" && f.name != "updated_at" && !f.is_pk)
        .map(|f| format!("\"{}\"", f.name))
        .collect();

    let insert_column_names_joined_sql = insert_col_sql_names.join(", ");
    let insert_bind_placeholders_sql = (1..=insert_col_sql_names.len()).map(|i| format!("${}", i)).collect::<Vec<String>>().join(", ");
    
    let all_sql_columns_joined_str = active_fields.iter().map(|s| format!("\"{}\"", s.name)).collect::<Vec<String>>().join(", ");

    format!("INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}", table_name_str, insert_column_names_joined_sql, insert_bind_placeholders_sql, all_sql_columns_joined_str)
}

fn generate_update_sql(table_name_str: &str, fields_data: &[FieldData]) -> (String, bool) {
    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();

    let update_set_clauses_sql: Vec<String> = active_fields.iter()
        .filter(|f| f.name != "created_at" && f.name != "updated_at" && !f.is_pk)
        .enumerate()
        .map(|(i, f)| format!("\"{}\" = ${}", f.name, i + 1))
        .collect();
    
    let all_sql_columns_joined_str = active_fields.iter().map(|s| format!("\"{}\"", s.name)).collect::<Vec<String>>().join(", ");

    let is_select_only = update_set_clauses_sql.is_empty();
    let sql = if is_select_only {
        format!("SELECT {} FROM \"{}\" WHERE \"id\" = $1", all_sql_columns_joined_str, table_name_str) 
    } else {
        let update_set_str_sql = update_set_clauses_sql.join(", ");
        let pk_placeholder_idx = update_set_clauses_sql.len() + 1;
        format!("UPDATE \"{}\" SET {} WHERE \"id\" = ${} RETURNING {}", table_name_str, update_set_str_sql, pk_placeholder_idx, all_sql_columns_joined_str)
    };

    (sql, is_select_only)
}

fn generate_bind_streams(fields_data: &[FieldData]) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut insert_bindings_streams: Vec<TokenStream> = Vec::new();
    let mut update_bindings_streams: Vec<TokenStream> = Vec::new();

    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();

    for field in active_fields {
        if field.name == "created_at" || field.name == "updated_at" || field.is_pk {
            continue;
        }

        let field_ident = format_ident!("{}", field.name);
        let field_access_path = quote!{ self.#field_ident };
        let field_is_option = field.is_option;
        let type_for_analysis = get_option_inner_type(&field.ty).unwrap_or_else(|| field.ty.clone()); 
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        
        let is_standalone_text_mappable_candidate =
            !is_simple_type(&type_for_analysis) &&
            !fq_type_str_for_analysis.starts_with("Json<") &&
            get_vec_inner_type(&type_for_analysis).is_none();
        
        let is_vec_text_mappable_enum = get_vec_inner_type(&field.ty).map_or(false, |vt| 
            !is_simple_type(&vt) && 
            !get_fully_qualified_type_string(&vt).starts_with("Option<") && 
            !get_fully_qualified_type_string(&vt).starts_with("Vec<")
        );
        
        let bind_stream = if is_standalone_text_mappable_candidate {
            if field_is_option {
                 quote! { .bind(#field_access_path.as_ref().map(|v| v.to_string())) }
            } else {
                 quote! { .bind(#field_access_path.to_string()) }
            }
        } else if is_vec_text_mappable_enum {
            quote! { .bind(#field_access_path.iter().map(|v| v.to_string()).collect::<Vec<String>>()) }
        } else {
            quote! { .bind(#field_access_path.clone()) }
        };
        
        insert_bindings_streams.push(bind_stream.clone());
        update_bindings_streams.push(bind_stream);
    }

    (insert_bindings_streams, update_bindings_streams)
}

fn get_sql_default_value(field: &FieldData) -> String {
    let sql_type_upper = field.sql_type.to_uppercase();

    if sql_type_upper.ends_with("[]") {
        "DEFAULT '{}'".to_string()
    } else if sql_type_upper.starts_with("TEXT") || sql_type_upper.starts_with("VARCHAR") {
        "DEFAULT ''".to_string()
    } else if sql_type_upper.starts_with("INT") || sql_type_upper.starts_with("BIGINT") || sql_type_upper.starts_with("REAL") || sql_type_upper.starts_with("DOUBLE") {
        "DEFAULT 0".to_string()
    } else if sql_type_upper.starts_with("BOOL") {
        "DEFAULT false".to_string()
    } else if sql_type_upper.starts_with("JSON") {
        "DEFAULT '{}'".to_string()
    } else if sql_type_upper.starts_with("UUID") {
        "DEFAULT '00000000-0000-0000-0000-000000000000'".to_string()
    } else if sql_type_upper.starts_with("VECTOR") {
        let dim = field.vector_dimension.unwrap_or(0);
        let zeros = vec!["0"; dim].join(",");
        format!("DEFAULT '[{}]'", zeros)
    } else if sql_type_upper.contains("TIMESTAMP") {
        "DEFAULT to_timestamp(0)".to_string()
    } else {
        "".to_string()
    }
}

pub fn generate_migrate_fn(
    struct_name: &syn::Ident,
    table_name: &str,
    fields_data: &[FieldData],
    allow_column_dropping: bool,
) -> TokenStream {
    let active_fields: Vec<_> = fields_data.iter().filter(|f| !f.is_skipped).collect();

    let add_column_logics = active_fields
        .iter()
        .map(|field| {
            let col_name = &field.name;
            let sql_type = &field.sql_type;
            let is_nullable = field.is_option;

            let mut add_sql_parts = vec![
                format!("ALTER TABLE \"{}\"", table_name),
                "ADD COLUMN".to_string(),
                format!("\"{}\"", col_name),
                sql_type.clone(),
            ];

            if !is_nullable {
                add_sql_parts.push("NOT NULL".to_string());
                let default_clause = get_sql_default_value(field);
                if !default_clause.is_empty() {
                    add_sql_parts.push(default_clause);
                }
            }

            let add_sql = add_sql_parts.join(" ");

            quote! {
                if !db_columns.contains_key(#col_name) {
                    println!("[MIGRATE][ACTION] Table '{}': Adding column '{}'.", #table_name, #col_name);
                    alter_statements.push(#add_sql.to_string());
                }
            }
        });

    let struct_column_definitions: Vec<_> = active_fields
        .iter()
        .map(|f| {
            let column_name = &f.name;
            let sql_type = &f.sql_type;
            let is_nullable = f.is_option;
            quote! {
                ( #column_name, #sql_type, #is_nullable )
            }
        })
        .collect();
    
    let has_updated_at = active_fields.iter().any(|f| f.name == "updated_at");
    let trigger_name = format!("set_updated_at_{}", table_name);

    quote! {
        #[async_trait::async_trait]
        impl ::metastable_database::SchemaMigrator for #struct_name {
            async fn migrate(pool: &::sqlx::PgPool) -> anyhow::Result<()> {
                use sqlx::Row;
                
                fn are_sql_types_equivalent(struct_type: &str, db_type_raw: &str) -> bool {
                    let struct_type_upper = struct_type.trim().to_uppercase();
                    let db_type_upper = db_type_raw.trim().to_uppercase();

                    if struct_type_upper.ends_with("[]") && db_type_upper.starts_with('_') {
                        let struct_inner = struct_type_upper.trim_end_matches("[]");
                        let db_inner = db_type_upper.trim_start_matches('_');
                        return are_sql_types_equivalent(struct_inner, db_inner);
                    }
                    
                    let struct_type_base = struct_type_upper.split(|c| c == '(' || c == '[').next().unwrap_or("").trim();

                    if db_type_upper.starts_with(struct_type_base) {
                        return true;
                    }

                    match (struct_type_base, db_type_upper.as_str()) {
                        ("BIGINT", "INT8") => true,
                        ("INTEGER", "INT4") => true,
                        ("REAL", "FLOAT4") => true,
                        ("BOOLEAN", "BOOL") => true,
                        ("DOUBLE PRECISION", "FLOAT8") => true,
                        ("TEXT", s) if s.starts_with("VARCHAR") => true,
                        _ => false,
                    }
                }

                println!("[MIGRATE][INFO] Starting migration check for table '{}'...", #table_name);

                let table_exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS (
                        SELECT FROM information_schema.tables 
                        WHERE table_schema = 'public' AND table_name = $1
                    )"
                )
                .bind(#table_name)
                .fetch_one(pool)
                .await?;

                if !table_exists {
                    println!("[MIGRATE][ACTION] Table '{}' does not exist. Creating it now.", #table_name);
                    let create_sql = Self::create_table_sql();
                    let trigger_func_sql = r#"
                    CREATE OR REPLACE FUNCTION set_updated_at_unix_timestamp()
                    RETURNS TRIGGER AS $$
                    BEGIN NEW.updated_at = floor(extract(epoch from now())); RETURN NEW; END;
                    $$ language 'plpgsql';
                    "#;
                    let mut tx = pool.begin().await?;
                    sqlx::query(trigger_func_sql).execute(&mut *tx).await.ok();
                    sqlx::query(&create_sql).execute(&mut *tx).await?;
                    for index_sql in Self::INDEXES_SQL {
                        sqlx::query(index_sql).execute(&mut *tx).await?;
                    }
                    let trigger_sql = Self::trigger_sql();
                     if !trigger_sql.is_empty() {
                        for statement in trigger_sql.split(';').filter(|s| !s.trim().is_empty()) {
                            sqlx::query(statement).execute(&mut *tx).await
                                .map_err(|e| anyhow::anyhow!("Failed to execute trigger statement '{}': {}", statement, e))?;
                        }
                    }
                    tx.commit().await?;

                    println!("[MIGRATE][SUCCESS] Table '{}' created.", #table_name);
                    return Ok(());
                }

                let db_columns: std::collections::HashMap<String, (String, bool)> = sqlx::query(
                    "SELECT column_name, udt_name, is_nullable 
                     FROM information_schema.columns 
                     WHERE table_name = $1 AND table_schema = 'public'"
                )
                .bind(#table_name)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| {
                    let col_name: String = row.get("column_name");
                    let type_name: String = row.get("udt_name");
                    let nullable: String = row.get("is_nullable");
                    (col_name, (type_name.to_uppercase(), nullable == "YES"))
                })
                .collect();

                let struct_columns: std::collections::HashMap<String, (String, bool)> = {
                    let mut map = std::collections::HashMap::new();
                    #(
                        let (col_name, sql_type, is_nullable) = #struct_column_definitions;
                        map.insert(col_name.to_string(), (sql_type.to_string(), is_nullable));
                    )*
                    map
                };

                let mut alter_statements = Vec::new();

                #(#add_column_logics)*
                
                for (col_name, _) in &db_columns {
                    if !struct_columns.contains_key(col_name) {
                        if #allow_column_dropping {
                            let drop_sql = format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\"", #table_name, col_name);
                            println!("[MIGRATE][ACTION] Table '{}': Dropping column '{}' as 'allow_column_dropping' is enabled.", #table_name, col_name);
                            alter_statements.push(drop_sql);
                        } else {
                            println!("[MIGRATE][WARNING] Table '{}': Column '{}' exists in the database but not in the struct. This column will NOT be dropped automatically.", #table_name, col_name);
                        }
                    }
                }

                for (col_name, (db_type, db_nullable)) in &db_columns {
                    if let Some((struct_type, struct_nullable)) = struct_columns.get(col_name) {
                        if !are_sql_types_equivalent(struct_type, db_type) {
                             println!("[MIGRATE][WARNING] Table '{}': Mismatch for column '{}'. Struct expects compatible with '{}' but database has '{}'. The column type will NOT be changed.", #table_name, col_name, struct_type, db_type);
                        }
                        if *db_nullable != *struct_nullable {
                            let new_nullability = if *struct_nullable { "DROP NOT NULL" } else { "SET NOT NULL" };
                            let alter_null_sql = format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" {}", #table_name, col_name, new_nullability);
                            println!("[MIGRATE][ACTION] Table '{}': Altering nullability of column '{}'.", #table_name, col_name);
                            alter_statements.push(alter_null_sql);
                        }
                    }
                }

                if !alter_statements.is_empty() {
                    let mut tx = pool.begin().await?;

                    if #has_updated_at {
                        let disable_trigger_sql = format!("ALTER TABLE \"{}\" DISABLE TRIGGER \"{}\"", #table_name, #trigger_name);
                        sqlx::query(&disable_trigger_sql).execute(&mut *tx).await.ok();
                    }

                    for stmt in alter_statements {
                        sqlx::query(&stmt).execute(&mut *tx).await?;
                    }

                    if #has_updated_at {
                        let enable_trigger_sql = format!("ALTER TABLE \"{}\" ENABLE TRIGGER \"{}\"", #table_name, #trigger_name);
                        sqlx::query(&enable_trigger_sql).execute(&mut *tx).await.ok();
                    }

                    tx.commit().await?;
                    println!("[MIGRATE][SUCCESS] Table '{}' migrated successfully.", #table_name);
                } else {
                    println!("[MIGRATE][INFO] Table '{}' is already up-to-date.", #table_name);
                }

                Ok(())
            }
        }
    }
} 