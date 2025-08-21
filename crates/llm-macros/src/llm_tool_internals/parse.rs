use darling::{FromDeriveInput, FromField};
use syn::{DeriveInput, Ident, Type};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(llm_tool), forward_attrs(doc))]
pub struct LlmTool {
    pub ident: Ident,
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub description: Option<String>,
    pub data: darling::ast::Data<darling::util::Ignored, LlmToolField>,
}

#[derive(Debug, FromField)]
#[darling(attributes(llm_tool), forward_attrs(doc))]
pub struct LlmToolField {
    pub ident: Option<Ident>,
    pub ty: Type,
    #[darling(default)]
    pub description: Option<String>,
    #[darling(default)]
    pub enum_lang: Option<String>,
    #[darling(default)]
    pub is_enum: bool,
}

pub fn parse_llm_tool(input: &DeriveInput) -> Result<LlmTool, darling::Error> {
    LlmTool::from_derive_input(input)
}
