
use unt_rust_ed::UntrustedRustProject;

fn main() {
    let rust_code = "pub fn add2(a: i32) -> i32 {
        return a + 2;
    }";

    let project = UntrustedRustProject::new(rust_code);

    let mut compiled_project = project.compile().unwrap();

    let outputs: i32 = compiled_project.call("add2", 10).unwrap();

    println!("output: {}", outputs);
}
