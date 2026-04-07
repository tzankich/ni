use ni_error::Span;
use ni_parser::ast::*;

use crate::diagnostic::LintDiagnostic;
use crate::visitor::AstVisitor;

pub struct NamingCheck {
    pub diagnostics: Vec<LintDiagnostic>,
}

impl Default for NamingCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl NamingCheck {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }
}

fn is_snake_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    // Allow leading underscores
    let s = s.trim_start_matches('_');
    if s.is_empty() {
        return true;
    }
    s.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.contains("__")
        && !s.ends_with('_')
}

fn is_pascal_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let first = s.chars().next().unwrap();
    if !first.is_ascii_uppercase() {
        return false;
    }
    // No underscores in PascalCase
    !s.contains('_') && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

impl AstVisitor for NamingCheck {
    fn visit_var_decl(&mut self, decl: &VarDecl, span: Span) {
        if !is_snake_case(&decl.name) {
            let suggestion = to_snake_case(&decl.name);
            self.diagnostics.push(
                LintDiagnostic::warning(
                    "NI001",
                    format!("Variable '{}' should use snake_case", decl.name),
                    span,
                )
                .with_suggestion(format!("Rename to '{suggestion}'")),
            );
        }
        self.visit_expr(&decl.initializer);
    }

    fn visit_fun_decl(&mut self, decl: &FunDecl, span: Span) {
        // "init" is a special method name -- skip it
        if decl.name != "init" && !is_snake_case(&decl.name) {
            let suggestion = to_snake_case(&decl.name);
            self.diagnostics.push(
                LintDiagnostic::warning(
                    "NI002",
                    format!("Function '{}' should use snake_case", decl.name),
                    span,
                )
                .with_suggestion(format!("Rename to '{suggestion}'")),
            );
        }
        for param in &decl.params {
            if let Some(ref default) = param.default {
                self.visit_expr(default);
            }
        }
        for stmt in &decl.body {
            self.visit_statement(stmt);
        }
    }

    fn visit_const_decl(&mut self, decl: &ConstDecl, span: Span) {
        if !is_snake_case(&decl.name) {
            let suggestion = to_snake_case(&decl.name);
            self.diagnostics.push(
                LintDiagnostic::warning(
                    "NI003",
                    format!("Constant '{}' should use snake_case", decl.name),
                    span,
                )
                .with_suggestion(format!("Rename to '{suggestion}'")),
            );
        }
        self.visit_expr(&decl.initializer);
    }

    fn visit_class_decl(&mut self, decl: &ClassDecl, span: Span) {
        if !is_pascal_case(&decl.name) {
            let suggestion = to_pascal_case(&decl.name);
            self.diagnostics.push(
                LintDiagnostic::warning(
                    "NI004",
                    format!("Class '{}' should use PascalCase", decl.name),
                    span,
                )
                .with_suggestion(format!("Rename to '{suggestion}'")),
            );
        }
        // Continue default recursion for class body
        for field in &decl.fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
        for method in &decl.methods {
            self.visit_fun_decl(&method.fun, span);
        }
        for field in &decl.static_fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
        for method in &decl.static_methods {
            self.visit_fun_decl(method, span);
        }
    }

    fn visit_enum_decl(&mut self, decl: &EnumDecl, span: Span) {
        if !is_pascal_case(&decl.name) {
            let suggestion = to_pascal_case(&decl.name);
            self.diagnostics.push(
                LintDiagnostic::warning(
                    "NI005",
                    format!("Enum '{}' should use PascalCase", decl.name),
                    span,
                )
                .with_suggestion(format!("Rename to '{suggestion}'")),
            );
        }
        for variant in &decl.variants {
            if let Some(ref val) = variant.value {
                self.visit_expr(val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert!(is_snake_case("hello_world"));
        assert!(is_snake_case("x"));
        assert!(is_snake_case("_private"));
        assert!(!is_snake_case("MyVar"));
        assert!(!is_snake_case("camelCase"));
        assert!(!is_snake_case("SCREAMING"));
    }

    #[test]
    fn test_pascal_case() {
        assert!(is_pascal_case("MyClass"));
        assert!(is_pascal_case("Point3D"));
        assert!(!is_pascal_case("my_class"));
        assert!(!is_pascal_case("myClass"));
    }
}
