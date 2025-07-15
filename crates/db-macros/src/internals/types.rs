use quote::ToTokens;
use syn::{GenericArgument, PathArguments, Type};

// Helper data structures
#[derive(Debug)]
pub struct ForeignKeyInfo {
    pub referenced_table: String,
    pub related_rust_type: syn::Ident,
}

#[derive(Debug)]
pub struct ForeignKeyManyInfo {
    pub referenced_table: String,
    pub related_rust_type: syn::Ident,
}

pub struct FieldData {
    pub name: String,
    pub ty: syn::Type,
    pub is_option: bool,
    pub is_pk: bool,
    pub is_skipped: bool,
    pub sql_type: String,
    pub foreign_key: Option<ForeignKeyInfo>,
    pub foreign_key_many: Option<ForeignKeyManyInfo>,
    pub unique: bool,
    pub indexed: bool,
    pub vector_dimension: Option<usize>,
}

impl std::fmt::Debug for FieldData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldData")
            .field("name", &self.name)
            .field("ty", &self.ty.to_token_stream().to_string())
            .field("is_option", &self.is_option)
            .field("is_pk", &self.is_pk)
            .field("is_skipped", &self.is_skipped)
            .field("sql_type", &self.sql_type)
            .field("foreign_key", &self.foreign_key)
            .field("foreign_key_many", &self.foreign_key_many)
            .field("unique", &self.unique)
            .field("indexed", &self.indexed)
            .field("vector_dimension", &self.vector_dimension)
            .finish()
    }
}


// Helper functions for type analysis
pub fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Option" {
                return true;
            }
        }
    }
    false
}

pub fn get_option_inner_type(ty: &Type) -> Option<Type> {
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

pub fn get_vec_inner_type(ty: &Type) -> Option<Type> {
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

pub fn get_fully_qualified_type_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

pub fn is_simple_type(ty: &Type) -> bool {
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

pub fn map_rust_type_to_sql(ty: &Type, _is_pk: bool, processing_array_inner: bool, vector_dimension: Option<usize>) -> String {
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
                 panic!("Vec<{}> mapped to SQL type {} cannot be directly made into an SQL array. Consider Json<Vec<{}>> or a different structure.", quote::quote!(#inner_ty), inner_type_sql, quote::quote!(#inner_ty));
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