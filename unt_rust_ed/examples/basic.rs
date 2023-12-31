
use unt_rust_ed::{UntrustedRustProject, ExportedHostType};
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
    let rust_code = "pub fn process(a: i32) -> i32 {
        return a + 2;
    }";

    let mut project = UntrustedRustProject::new(rust_code);

    project.add_exported_host_type::<Inputs>();
    project.add_exported_host_type::<Outputs>();

    let mut compiled_project = project.compile().unwrap();

    println!("inputs typdef: {}", Inputs::typedef_as_string());

    let outputs: i32 = compiled_project.call("process", 10).unwrap();

    println!("output: {:?}", outputs);
}
