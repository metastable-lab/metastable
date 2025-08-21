extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod llm_tool_internals;

#[proc_macro_derive(LlmTool, attributes(llm_tool))]
pub fn derive_llm_tool(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let parsed_tool = match llm_tool_internals::parse::parse_llm_tool(&input) {
        Ok(data) => data,
        Err(e) => return e.write_errors().into(),
    };

    let expanded = llm_tool_internals::codegen::generate_llm_tool_impl(&parsed_tool);
    TokenStream::from(expanded)
}
