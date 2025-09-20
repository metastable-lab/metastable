use darling::{ast, FromDeriveInput, FromField};
use syn::{DeriveInput, Type};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(llm_tool))]
pub struct LlmTool {
    pub ident: syn::Ident,
    pub data: ast::Data<darling::util::Ignored, LlmToolField>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub enum_lang: Option<String>,
}

#[derive(Debug, FromField)]
#[darling(attributes(llm_tool))]
pub struct LlmToolField {
    pub ident: Option<syn::Ident>,
    pub ty: Type,
    pub description: Option<String>,
    #[darling(default)]
    pub is_enum: bool,
}

pub fn parse_llm_tool(input: &DeriveInput) -> Result<LlmTool, darling::Error> {
    LlmTool::from_derive_input(input)
}
