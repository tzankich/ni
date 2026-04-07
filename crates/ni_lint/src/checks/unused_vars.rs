use std::collections::HashMap;

use ni_error::Span;
use ni_parser::ast::*;

use crate::diagnostic::LintDiagnostic;
use crate::visitor::AstVisitor;

struct VarEntry {
    span: Span,
    used: bool,
}

pub struct UnusedVarsCheck {
    pub diagnostics: Vec<LintDiagnostic>,
    scopes: Vec<HashMap<String, VarEntry>>,
}

impl Default for UnusedVarsCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl UnusedVarsCheck {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            for (name, entry) in scope {
                if !entry.used && !name.starts_with('_') {
                    self.diagnostics.push(
                        LintDiagnostic::warning(
                            "NI010",
                            format!("Variable '{name}' is declared but never used"),
                            entry.span,
                        )
                        .with_suggestion(format!("Prefix with underscore to silence: '_{name}'")),
                    );
                }
            }
        }
    }

    fn register_var(&mut self, name: &str, span: Span) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), VarEntry { span, used: false });
        }
    }

    fn mark_used(&mut self, name: &str) {
        // Search scopes from innermost to outermost
        for scope in self.scopes.iter_mut().rev() {
            if let Some(entry) = scope.get_mut(name) {
                entry.used = true;
                return;
            }
        }
    }
}

impl AstVisitor for UnusedVarsCheck {
    fn visit_program(&mut self, program: &Program) {
        // Top-level scope is already pushed in new()
        for decl in &program.declarations {
            self.visit_declaration(decl);
        }
        self.pop_scope();
    }

    fn visit_var_decl(&mut self, decl: &VarDecl, span: Span) {
        self.visit_expr(&decl.initializer);
        self.register_var(&decl.name, span);
    }

    fn visit_const_decl(&mut self, decl: &ConstDecl, span: Span) {
        self.visit_expr(&decl.initializer);
        self.register_var(&decl.name, span);
    }

    fn visit_fun_decl(&mut self, decl: &FunDecl, span: Span) {
        // Function body gets its own scope with params
        self.push_scope();
        for param in &decl.params {
            self.register_var(&param.name, span);
            if let Some(ref default) = param.default {
                self.visit_expr(default);
            }
        }
        for stmt in &decl.body {
            self.visit_statement(stmt);
        }
        self.pop_scope();
    }

    fn visit_for_stmt(&mut self, stmt: &ForStmt, span: Span) {
        self.visit_expr(&stmt.iterable);

        self.push_scope();
        self.register_var(&stmt.variable, span);
        if let Some(ref second) = stmt.second_var {
            self.register_var(second, span);
        }
        for s in &stmt.body {
            self.visit_statement(s);
        }
        self.pop_scope();
    }

    fn visit_try_stmt(&mut self, stmt: &TryStmt, span: Span) {
        for s in &stmt.body {
            self.visit_statement(s);
        }
        self.push_scope();
        if let Some(ref var) = stmt.catch_var {
            self.register_var(var, span);
        }
        match &stmt.catch_body {
            CatchBody::Block(stmts) => {
                for s in stmts {
                    self.visit_statement(s);
                }
            }
            CatchBody::Match(cases) => {
                for case in cases {
                    if let Some(ref guard) = case.guard {
                        self.visit_expr(guard);
                    }
                    for s in &case.body {
                        self.visit_statement(s);
                    }
                }
            }
        }
        self.pop_scope();
    }

    fn visit_class_decl(&mut self, decl: &ClassDecl, span: Span) {
        // Don't track unused inside class bodies (methods use `self`)
        for field in &decl.fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
        for method in &decl.methods {
            self.push_scope();
            for param in &method.fun.params {
                self.register_var(&param.name, span);
                if let Some(ref default) = param.default {
                    self.visit_expr(default);
                }
            }
            for stmt in &method.fun.body {
                self.visit_statement(stmt);
            }
            self.pop_scope();
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

    fn visit_match_stmt(&mut self, stmt: &MatchStmt) {
        self.visit_expr(&stmt.subject);
        for case in &stmt.cases {
            self.push_scope();
            for pattern in &case.patterns {
                match pattern {
                    Pattern::Binding(name) => {
                        // Use subject span as a proxy; real span not stored on Pattern
                        self.register_var(name, stmt.subject.span);
                    }
                    Pattern::Literal(expr) => self.visit_expr(expr),
                    Pattern::Wildcard | Pattern::TypeCheck(_, _) => {}
                }
            }
            if let Some(ref guard) = case.guard {
                self.visit_expr(guard);
            }
            for s in &case.body {
                self.visit_statement(s);
            }
            self.pop_scope();
        }
    }

    fn visit_enum_decl(&mut self, decl: &EnumDecl, _span: Span) {
        for variant in &decl.variants {
            if let Some(ref val) = variant.value {
                self.visit_expr(val);
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if let ExprKind::Identifier(ref name) = expr.kind {
            self.mark_used(name);
        }

        // Continue default recursion
        match &expr.kind {
            ExprKind::IntLiteral(_)
            | ExprKind::FloatLiteral(_)
            | ExprKind::StringLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::NoneLiteral
            | ExprKind::SelfExpr
            | ExprKind::Identifier(_) => {}
            ExprKind::FStringLiteral(parts) => {
                for part in parts {
                    if let FStringPart::Expr(e) = part {
                        self.visit_expr(e);
                    }
                }
            }
            ExprKind::Negate(e) | ExprKind::Not(e) => self.visit_expr(e),
            ExprKind::BinaryOp { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::And(l, r) | ExprKind::Or(l, r) => {
                self.visit_expr(l);
                self.visit_expr(r);
            }
            ExprKind::Compare { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Assign { target, value } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            ExprKind::CompoundAssign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            ExprKind::GetField(obj, _) => self.visit_expr(obj),
            ExprKind::SetField(obj, _, val) => {
                self.visit_expr(obj);
                self.visit_expr(val);
            }
            ExprKind::GetIndex(obj, idx) => {
                self.visit_expr(obj);
                self.visit_expr(idx);
            }
            ExprKind::SetIndex(obj, idx, val) => {
                self.visit_expr(obj);
                self.visit_expr(idx);
                self.visit_expr(val);
            }
            ExprKind::SafeNav(obj, _) => self.visit_expr(obj),
            ExprKind::NoneCoalesce(l, r) => {
                self.visit_expr(l);
                self.visit_expr(r);
            }
            ExprKind::Call {
                callee,
                args,
                named_args,
            } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
                for (_, arg) in named_args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::MethodCall {
                object,
                args,
                named_args,
                ..
            } => {
                self.visit_expr(object);
                for arg in args {
                    self.visit_expr(arg);
                }
                for (_, arg) in named_args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::SuperCall { args, .. } => {
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::List(items) => {
                for item in items {
                    self.visit_expr(item);
                }
            }
            ExprKind::Map(pairs) => {
                for (k, v) in pairs {
                    self.visit_expr(k);
                    self.visit_expr(v);
                }
            }
            ExprKind::Range { start, end, .. } => {
                self.visit_expr(start);
                self.visit_expr(end);
            }
            ExprKind::Lambda { params, body } => {
                self.push_scope();
                for param in params {
                    self.register_var(&param.name, expr.span);
                    if let Some(ref default) = param.default {
                        self.visit_expr(default);
                    }
                }
                for stmt in body {
                    self.visit_statement(stmt);
                }
                self.pop_scope();
            }
            ExprKind::IfExpr {
                value,
                condition,
                else_value,
            } => {
                self.visit_expr(value);
                self.visit_expr(condition);
                self.visit_expr(else_value);
            }
            ExprKind::Spawn(e)
            | ExprKind::Await(e)
            | ExprKind::TryExpr(e)
            | ExprKind::FailExpr(e)
            | ExprKind::Wait(e) => {
                self.visit_expr(e);
            }
            ExprKind::Yield(val) => {
                if let Some(v) = val {
                    self.visit_expr(v);
                }
            }
        }
    }
}
