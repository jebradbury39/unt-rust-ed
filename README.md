# Unt-Rust-Ed

This library provides a simple api for taking in some untrusted rust code and executing it. It does this by compiling the rust to wasm, and then runs the wasm in a lightweight container (the containerization is mainly provided by Extism). The main benefit of this crate is that it will change the types as needed, so the untrusted code does not necessarily need to be aware that it will be executed as an Extism plugin. It also will take care of building the code into wasm.

# Examples

You can look at the examples folder for a full demo of how to use this crate, but here is the basic concept:

```rust
#[exported_host_type]
pub struct Inputs {
  pub a: i32,
  pub b: i32,
}

impl Inputs {
  fn new(a: i32, b: i32) -> Self {
    Self { a, b}
  }
}

fn main() {
  let rust_code = "pub fn add(inputs: Inputs) -> i32 {\nreturn inputs.a + inputs.b;\n}";       
                                                                                
  let project = UntrustedRustProject::new(rust_code)
    .with_exported_host_type::<Inputs>();                   
                                                                                
  let mut compiled_project = project.compile().unwrap();                  
                                                                                
  let outputs: i32 = compiled_project.call("add", Inputs::new(10, 2)).unwrap();          
                                                                                
  println!("output: {}", outputs); // prints "12"
}
```


