use ni_parser::{ClassDecl, ConstDecl, EnumDecl, Expr, FunDecl, ImportDecl, Statement, VarDecl};

/// Trait that each code generation backend implements.
/// The ast_walker dispatches to these methods as it traverses the AST.
pub trait NiCodeGen {
    // Program structure
    fn begin_program(&mut self);
    fn end_program(&mut self);

    // Declarations
    fn emit_var_decl(&mut self, decl: &VarDecl);
    fn emit_const_decl(&mut self, decl: &ConstDecl);
    fn emit_fun_decl(&mut self, decl: &FunDecl);
    fn emit_class_decl(&mut self, decl: &ClassDecl);
    fn emit_enum_decl(&mut self, decl: &EnumDecl);
    fn emit_import(&mut self, decl: &ImportDecl);

    // Statements
    fn emit_statement(&mut self, stmt: &Statement);

    // Expression (returns the expression as a string in target language)
    fn emit_expr(&mut self, expr: &Expr) -> String;
}
