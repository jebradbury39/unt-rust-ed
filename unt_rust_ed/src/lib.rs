mod error;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::ops::Deref;
use std::collections::{HashSet, HashMap};

use extism::{Manifest, Plugin, Wasm, ToBytes, FromBytes};
pub use extism_manifest::MemoryOptions;
pub use extism_convert::Json;

use tempfile::TempDir;

use syn::Token;
use syn::token::{Paren, Bracket};
use syn::punctuated::Punctuated;
use syn::__private::Span;
use syn::__private::ToTokens;

use crate::error::*;

pub trait ExportedHostType {
    fn typename() -> &'static str;
    fn typedef_as_string() -> &'static str;
}

/// Returns the number of bytes in a page of memory.
/// This is useful for determining how many pages to give to an untrusted project
pub fn get_page_size() -> usize {
    return page_size::get();
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub enum WasmCompileTarget {
    #[default]
    Lightweight,
    Wasi,
}

impl WasmCompileTarget {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Lightweight => "wasm32-unknown-unknown",
            Self::Wasi => "wasm32-wasi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UntrustedRustProject {
    rust_code: String,
    runtime_memory_options: MemoryOptions,
    runtime_timeout_ms: Option<u64>,
    target: WasmCompileTarget,
    /// map type name to typedef
    exported_host_types: HashMap<String, String>,
    /// type names to replace during compilation. May contain module separators ('::')
    sdk_types: HashSet<String>,
}

impl UntrustedRustProject {

    pub fn new(rust_code: &str) -> Self {
        Self {
            rust_code: rust_code.into(),
            runtime_memory_options: MemoryOptions::default(),
            runtime_timeout_ms: None,
            target: WasmCompileTarget::default(),
            exported_host_types: HashMap::new(),
            sdk_types: HashSet::new(),          
        }
    }

    pub fn with_max_memory_bytes(mut self, num_bytes: usize) -> Self {
        let page_size = get_page_size();
        let num_pages = if num_bytes % page_size == 0 {
            num_bytes / page_size
        } else {
            num_bytes / page_size + 1
        };
        self.runtime_memory_options = MemoryOptions {
            max_pages: Some(num_pages as u32),
        };
        self
    }

    pub fn with_target(mut self, target: WasmCompileTarget) -> Self {
        self.target = target;
        self
    }

    /// These are "plain-old-data" types, and they exist mainly as a convenience. For more flexibility, use an sdk crate and tag the types as sdk types
    pub fn with_exported_host_type<T: ExportedHostType>(mut self) -> Self {
        self.exported_host_types.insert(T::typename().to_string(), T::typedef_as_string().to_string());
        self
    }

    /// These types are imported by the `rust_code`, and we need to know to 'jsonify' them
    pub fn with_sdk_type(mut self, typename: &str) -> Self {
        self.sdk_types.insert(typename.to_string());
        self
    }

    pub fn with_runtime_timeout_ms(mut self, ms: u64) -> Self {
        self.runtime_timeout_ms = Some(ms);
        self
    }

    pub fn with_runtime_memory_options(mut self, mem_opts: MemoryOptions) -> Self {
        self.runtime_memory_options = mem_opts;
        self
    }

    /// Converts the modules into compiled modules containing WASM
    pub fn compile(&self) -> Result<CompiledUntrustedRustProject> {
        // create temp directory
        let tmp_cargo_dir = TempDir::new().map_err(|err| UntRustedError::IoError {
   resource: "TempDir".into(),
   err,
   })?;

        // setup cargo project by creating Cargo.toml in temp directory
        let cargo_toml_path = tmp_cargo_dir.path().join("Cargo.toml");

        Self::write_cargo_toml(cargo_toml_path)?;

        // mkdir 'src' under tmp_cargo_dir
        let cargo_src_path = tmp_cargo_dir.path().join("src");
        fs::create_dir(&cargo_src_path).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", cargo_src_path),
   err,
   })?;

        // create modules in src/lib.rs file in temp directory.
        // For every exported function, create a copy with the module underscore prefix, and tag it as wasm-exported
        // Also perform checks such as ensuring that other functions do not start with any of the module names and an underscore
        self.write_rust_code_to_cargo_dir(&cargo_src_path)?;

        // compile project to wasm by spawning cargo as a subprocess
        let built_wasm_file_path: PathBuf = self.cargo_build_to_wasm(&tmp_cargo_dir)?;

        let built_wasm_bytes: Vec<u8> = fs::read(&built_wasm_file_path).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", built_wasm_file_path),
   err,
   })?;

      let wasm = Wasm::data(built_wasm_bytes);

        let manifest = Manifest::new(vec![wasm])
            .disallow_all_hosts()
            .with_memory_options(self.runtime_memory_options.clone());

        let manifest = if let Some(runtime_timeout_ms) = self.runtime_timeout_ms {
            manifest.with_timeout(Duration::from_millis(runtime_timeout_ms))
        } else {
            manifest
        };

        let plugin = Plugin::new(&manifest, [], self.target == WasmCompileTarget::Wasi)?;

        return Ok(CompiledUntrustedRustProject {
            plugin,
        });
    }

    fn write_cargo_toml<P: AsRef<Path>>(cargo_toml_path: P) -> Result<()> {
      let mut cargo_toml_file = File::create(&cargo_toml_path).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", cargo_toml_path.as_ref()),
   err,
   })?;

        let content = "[package]
    name = \"test-wasm\"
    version = \"0.1.0\"
    edition = \"2021\"

    [lib]
    crate-type = [\"cdylib\"]

    [dependencies]
    extism-pdk = \"1.0.0-rc1\"
    serde = { version = \"1.0\", features = [\"derive\"] }";

        cargo_toml_file.write_all(content.as_bytes()).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", cargo_toml_path.as_ref()),
   err,
   })?;

        return Ok(());
    }

    fn write_rust_code_to_cargo_dir<P: AsRef<Path>>(&self, cargo_src_path: P) -> Result<()> {

        let mut rust_code = self.rust_code.clone();

        // add exported type defs
        for (_, typedef) in &self.exported_host_types {
            rust_code.push('\n');
            rust_code.push_str("#[derive(Debug, serde::Serialize, serde::Deserialize)]\n");
            rust_code.push_str(typedef);
        }

        let mut ast: syn::File = syn::parse_file(&rust_code)?;

        // add validator

        // update the ast
        let use_extism_item = syn::Item::Use(syn::ItemUse {
            attrs: Vec::new(),
            vis: syn::Visibility::Inherited,
            use_token: Token![use](Span::call_site()),
            leading_colon: None,
            tree: syn::UseTree::Path(syn::UsePath {
                ident: syn::Ident::new("extism_pdk", Span::call_site()),
                colon2_token: Token![::](Span::call_site()),
                tree: Box::new(syn::UseTree::Glob(syn::UseGlob {
                    star_token: Token![*](Span::call_site()),
                })),
            }),
            semi_token: Token![;](Span::call_site()),
        });
        ast.items.insert(0, use_extism_item);

        let mut jsonify_typenames = HashSet::new();
        for (typename, _) in &self.exported_host_types {
            jsonify_typenames.insert(typename.clone());
        }

        for typename in &self.sdk_types {
            jsonify_typenames.insert(typename.clone());
        }

        Self::tag_functions_for_export(&mut ast.items, "", &jsonify_typenames)?;

        let new_rust_code = prettyplease::unparse(&ast);

        let lib_rs_path = cargo_src_path.as_ref().join("lib.rs");
        let mut lib_rs_file = File::create(&lib_rs_path).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", lib_rs_path),
   err,
   })?;

        lib_rs_file.write_all(new_rust_code.as_bytes()).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", lib_rs_path),
   err,
   })?;

        return Ok(());
    }

    fn tag_functions_for_export(items: &mut Vec<syn::Item>, mod_names: &str, jsonify_typenames: &HashSet<String>) -> Result<()> {
        let mut item_idx: usize = 0;
        while item_idx < items.len() {

            match &mut items[item_idx] {
                syn::Item::Mod(item_mod) => if let Some(content) = &mut item_mod.content {
                    let item_mod_name = item_mod.ident.to_string();

                    let new_mod_names = if mod_names.is_empty() {
                        item_mod_name
                    } else {
                        if item_mod_name.is_empty() {
                            panic!("mod name should not be empty");
                        }
                        format!("{}__{}", mod_names, item_mod_name)
                    };

                    Self::tag_functions_for_export(&mut content.1, &new_mod_names, jsonify_typenames)?;
                },
                syn::Item::Fn(item_fn) => {
                    if item_fn.vis != syn::Visibility::Public(Token![pub](Span::call_site())) {
                        continue;
                    }

                    // export it by creating a clone of the function
                    let new_fn_name = format!("{}__{}", mod_names, item_fn.sig.ident.to_string());

                    let mut new_fn_sig = item_fn.sig.clone();
                    new_fn_sig.ident = syn::Ident::new(&new_fn_name, Span::call_site());

                    // jsonify the input params of the new function
                    for param in &mut new_fn_sig.inputs {
                        match param {
                            syn::FnArg::Typed(pat_type) => {
                                if Self::can_jsonify_type(jsonify_typenames, &pat_type.ty) {
                                    pat_type.pat = Box::new(syn::Pat::TupleStruct(syn::PatTupleStruct {
                                        attrs: Vec::new(),
                                        qself: None,
                                        path: Self::create_simple_path(&["Json"]),
                                        paren_token: Paren::default(),
                                        elems: {
                                            let mut elems = Punctuated::new();
                                            elems.push((*pat_type.pat).clone());
                                            elems
                                        }
                                    }));

                                    pat_type.ty = Box::new(Self::wrap_type("Json", &[&pat_type.ty]));
                                }
                            },
                            _ => continue,
                        }
                    }

                    // jsonify the return type of the new function
                    let can_jsonify_ret_ty = match &item_fn.sig.output {
                        syn::ReturnType::Type(_, ty) => {
                            let can_jsonify_ret_ty = Self::can_jsonify_type(jsonify_typenames, ty);
                            let new_ret_ty = if can_jsonify_ret_ty {
                                let new_ret_ty = Self::wrap_type("Json", &[ty]);
                                Self::wrap_type("FnResult", &[&new_ret_ty])
                            } else {
                                Self::wrap_type("FnResult", &[ty])
                            };

                            new_fn_sig.output = syn::ReturnType::Type(Token![->](Span::call_site()), Box::new(new_ret_ty));
                            can_jsonify_ret_ty
                        },
                        _ => false,
                    };

                    let mut call_old_fn_args = Punctuated::new();
                    for param in &item_fn.sig.inputs {
                        match param {
                            syn::FnArg::Typed(pat_type) => {
                                let param_name: String = Self::get_param_name(pat_type)?;

                                call_old_fn_args.push(syn::Expr::Path(syn::ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path: Self::create_simple_path(&[&param_name]),
                                }));
                            },
                            _ => return Err(UntRustedError::UnsupportedFnArg(format!("{:?}", param))),
                        }
                    }

                    let old_fn_call = syn::Expr::Call(syn::ExprCall {
                        attrs: Vec::new(),
                        func: Box::new(syn::Expr::Path(syn::ExprPath {
                            attrs: Vec::new(),
                            qself: None,
                            path: syn::Path {
                                leading_colon: None,
                                segments: {
                                    let mut segments = Punctuated::new();
                                    segments.push(syn::PathSegment {
                                        ident: item_fn.sig.ident.clone(),
                                        arguments: syn::PathArguments::None,
                                    });
                                    segments
                                },
                            }
                        })),
                        paren_token: Paren::default(),
                        args: call_old_fn_args,
                    });

                    let ok_wrapper_call = if can_jsonify_ret_ty {
                        let json_wrapper_call_expr = Self::create_call_expr("Json", &[&old_fn_call]);
                        syn::Stmt::Expr(Self::create_call_expr("Ok", &[&json_wrapper_call_expr]), None)
                    } else {
                        syn::Stmt::Expr(Self::create_call_expr("Ok", &[&old_fn_call]), None)
                    };

                    let mut new_fn_attrs = item_fn.attrs.clone();
                    new_fn_attrs.push(syn::Attribute {
                        pound_token: Token![#](Span::call_site()),
                        style: syn::AttrStyle::Outer,
                        bracket_token: Bracket::default(),
                        meta: syn::Meta::Path(Self::create_simple_path(&["plugin_fn"])),
                    });

                    let new_fn_item = syn::Item::Fn(syn::ItemFn {
                        attrs: new_fn_attrs,
                        vis: item_fn.vis.clone(),
                        sig: new_fn_sig,
                        block: Box::new(syn::Block {
                            brace_token: item_fn.block.brace_token.clone(),
                            stmts: vec![ok_wrapper_call],
                        }),
                    });

                    items.insert(item_idx + 1, new_fn_item);
                    item_idx += 1;
                },
                _ => (),
            }

            item_idx += 1;
        }

        return Ok(());
    }

    fn can_jsonify_type(jsonify_typenames: &HashSet<String>, ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let path_as_str = path.to_token_stream().to_string();

                jsonify_typenames.contains(&path_as_str)
            }
            _ => false,
        }
    }

    fn create_simple_path(pathname: &[&str]) -> syn::Path {
        syn::Path {
            leading_colon: None,
            segments: {
                let mut segments = Punctuated::new();
                for segment in pathname {
                    segments.push(syn::PathSegment {
                        ident: syn::Ident::new(segment, Span::call_site()),
                        arguments: syn::PathArguments::None,
                    });
                }
                segments
            },
        }
    }

    fn create_call_expr(fn_name: &str, args: &[&syn::Expr]) -> syn::Expr {
        syn::Expr::Call(syn::ExprCall {
            attrs: Vec::new(),
            func: Box::new(syn::Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path: Self::create_simple_path(&[fn_name]),
            })),
            paren_token: Paren::default(),
            args: {
                let mut new_args = Punctuated::new();
                for arg in args {
                    new_args.push((*arg).clone());
                }
                new_args
            }
        })
    }

    fn wrap_type(outer_type: &str, inner_types: &[&syn::Type]) -> syn::Type {
        syn::Type::Path(syn::TypePath {
            qself: None,
            path: syn::Path {
                leading_colon: None,
                segments: {
                    let mut segments = Punctuated::new();
                    segments.push(syn::PathSegment {
                        ident: syn::Ident::new(outer_type, Span::call_site()),
                        arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                            colon2_token: None,
                            lt_token: Token![<](Span::call_site()),
                            args: {
                                let mut args = Punctuated::new();
                                for inner_type in inner_types {
                                    args.push(syn::GenericArgument::Type((*inner_type).clone()));
                                }
                                args
                            },
                            gt_token: Token![>](Span::call_site()),
                        })
                    });
                    segments
                }
            }
        })
    }

    fn get_param_name(pat_type: &syn::PatType) -> Result<String> {
        let name = match pat_type.pat.deref() {
            syn::Pat::Ident(pat_ident) => {
                pat_ident.ident.to_string()
            },
            _ => return Err(UntRustedError::UnsupportedParamName(format!("{:?}", pat_type))),
        };

        return Ok(name);
    }

    fn cargo_build_to_wasm<P: AsRef<Path>>(&self, cargo_dir: P) -> Result<PathBuf> {
        let cargo_output = Command::new("cargo")
            .args(["build", "--target", self.target.as_str(), "--release"])
            .current_dir(&cargo_dir)
            .output().map_err(|err| UntRustedError::IoError {
   resource: "cargo build".into(),
   err,
   })?;

        // parse cargo output, find target
        //println!("cargo build output:\n{:?}", cargo_output);

        if !cargo_output.status.success() {
            let stdout_str = String::from_utf8_lossy(&cargo_output.stdout);
            let stderr_str = String::from_utf8_lossy(&cargo_output.stderr);

            let need_target_err = format!("note: the `{}` target may not be installed", self.target.as_str());
            if stdout_str.contains(&need_target_err) || stderr_str.contains(&need_target_err) {
                return Err(UntRustedError::MissingCargoTargetInstallation(self.target.as_str().into()));
            }

            // unknown error
            return Err(UntRustedError::UnknownCargoError(stdout_str.into(), stderr_str.into()));
        }

        return Ok(cargo_dir.as_ref().join("target").join(self.target.as_str()).join("release/test_wasm.wasm"));
    }
}

pub struct CompiledUntrustedRustProject {
    plugin: Plugin,
}

impl CompiledUntrustedRustProject {
    /// fn_name may have module prefixes (e.g. `foo::exported_fn`)
    /// The '::' is converted to '_'
    pub fn call<'a, 'b, T: ToBytes<'a>, U: FromBytes<'b>>(
        &'b mut self,
        fn_name: impl AsRef<str>,
        input: T,
    ) -> Result<U> {
        let exported_fn_name = if fn_name.as_ref().contains("::") {
            fn_name.as_ref().replace("::", "__")
        } else {
            format!("__{}", fn_name.as_ref())
        };

        return match self.plugin.call(&exported_fn_name, input) {
            Ok(val) => Ok(val),
            Err(extism_err) => Err(match extism_err.to_string().as_str() {
                "oom" => UntRustedError::RuntimeExceededMemory(fn_name.as_ref().to_string()),
                "timeout" => UntRustedError::RuntimeExceededTimeout(fn_name.as_ref().to_string()),
                _ => UntRustedError::Extism(extism_err),
            })
        };
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let rust_code = "use extism_pdk::*;\n#[plugin_fn]\npub fn add2(a: i32) -> FnResult<i32> {\nreturn Ok(a + 2);\n}";

        let project = UntrustedRustProject::new(rust_code);

        let mut compiled_project = project.compile().unwrap();

        let outputs: i32 = compiled_project.call("add2", 10).unwrap();

        assert_eq!(12, outputs);
    }
}
