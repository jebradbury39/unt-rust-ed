
use unt_rust_ed::{UntrustedRustProject, Json, WasmCompileTarget};
use unt_rust_ed_derive::exported_host_type;

#[exported_host_type]
pub struct Inputs {
    pub a: i32,
    pub b: i32,
}

#[exported_host_type]
pub struct Outputs {
    pub c: i32,
    pub d: String,
}

fn main() {
    let rust_code = "pub fn process(a: Inputs) -> Outputs {
        println!(\"start plugin\");
        loop {}
        return Outputs { c: a.a + a.b, d: String::from(\"done\") };
    }";

    let project = UntrustedRustProject::new(rust_code)
        .with_target(WasmCompileTarget::Wasi)
        .with_max_memory_bytes(1 * 1024 * 1024) // 1 MB
        .with_runtime_timeout_ms(30 * 1000)
        .with_exported_host_type::<Inputs>()
        .with_exported_host_type::<Outputs>();

    let mut compiled_project = project.compile().unwrap();

    let inputs = Inputs {
        a: 10,
        b: -3,
    };

    let outputs: Json<Outputs> = compiled_project.call("process", Json(inputs)).unwrap();

    println!("output: {:?}", outputs);
}
