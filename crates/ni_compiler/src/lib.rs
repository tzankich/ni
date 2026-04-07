mod compiler;

pub use compiler::Compiler;

use ni_error::NiResult;
use ni_parser::Program;
use ni_vm::{GcHeap, GcRef, InternTable, Vm};
use std::path::PathBuf;

pub fn compile(
    program: &Program,
    heap: &mut GcHeap,
    interner: &mut InternTable,
) -> NiResult<GcRef> {
    let mut compiler = Compiler::new(heap, interner);
    compiler.compile(program)
}

pub fn compile_with_source_root(
    program: &Program,
    heap: &mut GcHeap,
    interner: &mut InternTable,
    source_root: PathBuf,
) -> NiResult<GcRef> {
    let mut compiler = Compiler::new(heap, interner).with_source_root(source_root);
    compiler.compile(program)
}

pub fn compile_spec_mode(
    program: &Program,
    heap: &mut GcHeap,
    interner: &mut InternTable,
) -> NiResult<GcRef> {
    let mut compiler = Compiler::new(heap, interner).with_spec_mode(true);
    compiler.compile(program)
}

pub fn compile_spec_mode_with_source_root(
    program: &Program,
    heap: &mut GcHeap,
    interner: &mut InternTable,
    source_root: PathBuf,
) -> NiResult<GcRef> {
    let mut compiler = Compiler::new(heap, interner)
        .with_source_root(source_root)
        .with_spec_mode(true);
    compiler.compile(program)
}

/// Compile with an optional module allowlist.
/// Pass `None` to allow all modules (default behavior).
pub fn compile_with_allowed_modules(
    program: &Program,
    heap: &mut GcHeap,
    interner: &mut InternTable,
    allowed_modules: Option<Vec<String>>,
) -> NiResult<GcRef> {
    let mut compiler = Compiler::new(heap, interner).with_allowed_modules(allowed_modules);
    compiler.compile(program)
}

/// Compile source code into a closure without interpreting it.
///
/// Lexes, parses, and compiles the source, returning the top-level closure.
/// Useful for async runtimes that need to queue closures as fibers.
pub fn compile_source(
    source: &str,
    heap: &mut GcHeap,
    interner: &mut InternTable,
) -> NiResult<GcRef> {
    let tokens = ni_lexer::lex(source)?;
    let program = ni_parser::parse(tokens)?;
    compile(&program, heap, interner)
}

/// Hot-reload new source into a running VM.
///
/// Lexes, parses, compiles, and interprets the source in the existing VM.
/// Globals persist across calls, so redefined functions/classes take effect
/// immediately while state variables are preserved.
pub fn hot_reload(vm: &mut Vm, source: &str) -> NiResult<()> {
    let tokens = ni_lexer::lex(source)?;
    let program = ni_parser::parse(tokens)?;
    let closure = compile(&program, &mut vm.heap, &mut vm.interner)?;
    vm.interpret(closure)?;
    Ok(())
}

/// Hot-reload with a source root for import resolution.
pub fn hot_reload_with_source_root(vm: &mut Vm, source: &str, root: PathBuf) -> NiResult<()> {
    let tokens = ni_lexer::lex(source)?;
    let program = ni_parser::parse(tokens)?;
    let closure = compile_with_source_root(&program, &mut vm.heap, &mut vm.interner, root)?;
    vm.interpret(closure)?;
    Ok(())
}
