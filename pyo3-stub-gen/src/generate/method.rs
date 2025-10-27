use crate::stub_type::ImportRef;
use crate::{generate::*, rule_name::RuleName, type_info::*, TypeInfo};
use itertools::Itertools;
use std::{collections::HashSet, fmt};

pub use crate::type_info::MethodType;

/// Definition of a class method.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDef {
    pub name: &'static str,
    pub parameters: Parameters,
    pub r#return: TypeInfo,
    pub doc: &'static str,
    pub r#type: MethodType,
    pub is_async: bool,
    pub deprecated: Option<DeprecatedInfo>,
    pub type_ignored: Option<IgnoreTarget>,
    pub is_abstract: bool,
}

impl Import for MethodDef {
    fn import(&self) -> HashSet<ImportRef> {
        let mut import = self.r#return.import.clone();
        import.extend(self.parameters.import());
        // Add typing_extensions import if deprecated
        if self.deprecated.is_some() {
            import.insert("typing_extensions".into());
        }
        if self.is_abstract {
            import.insert("abc".into());
        }
        import
    }
}

impl From<&MethodInfo> for MethodDef {
    fn from(info: &MethodInfo) -> Self {
        let mut return_type = (info.r#return)();
        if info.r#type == MethodType::New {
            return_type = TypeInfo::self_type();
        }
        Self {
            name: info.name,
            parameters: Parameters::from_infos(info.parameters),
            r#return: return_type,
            doc: info.doc,
            r#type: info.r#type,
            is_async: info.is_async,
            deprecated: info.deprecated.clone(),
            type_ignored: info.type_ignored,
            is_abstract: info.is_abstract,
        }
    }
}

impl fmt::Display for MethodDef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let indent = indent();
        let async_ = if self.is_async { "async " } else { "" };

        // Add deprecated decorator if present
        if let Some(deprecated) = &self.deprecated {
            writeln!(f, "{indent}{deprecated}")?;
        }

        let params_str = if self.parameters.is_empty() {
            String::new()
        } else {
            format!(", {}", self.parameters)
        };

        match self.r#type {
            MethodType::Static => {
                writeln!(f, "{indent}@staticmethod")?;
                if self.is_abstract {
                    writeln!(f, "{indent}@abc.abstractmethod")?;
                }
                write!(f, "{indent}{async_}def {}({})", self.name, self.parameters)?;
            }
            MethodType::Class | MethodType::New => {
                if self.r#type == MethodType::Class {
                    // new is a classmethod without the decorator
                    writeln!(f, "{indent}@classmethod")?;
                }
                if self.is_abstract {
                    writeln!(f, "{indent}@abc.abstractmethod")?;
                }
                write!(f, "{indent}{async_}def {}(cls{})", self.name, params_str)?;
            }
            MethodType::Instance => {
                if self.is_abstract {
                    writeln!(f, "{indent}@abc.abstractmethod")?;
                }
                write!(f, "{indent}{async_}def {}(self{})", self.name, params_str)?;
            }
        }
        write!(f, " -> {}:", self.r#return)?;

        // Calculate type: ignore comment once
        let type_ignore_comment = if let Some(target) = &self.type_ignored {
            match target {
                IgnoreTarget::All => Some("  # type: ignore".to_string()),
                IgnoreTarget::Specified(rules) => {
                    let rules_str = rules
                        .iter()
                        .map(|r| {
                            let result = r.parse::<RuleName>().unwrap();
                            if let RuleName::Custom(custom) = &result {
                                log::warn!("Unknown custom rule name '{custom}' used in type ignore. Ensure this is intended.");
                            }
                            result
                        })
                        .join(",");
                    Some(format!("  # type: ignore[{rules_str}]"))
                }
            }
        } else {
            None
        };

        let doc = self.doc;
        if !doc.is_empty() {
            // Add type: ignore comment for methods with docstrings
            if let Some(comment) = &type_ignore_comment {
                write!(f, "{comment}")?;
            }
            writeln!(f)?;
            let double_indent = format!("{indent}{indent}");
            docstring::write_docstring(f, self.doc, &double_indent)?;
        } else {
            write!(f, " ...")?;
            // Add type: ignore comment for methods without docstrings
            if let Some(comment) = &type_ignore_comment {
                write!(f, "{comment}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstract_instance_method_renders_decorator() {
        let method = MethodDef {
            name: "do_work",
            parameters: Parameters::new(),
            r#return: TypeInfo::builtin("int"),
            doc: "",
            r#type: MethodType::Instance,
            is_async: false,
            deprecated: None,
            type_ignored: None,
            is_abstract: true,
        };
        let rendered = method.to_string();
        assert!(rendered.contains("@abc.abstractmethod"));
        assert!(rendered.contains("def do_work(self"));
    }
}
