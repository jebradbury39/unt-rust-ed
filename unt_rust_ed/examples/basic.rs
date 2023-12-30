
use unt_rust_ed::UntrustedRustProject;

#[derive(serde::Serialize)]
pub struct Inputs {
    pub a: i32,
    pub b: i32,
}

#[derive(Debug, serde::Deserialize)]
pub struct Outputs {
    pub c: i32,
    pub d: String,
}

fn main() {
    let rust_code = "pub fn process(a: Inputs) -> Outputs {
        return a + 2;
    }";

    let project = UntrustedRustProject::new(rust_code);

    let mut compiled_project = project.compile().unwrap();

    let inputs = Inputs {
        a: 12,
        b: -3,
    };

    let outputs: Outputs = compiled_project.call("process", &inputs).unwrap();

    println!("output: {:?}", outputs);
}
