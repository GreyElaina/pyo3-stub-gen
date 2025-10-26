use super::{extract_documents, parse_pyo3_attrs, util::quote_option, Attr, StubType};
use crate::gen_stub::variant::VariantInfo;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{parse_quote, Error, ItemEnum, Result, Type};

pub struct PyComplexEnumInfo {
    pyclass_name: String,
    enum_type: Type,
    module: Option<String>,
    variants: Vec<VariantInfo>,
    doc: String,
}

impl From<&PyComplexEnumInfo> for StubType {
    fn from(info: &PyComplexEnumInfo) -> Self {
        let PyComplexEnumInfo {
            pyclass_name,
            module,
            enum_type,
            variants,
            ..
        } = info;
        let union_terms: Vec<_> = variants
            .iter()
            .map(|variant| union_type_for_variant(pyclass_name, variant))
            .collect();
        let type_union = (!union_terms.is_empty()).then(|| {
            let mut iter = union_terms.into_iter();
            let first = iter.next().unwrap();
            iter.fold(first, |acc, expr| quote! { (#acc) | (#expr) })
        });
        Self {
            ty: enum_type.clone(),
            name: pyclass_name.clone(),
            module: module.clone(),
            type_input_override: type_union.clone(),
            type_output_override: type_union,
        }
    }
}

fn union_type_for_variant(enum_name: &str, variant: &VariantInfo) -> TokenStream2 {
    match variant.form {
        crate::gen_stub::variant::VariantForm::Tuple if variant.constr_args.len() == 1 => {
            let arg = &variant.constr_args[0];
            match &arg.r#type {
                crate::gen_stub::util::TypeOrOverride::RustType { r#type } => {
                    let ty = r#type;
                    quote! { <#ty as ::pyo3_stub_gen::PyStubType>::type_input() }
                }
                crate::gen_stub::util::TypeOrOverride::OverrideType { type_repr, .. } => {
                    quote! {
                        ::pyo3_stub_gen::TypeInfo {
                            name: #type_repr.to_string(),
                            import: ::std::collections::HashSet::new(),
                        }
                    }
                }
            }
        }
        _ => {
            let variant_name = format!("{enum_name}.{}", variant.pyclass_name);
            quote! { ::pyo3_stub_gen::TypeInfo::unqualified(#variant_name) }
        }
    }
}

impl TryFrom<ItemEnum> for PyComplexEnumInfo {
    type Error = Error;

    fn try_from(item: ItemEnum) -> Result<Self> {
        let ItemEnum {
            variants,
            attrs,
            ident,
            ..
        } = item;

        let doc = extract_documents(&attrs).join("\n");
        let mut pyclass_name = None;
        let mut module = None;
        let mut renaming_rule = None;
        let mut bases = Vec::new();
        for attr in parse_pyo3_attrs(&attrs)? {
            match attr {
                Attr::Name(name) => pyclass_name = Some(name),
                Attr::Module(name) => module = Some(name),
                Attr::RenameAll(name) => renaming_rule = Some(name),
                Attr::Extends(typ) => bases.push(typ),
                _ => {}
            }
        }

        let enum_type = parse_quote!(#ident);
        let pyclass_name = pyclass_name.unwrap_or_else(|| ident.clone().to_string());

        let mut items = Vec::new();
        for variant in variants {
            items.push(VariantInfo::from_variant(variant, &renaming_rule)?)
        }

        Ok(Self {
            doc,
            enum_type,
            pyclass_name,
            module,
            variants: items,
        })
    }
}

impl ToTokens for PyComplexEnumInfo {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Self {
            pyclass_name,
            enum_type,
            variants,
            doc,
            module,
            ..
        } = self;
        let module = quote_option(module);

        tokens.append_all(quote! {
            ::pyo3_stub_gen::type_info::PyComplexEnumInfo {
                pyclass_name: #pyclass_name,
                enum_id: std::any::TypeId::of::<#enum_type>,
                variants: &[ #( #variants ),* ],
                module: #module,
                doc: #doc,
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use syn::parse_str;

    #[test]
    fn test_complex_enum() -> Result<()> {
        let input: ItemEnum = parse_str(
            r#"
            #[pyclass(mapping, module = "my_module", name = "Placeholder")]
            #[derive(
                Debug, Clone, PyNeg, PyAdd, PySub, PyMul, PyDiv, PyMod, PyPow, PyCmp, PyIndex, PyPrint,
            )]
            pub enum PyPlaceholder {
                #[pyo3(name="Name")]
                name(String),
                #[pyo3(constructor = (_0, _1=1.0))]
                twonum(i32,f64),
                ndim{count: usize},
                description,
            }
            "#,
        )?;
        let out = PyComplexEnumInfo::try_from(input)?.to_token_stream();
        insta::assert_snapshot!(format_as_value(out), @r###"
        ::pyo3_stub_gen::type_info::PyComplexEnumInfo {
            pyclass_name: "Placeholder",
            enum_id: std::any::TypeId::of::<PyPlaceholder>,
            variants: &[
                ::pyo3_stub_gen::type_info::VariantInfo {
                    pyclass_name: "Name",
                    fields: &[
                        ::pyo3_stub_gen::type_info::MemberInfo {
                            name: "_0",
                            r#type: <String as ::pyo3_stub_gen::PyStubType>::type_output,
                            doc: "",
                            default: None,
                            deprecated: None,
                            item: false,
                        },
                    ],
                    module: None,
                    doc: "",
                    form: &pyo3_stub_gen::type_info::VariantForm::Tuple,
                    constr_args: &[
                        ::pyo3_stub_gen::type_info::ParameterInfo {
                            name: "_0",
                            kind: ::pyo3_stub_gen::type_info::ParameterKind::PositionalOrKeyword,
                            type_info: <String as ::pyo3_stub_gen::PyStubType>::type_input,
                            default: ::pyo3_stub_gen::type_info::ParameterDefault::None,
                        },
                    ],
                    is_mapping: false,
                },
                ::pyo3_stub_gen::type_info::VariantInfo {
                    pyclass_name: "twonum",
                    fields: &[
                        ::pyo3_stub_gen::type_info::MemberInfo {
                            name: "_0",
                            r#type: <i32 as ::pyo3_stub_gen::PyStubType>::type_output,
                            doc: "",
                            default: None,
                            deprecated: None,
                            item: false,
                        },
                        ::pyo3_stub_gen::type_info::MemberInfo {
                            name: "_1",
                            r#type: <f64 as ::pyo3_stub_gen::PyStubType>::type_output,
                            doc: "",
                            default: None,
                            deprecated: None,
                            item: false,
                        },
                    ],
                    module: None,
                    doc: "",
                    form: &pyo3_stub_gen::type_info::VariantForm::Tuple,
                    constr_args: &[
                        ::pyo3_stub_gen::type_info::ParameterInfo {
                            name: "_0",
                            kind: ::pyo3_stub_gen::type_info::ParameterKind::PositionalOrKeyword,
                            type_info: <i32 as ::pyo3_stub_gen::PyStubType>::type_input,
                            default: ::pyo3_stub_gen::type_info::ParameterDefault::None,
                        },
                        ::pyo3_stub_gen::type_info::ParameterInfo {
                            name: "_1",
                            kind: ::pyo3_stub_gen::type_info::ParameterKind::PositionalOrKeyword,
                            type_info: <f64 as ::pyo3_stub_gen::PyStubType>::type_input,
                            default: ::pyo3_stub_gen::type_info::ParameterDefault::Expr({
                                fn _fmt() -> String {
                                    let v: f64 = 1.0;
                                    ::pyo3_stub_gen::util::fmt_py_obj(v)
                                }
                                _fmt
                            }),
                        },
                    ],
                    is_mapping: false,
                },
                ::pyo3_stub_gen::type_info::VariantInfo {
                    pyclass_name: "ndim",
                    fields: &[
                        ::pyo3_stub_gen::type_info::MemberInfo {
                            name: "count",
                            r#type: <usize as ::pyo3_stub_gen::PyStubType>::type_output,
                            doc: "",
                            default: None,
                            deprecated: None,
                            item: false,
                        },
                    ],
                    module: None,
                    doc: "",
                    form: &pyo3_stub_gen::type_info::VariantForm::Struct,
                    constr_args: &[
                        ::pyo3_stub_gen::type_info::ParameterInfo {
                            name: "count",
                            kind: ::pyo3_stub_gen::type_info::ParameterKind::PositionalOrKeyword,
                            type_info: <usize as ::pyo3_stub_gen::PyStubType>::type_input,
                            default: ::pyo3_stub_gen::type_info::ParameterDefault::None,
                        },
                    ],
                    is_mapping: false,
                },
                ::pyo3_stub_gen::type_info::VariantInfo {
                    pyclass_name: "description",
                    fields: &[],
                    module: None,
                    doc: "",
                    form: &pyo3_stub_gen::type_info::VariantForm::Unit,
                    constr_args: &[],
                    is_mapping: false,
                },
            ],
            module: Some("my_module"),
            doc: "",
        }
        "###);
        Ok(())
    }

    fn format_as_value(tt: TokenStream2) -> String {
        let ttt = quote! { const _: () = #tt; };
        let formatted = prettyplease::unparse(&syn::parse_file(&ttt.to_string()).unwrap());
        formatted
            .trim()
            .strip_prefix("const _: () = ")
            .unwrap()
            .strip_suffix(';')
            .unwrap()
            .to_string()
    }
}
