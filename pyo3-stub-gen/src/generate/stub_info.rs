#[cfg(test)]
use crate::stub_type::self_import_strategy;
use crate::{
    generate::*,
    pyproject::PyProject,
    stub_type::{set_self_import_strategy, SelfImportStrategy},
    type_info::*,
};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::*,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StubInfo {
    pub modules: BTreeMap<String, Module>,
    pub python_root: PathBuf,
}

fn configure_self_import_strategy_from_requires_python(spec: Option<&str>) {
    use SelfImportStrategy::{Typing, TypingExtensions};

    if let Some(min_version) = spec.and_then(parse_minimum_python_version) {
        let strategy = if min_version.0 > 3 || (min_version.0 == 3 && min_version.1 >= 11) {
            Typing
        } else {
            TypingExtensions
        };
        set_self_import_strategy(strategy);
    } else {
        set_self_import_strategy(Typing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub_type::{set_self_import_strategy, SelfImportStrategy};

    #[test]
    fn parses_minimum_python_version() {
        assert_eq!(parse_minimum_python_version(">=3.10"), Some((3, 10)));
        assert_eq!(parse_minimum_python_version(">=3.8, <3.12"), Some((3, 8)));
        assert_eq!(parse_minimum_python_version("~=3.11.0"), Some((3, 11)));
        assert_eq!(parse_minimum_python_version(""), None);
        assert_eq!(parse_minimum_python_version(">=3"), Some((3, 0)));
    }

    #[test]
    fn configure_strategy_defaults_to_typing_when_unspecified() {
        set_self_import_strategy(SelfImportStrategy::TypingExtensions);
        configure_self_import_strategy_from_requires_python(None);
        assert_eq!(self_import_strategy(), SelfImportStrategy::Typing);
    }

    #[test]
    fn configure_strategy_prefers_typing_extensions_below_311() {
        set_self_import_strategy(SelfImportStrategy::Typing);
        configure_self_import_strategy_from_requires_python(Some(">=3.10"));
        assert_eq!(self_import_strategy(), SelfImportStrategy::TypingExtensions);
    }

    #[test]
    fn configure_strategy_prefers_typing_from_311_onwards() {
        set_self_import_strategy(SelfImportStrategy::TypingExtensions);
        configure_self_import_strategy_from_requires_python(Some(">=3.11"));
        assert_eq!(self_import_strategy(), SelfImportStrategy::Typing);
    }
}

fn parse_minimum_python_version(spec: &str) -> Option<(u8, u8)> {
    let mut minimum: Option<(u8, u8)> = None;
    for token in spec.split(|c| c == ',' || c == ' ') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        let candidate = if let Some(rest) = token.strip_prefix(">=") {
            parse_python_version_fragment(rest)
        } else if let Some(rest) = token.strip_prefix("==") {
            parse_python_version_fragment(rest)
        } else if let Some(rest) = token.strip_prefix("~=") {
            parse_python_version_fragment(rest)
        } else {
            None
        };

        if let Some(version) = candidate {
            minimum = Some(match minimum {
                Some(current) => max_version(current, version),
                None => version,
            });
        }
    }
    minimum
}

fn parse_python_version_fragment(fragment: &str) -> Option<(u8, u8)> {
    let cleaned = fragment.trim().trim_start_matches('=').trim();
    let cleaned = cleaned.trim_start_matches('v');
    let cleaned = cleaned.trim_end_matches(".*");
    let cleaned = cleaned.trim_end_matches('*');

    let mut parts = cleaned.split('.');
    let major: u8 = parts.next()?.parse().ok()?;
    let mut minor_part = parts.next().unwrap_or("0").trim();
    if let Some(idx) = minor_part.chars().position(|ch| !matches!(ch, '0'..='9')) {
        minor_part = &minor_part[..idx];
    }
    let minor: u8 = if minor_part.is_empty() {
        0
    } else {
        minor_part.parse().ok()?
    };
    Some((major, minor))
}

fn max_version(a: (u8, u8), b: (u8, u8)) -> (u8, u8) {
    if b.0 > a.0 || (b.0 == a.0 && b.1 > a.1) {
        b
    } else {
        a
    }
}

impl StubInfo {
    /// Initialize [StubInfo] from a `pyproject.toml` file in `CARGO_MANIFEST_DIR`.
    /// This is automatically set up by the [crate::define_stub_info_gatherer] macro.
    pub fn from_pyproject_toml(path: impl AsRef<Path>) -> Result<Self> {
        let pyproject = PyProject::parse_toml(path)?;
        Ok(StubInfoBuilder::from_pyproject_toml(pyproject).build())
    }

    /// Initialize [StubInfo] with a specific module name and project root.
    /// This must be placed in your PyO3 library crate, i.e. the same crate where [inventory::submit]ted,
    /// not in the `gen_stub` executables due to [inventory]'s mechanism.
    pub fn from_project_root(default_module_name: String, project_root: PathBuf) -> Result<Self> {
        Ok(StubInfoBuilder::from_project_root(default_module_name, project_root).build())
    }

    pub fn generate(&self) -> Result<()> {
        for (name, module) in self.modules.iter() {
            // Convert dashes to underscores for Python compatibility
            let normalized_name = name.replace("-", "_");
            let path = normalized_name.replace(".", "/");
            let dest = if module.submodules.is_empty() {
                self.python_root.join(format!("{path}.pyi"))
            } else {
                self.python_root.join(path).join("__init__.pyi")
            };

            let dir = dest.parent().context("Cannot get parent directory")?;
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }

            let mut f = fs::File::create(&dest)?;
            write!(f, "{module}")?;
            log::info!(
                "Generate stub file of a module `{name}` at {dest}",
                dest = dest.display()
            );
        }
        Ok(())
    }
}

struct StubInfoBuilder {
    modules: BTreeMap<String, Module>,
    default_module_name: String,
    python_root: PathBuf,
}

impl StubInfoBuilder {
    fn from_pyproject_toml(pyproject: PyProject) -> Self {
        configure_self_import_strategy_from_requires_python(
            pyproject.project.requires_python.as_deref(),
        );
        StubInfoBuilder::from_project_root(
            pyproject.module_name().to_string(),
            pyproject
                .python_source()
                .unwrap_or(PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())),
        )
    }

    fn from_project_root(default_module_name: String, project_root: PathBuf) -> Self {
        Self {
            modules: BTreeMap::new(),
            default_module_name,
            python_root: project_root,
        }
    }

    fn get_module(&mut self, name: Option<&str>) -> &mut Module {
        let name = name.unwrap_or(&self.default_module_name).to_string();
        let module = self.modules.entry(name.clone()).or_default();
        module.name = name;
        module.default_module_name = self.default_module_name.clone();
        module
    }

    fn register_submodules(&mut self) {
        let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for module in self.modules.keys() {
            let path = module.split('.').collect::<Vec<_>>();
            let n = path.len();
            if n <= 1 {
                continue;
            }
            map.entry(path[..n - 1].join("."))
                .or_default()
                .insert(path[n - 1].to_string());
        }
        for (parent, children) in map {
            if let Some(module) = self.modules.get_mut(&parent) {
                module.submodules.extend(children);
            }
        }
    }

    fn add_class(&mut self, info: &PyClassInfo) {
        self.get_module(info.module)
            .class
            .insert((info.struct_id)(), ClassDef::from(info));
    }

    fn add_complex_enum(&mut self, info: &PyComplexEnumInfo) {
        self.get_module(info.module)
            .class
            .insert((info.enum_id)(), ClassDef::from(info));
    }

    fn add_enum(&mut self, info: &PyEnumInfo) {
        self.get_module(info.module)
            .enum_
            .insert((info.enum_id)(), EnumDef::from(info));
    }

    fn add_function(&mut self, info: &PyFunctionInfo) {
        let target = self
            .get_module(info.module)
            .function
            .entry(info.name)
            .or_default();
        target.push(FunctionDef::from(info));
    }

    fn add_variable(&mut self, info: &PyVariableInfo) {
        self.get_module(Some(info.module))
            .variables
            .insert(info.name, VariableDef::from(info));
    }

    fn add_module_doc(&mut self, info: &ModuleDocInfo) {
        self.get_module(Some(info.module)).doc = (info.doc)();
    }

    fn add_methods(&mut self, info: &PyMethodsInfo) {
        let struct_id = (info.struct_id)();
        for module in self.modules.values_mut() {
            if let Some(entry) = module.class.get_mut(&struct_id) {
                for attr in info.attrs {
                    entry.attrs.push(MemberDef {
                        name: attr.name,
                        r#type: (attr.r#type)(),
                        doc: attr.doc,
                        default: attr.default.map(|f| f()),
                        deprecated: attr.deprecated.clone(),
                        is_abstract: false,
                    });
                }
                for getter in info.getters {
                    entry
                        .getter_setters
                        .entry(getter.name.to_string())
                        .or_default()
                        .0 = Some(MemberDef {
                        name: getter.name,
                        r#type: (getter.r#type)(),
                        doc: getter.doc,
                        default: getter.default.map(|f| f()),
                        deprecated: getter.deprecated.clone(),
                        is_abstract: getter.is_abstract,
                    });
                    if getter.is_abstract {
                        entry.mark_abstract();
                    }
                }
                for setter in info.setters {
                    entry
                        .getter_setters
                        .entry(setter.name.to_string())
                        .or_default()
                        .1 = Some(MemberDef {
                        name: setter.name,
                        r#type: (setter.r#type)(),
                        doc: setter.doc,
                        default: setter.default.map(|f| f()),
                        deprecated: setter.deprecated.clone(),
                        is_abstract: setter.is_abstract,
                    });
                    if setter.is_abstract {
                        entry.mark_abstract();
                    }
                }
                for method in info.methods {
                    let method_def = MethodDef::from(method);
                    if method_def.is_abstract {
                        entry.mark_abstract();
                    }
                    entry
                        .methods
                        .entry(method_def.name.to_string())
                        .or_default()
                        .push(method_def);
                }
                return;
            } else if let Some(entry) = module.enum_.get_mut(&struct_id) {
                for attr in info.attrs {
                    entry.attrs.push(MemberDef {
                        name: attr.name,
                        r#type: (attr.r#type)(),
                        doc: attr.doc,
                        default: attr.default.map(|f| f()),
                        deprecated: attr.deprecated.clone(),
                        is_abstract: false,
                    });
                }
                for getter in info.getters {
                    entry.getters.push(MemberDef {
                        name: getter.name,
                        r#type: (getter.r#type)(),
                        doc: getter.doc,
                        default: getter.default.map(|f| f()),
                        deprecated: getter.deprecated.clone(),
                        is_abstract: getter.is_abstract,
                    });
                }
                for setter in info.setters {
                    entry.setters.push(MemberDef {
                        name: setter.name,
                        r#type: (setter.r#type)(),
                        doc: setter.doc,
                        default: setter.default.map(|f| f()),
                        deprecated: setter.deprecated.clone(),
                        is_abstract: setter.is_abstract,
                    });
                }
                for method in info.methods {
                    entry.methods.push(MethodDef::from(method))
                }
                return;
            }
        }
        unreachable!("Missing struct_id/enum_id = {:?}", struct_id);
    }

    fn build(mut self) -> StubInfo {
        for info in inventory::iter::<PyClassInfo> {
            self.add_class(info);
        }
        for info in inventory::iter::<PyComplexEnumInfo> {
            self.add_complex_enum(info);
        }
        for info in inventory::iter::<PyEnumInfo> {
            self.add_enum(info);
        }
        for info in inventory::iter::<PyFunctionInfo> {
            self.add_function(info);
        }
        for info in inventory::iter::<PyVariableInfo> {
            self.add_variable(info);
        }
        for info in inventory::iter::<ModuleDocInfo> {
            self.add_module_doc(info);
        }
        for info in inventory::iter::<PyMethodsInfo> {
            self.add_methods(info);
        }
        self.register_submodules();
        StubInfo {
            modules: self.modules,
            python_root: self.python_root,
        }
    }
}
