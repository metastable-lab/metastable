use proc_macro::TokenStream;
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