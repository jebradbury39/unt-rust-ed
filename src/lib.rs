use std::collections::HashMap;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub struct UntrustedRustModule {
    module_name: String,
    /// These functions must exist in the `rust_code`.
    /// The `module_name` plus an underscore will be prepended to each of these.
    exported_functions: Vec<String>,
    rust_code: String,
    runtime_memory_options: MemoryOptions,
    runtime_timeout_ms: Option<u64>,
}

#[derive(Default)]
pub struct UntrustedRustProject {
    modules: HashMap<String, UntrustedRustModule>,
}

impl UntrustedRustProject {
    pub fn add_module(&mut self, module: UntrustedRustModule) {
        self.modules.insert(module.module_name.clone(), module);
    }

    /// Converts the modules into compiled modules containing WASM
    pub fn compile() -> CompiledUntrustedRustProject {
        // create temp directory

        // setup cargo project by creating Cargo.toml in temp directory
        // extism-pdk = "0.3.4"
        // extism-pdk-derive = "0.3.1"

        // create modules in src/lib.rs file in temp directory.
        // For every exported function, create a copy with the module underscore prefix, and tag it as wasm-exported
        // Also perform checks such as ensuring that other functions do not start with any of the module names and an underscore

        // compile project to wasm by spawning cargo as a subprocess
    }
}

pub struct CompiledUntrustedRustModule {
    module_name: String,
    exported_functions: Vec<String>,
    plugin: Plugin,
}

impl CompiledUntrustedRustModule {
    pub fn call(
        &mut self,
        fn_name: impl AsRef<str>,
        input: impl AsRef<[u8]>,
    ) -> Result<&[u8], Error> {
        // the compiled function has "module_name_" prepended to it
        let compiled_fn_name = format!("{}_{}", self.module_name, fn_name.as_ref());
        self.plugin.call_map(compiled_fn_name, input, |x| Ok(x))
    }
}

pub struct CompiledUntrustedRustProject {
    modules: HashMap<String, CompiledUntrustedRustModule>,
}

impl CompiledUntrustedRustProject {
    pub fn call(
        &mut self,
        module_name: impl AsRef<str>,
        fn_name: impl AsRef<str>,
        input: impl AsRef<[u8]>,
    ) -> Result<Option<&[u8]>, Error> {
        return if let Some(module) = self.get_mut(module_name.as_ref()) {
            module.plugin.call(fn_name, input)
        } else {
            Ok(None)
        }
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

let player1_mode = UntrustedRustModule {
    module_name: "player1",
    exported_functions: vec!["entry"],
    rust_code: rust_code1,
    ..
};

let mut project = UntrustedRustProject::default();
project.add_module(player1_mod);
project.add_module(player2_mod);

let mut compiled_project = project.compile();
let outputs = compiled_project.call("player1", "entry", inputs);
 */

