use ni_parser::{DeclKind, Program};

use crate::trait_def::NiCodeGen;

/// Walk the entire program AST, dispatching to the code generation backend.
pub fn walk_program(gen: &mut dyn NiCodeGen, program: &Program) {
    gen.begin_program();

    for decl in &program.declarations {
        match &decl.kind {
            DeclKind::Var(v) => gen.emit_var_decl(v),
            DeclKind::Const(c) => gen.emit_const_decl(c),
            DeclKind::Fun(f) => gen.emit_fun_decl(f),
            DeclKind::Class(c) => gen.emit_class_decl(c),
            DeclKind::Enum(e) => gen.emit_enum_decl(e),
            DeclKind::Import(i) => gen.emit_import(i),
            DeclKind::Spec(_) => {} // skip spec blocks in code generation
            DeclKind::Statement(s) => gen.emit_statement(s),
        }
    }

    gen.end_program();
}
