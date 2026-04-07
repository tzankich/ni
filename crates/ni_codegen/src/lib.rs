mod ast_walker;
pub mod c_backend;
pub mod coroutine_transform;
#[allow(dead_code)]
mod name_mangler;
pub mod rust_backend;
mod trait_def;

#[cfg(test)]
mod tests;

use ni_parser::Program;

pub use trait_def::NiCodeGen;

/// Generate Rust source code from a parsed Ni program.
pub fn codegen_rust(program: &Program) -> String {
    let mut backend = rust_backend::RustCodeGen::new();
    ast_walker::walk_program(&mut backend, program);
    backend.finish()
}

/// Generate C source code from a parsed Ni program.
pub fn codegen_c(program: &Program) -> String {
    let mut backend = c_backend::CCodeGen::new();
    ast_walker::walk_program(&mut backend, program);
    backend.finish()
}
