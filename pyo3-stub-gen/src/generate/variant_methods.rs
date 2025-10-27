use indexmap::IndexMap;

use crate::generate::{MethodDef, MethodType, Parameter, ParameterDefault, Parameters};
use crate::type_info::{ParameterKind, PyComplexEnumInfo, VariantForm, VariantInfo};
use crate::TypeInfo;

pub(super) fn get_variant_methods(
    _enum_info: &PyComplexEnumInfo,
    info: &VariantInfo,
) -> IndexMap<String, Vec<MethodDef>> {
    if info.is_mapping {
        return IndexMap::new();
    }
    let mut methods: IndexMap<String, Vec<MethodDef>> = IndexMap::new();

    methods
        .entry("__new__".to_string())
        .or_default()
        .push(MethodDef {
            name: "__new__",
            parameters: Parameters::from_infos(info.constr_args),
            r#return: TypeInfo::self_type(),
            doc: "",
            r#type: MethodType::New,
            is_async: false,
            deprecated: None,
            type_ignored: None,
            is_abstract: false,
        });

    if let VariantForm::Tuple = info.form {
        let len_name = "__len__";
        methods
            .entry(len_name.to_string())
            .or_default()
            .push(MethodDef {
                name: len_name,
                parameters: Parameters::new(),
                r#return: TypeInfo::builtin("int"),
                doc: "",
                r#type: MethodType::Instance,
                is_async: false,
                deprecated: None,
                type_ignored: None,
                is_abstract: false,
            });

        let getitem_name = "__getitem__";
        methods
            .entry(getitem_name.to_string())
            .or_default()
            .push(MethodDef {
                name: getitem_name,
                parameters: Parameters {
                    positional_or_keyword: vec![Parameter {
                        name: "key",
                        kind: ParameterKind::PositionalOrKeyword,
                        type_info: TypeInfo::builtin("int"),
                        default: ParameterDefault::None,
                    }],
                    ..Parameters::new()
                },
                r#return: TypeInfo::any(),
                doc: "",
                r#type: MethodType::Instance,
                is_async: false,
                deprecated: None,
                type_ignored: None,
                is_abstract: false,
            });
    }

    methods
}
