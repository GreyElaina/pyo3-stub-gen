use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::Type;

pub struct StubType {
    pub(crate) ty: Type,
    pub(crate) name: String,
    pub(crate) module: Option<String>,
    pub(crate) type_input_override: Option<TokenStream2>,
    pub(crate) type_output_override: Option<TokenStream2>,
}

impl ToTokens for StubType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Self {
            ty,
            name,
            module,
            type_input_override,
            type_output_override,
        } = self;
        let module_tt = if let Some(module) = module {
            quote! { #module.into() }
        } else {
            quote! { Default::default() }
        };
        let default_output =
            quote! { ::pyo3_stub_gen::TypeInfo::locally_defined(#name, #module_tt) };
        let type_output_tokens = type_output_override
            .clone()
            .unwrap_or_else(|| default_output.clone());
        let type_input_tokens = type_input_override
            .clone()
            .unwrap_or(type_output_tokens.clone());

        tokens.append_all(quote! {
            #[automatically_derived]
            impl ::pyo3_stub_gen::PyStubType for #ty {
                fn type_output() -> ::pyo3_stub_gen::TypeInfo {
                    #type_output_tokens
                }
                fn type_input() -> ::pyo3_stub_gen::TypeInfo {
                    #type_input_tokens
                }
            }
        })
    }
}
