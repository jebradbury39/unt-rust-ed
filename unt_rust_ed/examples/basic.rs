
use unt_rust_ed::{UntrustedRustProject, Json, WasmCompileTarget};
use unt_rust_ed_derive::exported_host_type;

#[exported_host_type]
pub struct Inputs {
    pub a: i32,
    pub b: i32,
    pub op: String,
}

#[exported_host_type]
pub struct Outputs {
    pub c: i32,
    pub d: String,
}

fn main() {
    let rust_code = "pub fn process(a: Inputs) -> Outputs {
        let mut items = Vec::new();
        loop {
            if a.op.is_empty() {
                break;
            }

            if a.op == \"oom\" {
                let x: Vec<u64> = Vec::with_capacity(1000);
                items.push(x);
            }
        }
        return Outputs { c: a.a + a.b, d: String::from(\"done\") };
    }";

    let project = UntrustedRustProject::new(rust_code)
        .with_target(WasmCompileTarget::Wasi)
        .with_max_memory_bytes(1 * 1024 * 1024) // 1 MB
        .with_runtime_timeout_ms(5 * 1000) // 5 sec
        .with_exported_host_type::<Inputs>()
        .with_exported_host_type::<Outputs>();

    let mut compiled_project = project.compile().unwrap();

    let ops = ["", "oom", "timeout"];

    for op in ops {
        let inputs = Inputs {
            a: 10,
            b: -3,
            op: op.into(),
        };

        use std::time::Instant;
        let now = Instant::now();
 
        let outputs: Json<Outputs> = match compiled_project.call("process", Json(inputs)) {
            Ok(outputs) => outputs,
            Err(err) => {
                println!("Hit error when calling 'process': {}", err);
                continue;
            }
        };

        println!("output (elapsed: {:.2?}) {:?}", now.elapsed(), outputs);
    }
}
