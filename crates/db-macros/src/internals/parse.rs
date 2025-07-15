use quote::format_ident;
use syn::Field;
use super::types::{
    FieldData, ForeignKeyInfo, ForeignKeyManyInfo, get_fully_qualified_type_string, 
    get_option_inner_type, is_option_type, map_rust_type_to_sql
};

// Functions for parsing attributes from fields
pub fn parse_foreign_key_attr(field: &Field) -> Option<ForeignKeyInfo> {
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
                    }
                }
                _ => {}
            }
        }
    }
    None
}

pub fn parse_foreign_key_many_attr(field: &Field) -> Option<ForeignKeyManyInfo> {
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
                    }
                }
                _ => {}
            }
        }
    }
    None
}

pub fn parse_vector_dimension_attr(field: &Field) -> Option<usize> {
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

pub fn has_unique_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("unique"))
}

pub fn has_indexed_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("indexed"))
}

pub fn has_sqlx_skip_column_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("sqlx_skip_column"))
}

/// Gathers all relevant data from the struct's fields.
pub fn get_fields_data(fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>) -> Vec<FieldData> {
    fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        let field_is_option = is_option_type(field_ty);
        let field_is_pk = field_ident == "id";
        let field_is_skipped = has_sqlx_skip_column_attr(field);

        let type_for_analysis = get_option_inner_type(field_ty).unwrap_or_else(|| field_ty.clone());
        let fq_type_str_for_analysis = get_fully_qualified_type_string(&type_for_analysis);
        let is_json_type_for_analysis = fq_type_str_for_analysis.starts_with("Json<") || fq_type_str_for_analysis.starts_with("::sqlx::types::Json<") || fq_type_str_for_analysis.starts_with("sqlx::types::Json<");

        let vector_dimension = parse_vector_dimension_attr(field);
        
        let sql_type_str = if field_is_skipped {
            "SKIP".to_string() 
        } else if is_json_type_for_analysis {
            "JSONB".to_string()
        } else {
            let actual_type_for_sql_map = if field_is_option { type_for_analysis.clone() } else { field_ty.clone() };
            map_rust_type_to_sql(&actual_type_for_sql_map, field_is_pk, false, vector_dimension)
        };

        FieldData {
            name: field_ident.to_string(),
            ty: field_ty.clone(),
            is_option: field_is_option,
            is_pk: field_is_pk,
            is_skipped: field_is_skipped,
            sql_type: sql_type_str,
            foreign_key: parse_foreign_key_attr(field),
            foreign_key_many: parse_foreign_key_many_attr(field),
            unique: has_unique_attr(field),
            indexed: has_indexed_attr(field),
            vector_dimension: vector_dimension,
        }
    }).collect()
} 