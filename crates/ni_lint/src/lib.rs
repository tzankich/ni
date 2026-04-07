pub mod checks;
pub mod diagnostic;
pub mod visitor;

use ni_parser::Program;

use crate::checks::naming::NamingCheck;
use crate::checks::unused_vars::UnusedVarsCheck;
use crate::diagnostic::LintDiagnostic;
use crate::visitor::AstVisitor;

/// Run all lint checks on a parsed program and return diagnostics.
pub fn lint(program: &Program) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();

    // Naming conventions
    let mut naming = NamingCheck::new();
    naming.visit_program(program);
    diagnostics.extend(naming.diagnostics);

    // Unused variables
    let mut unused = UnusedVarsCheck::new();
    unused.visit_program(program);
    diagnostics.extend(unused.diagnostics);

    // Sort by line number for consistent output
    diagnostics.sort_by_key(|d| (d.span.line, d.span.column));
    diagnostics
}

#[cfg(test)]
mod tests;
