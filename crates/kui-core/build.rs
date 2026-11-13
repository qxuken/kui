// Compile the same sources, wrapper and validator the runtime uses. A bad
// built-in must fail every build, including cross builds and cargo publish.
#[allow(dead_code)]
#[path = "src/fragment/shader.rs"]
mod shader;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/fragment/shader.rs");
    for builtin in shader::Builtin::ALL {
        shader::validate(builtin.source()).unwrap_or_else(|error| {
            panic!("built-in {builtin:?} fragment failed WGSL validation:\n{error}")
        });
    }
}
