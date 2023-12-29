mod error;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::ops::Deref;

use extism::{Manifest, Plugin, Wasm, ToBytes, FromBytes};
use extism_manifest::MemoryOptions;

use tempfile::TempDir;

use syn::Token;
use syn::token::Paren;
use syn::punctuated::Punctuated;
use syn::__private::Span;

use crate::error::*;

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

pub struct UntrustedRustProject {
    rust_code: String,
    runtime_memory_options: MemoryOptions,
    runtime_timeout_ms: Option<u64>,
    target: WasmCompileTarget,
}

impl UntrustedRustProject {

    pub fn new(rust_code: &str) -> Self {
        Self {
            rust_code: rust_code.into(),
            runtime_memory_options: MemoryOptions::default(),
            runtime_timeout_ms: None,
            target: WasmCompileTarget::default(),           
        }
    }

    /// Converts the modules into compiled modules containing WASM
    pub fn compile(&self) -> Result<CompiledUntrustedRustProject> {
        // create temp directory
        let tmp_cargo_dir = TempDir::new_in(".").map_err(|err| UntRustedError::IoError {
   resource: "TempDir".into(),
   err,
   })?;

        // setup cargo project by creating Cargo.toml in temp directory
        // extism-pdk = "0.3.4"
        // extism-pdk-derive = "0.3.1"
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

        let plugin = Plugin::new(&manifest, [], true)?;

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
    extism-pdk = \"1.0.0-rc1\"";

        cargo_toml_file.write_all(content.as_bytes()).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", cargo_toml_path.as_ref()),
   err,
   })?;

        return Ok(());
    }

    fn write_rust_code_to_cargo_dir<P: AsRef<Path>>(&self, cargo_src_path: P) -> Result<()> {

        let mut ast: syn::File = syn::parse_file(&self.rust_code)?;

        println!("{:?}", ast);

        // add validator

        // update the ast
        let use_extism_item = syn::Item::Use(syn::ItemUse {
            attrs: Vec::new(),
            vis: syn::Visibility::Inherited,
            use_token: Token![use](Span::call_site()),
            leading_colon: None,
            tree: syn::UseTree::Path(syn::UsePath {
                ident: syn::Ident::new("extism-pdk", Span::call_site()),
                colon2_token: Token![::](Span::call_site()),
                tree: Box::new(syn::UseTree::Glob(syn::UseGlob {
                    star_token: Token![*](Span::call_site()),
                })),
            }),
            semi_token: Token![;](Span::call_site()),
        });
        ast.items.insert(0, use_extism_item);

        Self::tag_functions_for_export(&mut ast.items, "")?;

        let lib_rs_path = cargo_src_path.as_ref().join("lib.rs");
        let mut lib_rs_file = File::create(&lib_rs_path).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", lib_rs_path),
   err,
   })?;

        lib_rs_file.write_all(self.rust_code.as_bytes()).map_err(|err| UntRustedError::IoError {
   resource: format!("{:?}", lib_rs_path),
   err,
   })?;

        return Ok(());
    }

    fn tag_functions_for_export(items: &mut Vec<syn::Item>, mod_names: &str) -> Result<()> {
        let mut item_idx: usize = 0;
        while item_idx < items.len() {

            match &mut items[item_idx] {
                syn::Item::Mod(item_mod) => if let Some(content) = &mut item_mod.content {
                    let new_mod_names = if mod_names.is_empty() {
                        item_mod.ident.to_string()
                    } else {
                        format!("{}__{}", mod_names, item_mod.ident.to_string())
                    };

                    Self::tag_functions_for_export(&mut content.1, &new_mod_names)?;
                },
                syn::Item::Fn(item_fn) => {
                    if item_fn.vis == syn::Visibility::Public(Token![pub](Span::call_site())) {
                        // export it by creating a clone of the function
                        let new_fn_name = format!("{}__{}", mod_names, item_fn.sig.ident.to_string());

                        let mut new_fn_sig = item_fn.sig.clone();
                        new_fn_sig.ident = syn::Ident::new(&new_fn_name, Span::call_site());

                        let mut call_old_fn_args = Punctuated::new();
                        for param in &item_fn.sig.inputs {
                            match param {
                                syn::FnArg::Typed(pat_type) => {
                                    let param_name: String = Self::get_param_name(pat_type)?;
                                    let mut param_segments = Punctuated::new();
                                    param_segments.push(syn::PathSegment {
                                        ident: syn::Ident::new(&param_name, Span::call_site()),
                                        arguments: syn::PathArguments::None,
                                    });

                                    call_old_fn_args.push(syn::Expr::Path(syn::ExprPath {
                                        attrs: Vec::new(),
                                        qself: None,
                                        path: syn::Path {
                                            leading_colon: None,
                                            segments: param_segments,
                                        },
                                    }));
                                },
                                _ => return Err(UntRustedError::UnsupportedFnArg(format!("{:?}", param))),
                            }
                        }

                        let mut old_fn_call_name_segments = Punctuated::new();
                        old_fn_call_name_segments.push(syn::PathSegment {
                            ident: item_fn.sig.ident.clone(),
                            arguments: syn::PathArguments::None,
                        });

                        let old_fn_call = syn::Stmt::Expr(syn::Expr::Call(syn::ExprCall {
                            attrs: Vec::new(),
                            func: Box::new(syn::Expr::Path(syn::ExprPath {
                                attrs: Vec::new(),
                                qself: None,
                                path: syn::Path {
                                    leading_colon: None,
                                    segments: old_fn_call_name_segments,
                                }
                            })),
                            paren_token: Paren::default(),
                            args: call_old_fn_args,
                        }), None);

                        let new_fn_block = syn::Block {
                            brace_token: item_fn.block.brace_token.clone(),
                            stmts: vec![old_fn_call],
                        };

                        let new_fn_item = syn::Item::Fn(syn::ItemFn {
                            attrs: item_fn.attrs.clone(),
                            vis: item_fn.vis.clone(),
                            sig: new_fn_sig,
                            block: Box::new(new_fn_block),
                        });

                        items.insert(item_idx + 1, new_fn_item);
                        item_idx += 1;
                    }
                },
                _ => (),
            }

            item_idx += 1;
        }

        return Ok(());
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
        println!("cargo build output:\n{:?}", cargo_output);

        if !cargo_output.status.success() {
            let stdout_str = String::from_utf8_lossy(&cargo_output.stdout);
            let stderr_str = String::from_utf8_lossy(&cargo_output.stderr);

            let need_target_err = format!("note: the `{}` target may not be installed", self.target.as_str());
            if stdout_str.contains(&need_target_err) || stderr_str.contains(&need_target_err) {
                return Err(UntRustedError::MissingCargoTargetInstallation);
            }

            // unknown error
            return Err(UntRustedError::UnknownCargoError(stdout_str.into(), stderr_str.into()));
        }

        return Ok(cargo_dir.as_ref().join("target/wasm32-unknown-unknown/release/test_wasm.wasm"));
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
        let exported_fn_name = fn_name.as_ref().replace("::", "_");
        return Ok(self.plugin.call(&exported_fn_name, input)?);
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
