mod error;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use extism::{Manifest, Plugin};
use extism::manifest::MemoryOptions;

use tempfile::TempDir;

use crate::error::*;

/// Returns the number of bytes in a page of memory.
/// This is useful for determining how many pages to give to an untrusted project
pub fn get_page_size() -> usize {
    return page_size::get();
}

pub struct UntrustedRustProject {
    rust_code: String,
    runtime_memory_options: MemoryOptions,
    runtime_timeout_ms: Option<u64>,
}

impl UntrustedRustProject {

    pub fn new(rust_code: &str) -> Self {
        Self {
            rust_code,
            ..
        }
    }

    /// Converts the modules into compiled modules containing WASM
    pub fn compile(self) -> Result<CompiledUntrustedRustProject> {
        // create temp directory
        let tmp_cargo_dir = TempDir::new_in(".")?;

        // setup cargo project by creating Cargo.toml in temp directory
        // extism-pdk = "0.3.4"
        // extism-pdk-derive = "0.3.1"
        let cargo_toml_path = tmp_cargo_dir.path().join("Cargo.toml");
        let mut cargo_toml_file = File::create(cargo_toml_path)?;

        Self::write_cargo_toml(&mut cargo_toml_file)?;

        // mkdir 'src' under tmp_cargo_dir
        let cargo_src_path = tmp_cargo_dir.path().join("src");
        fs::create_dir(cargo_src_path)?;

        // create modules in src/lib.rs file in temp directory.
        // For every exported function, create a copy with the module underscore prefix, and tag it as wasm-exported
        // Also perform checks such as ensuring that other functions do not start with any of the module names and an underscore
        self.write_rust_code_to_cargo_dir(&cargo_src_path)?;

        // compile project to wasm by spawning cargo as a subprocess
        let built_wasm_file_path: PathBuf = Self::cargo_build_to_wasm(&tmp_cargo_dir)?;

        let built_wasm_bytes = fs::read(&built_wasm_file_path)?;

        let manifest = Manifest::new(built_wasm_bytes)
            .disallow_all_hosts()
            .with_memory_options(self.runtime_memory_options);

        let manifest = if let Some(runtime_timeout_ms) = self.runtime_timeout_ms {
            manifest.with_timeout(runtime_timeout_ms.into())
        } else {
            manifest
        };

        let plugin = Plugin::new(&manifest, [], true)?;

        return CompiledUntrustedRustProject {
            plugin,
        };
    }

    fn write_cargo_toml(cargo_toml_file: &mut File) -> Result<()> {
        let content = "[package]
    name = \"test-wasm\"
    version = \"0.1.0\"
    edition = \"2021\"

    [lib]
    crate-type = [\"cdylib\"]

    [dependencies]";

        cargo_toml_file.write(content.as_bytes())?;

        return Ok(());
    }

    fn write_rust_code_to_cargo_dir<P: AsRef<Path>>(self, cargo_src_path: P) -> Result<()> {

    }

    fn cargo_build_to_wasm<P: AsRef<Path>>(cwd: P) -> Result<PathBuf> {
        let cargo_output = Command::new("cargo")
            .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
            .current_dir(&tmp_cargo_dir)
            .output()?;
    }
}

pub struct CompiledUntrustedRustProject {
    plugin: Plugin,
}

impl CompiledUntrustedRustProject {
    /// fn_name may have module prefixes (e.g. `foo::exported_fn`)
    /// The '::' is converted to '_'
    pub fn call(
        &mut self,
        fn_name: impl AsRef<str>,
        input: impl AsRef<[u8]>,
    ) -> Result<Option<&[u8]>, Error> {
        let exported_fn_name = fn_name.replace("::", "_");
        self.plugin.call_map(exported_fn_name, input, |x| Ok(x))
    }
}

/* Usage:

let rust_code1 = """mod player1 {
    pub fn entry(inputs: Inputs) -> Outputs {
    }
}
""";

let rust_code2 = """mod player2 {
    pub fn entry(inputs: Inputs) -> Outputs {
    }
}
""";

let project = UntrustedRustProject::new(&format!("{}{}", rust_code1, rust_code2));

let mut compiled_project = project.compile();
let outputs = compiled_project.call("player1::entry", inputs);
 */

