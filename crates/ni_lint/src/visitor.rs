use ni_parser::ast::*;

/// Trait for walking the AST. Implement only the methods you need;
/// the default implementations recurse into child nodes.
pub trait AstVisitor {
    // --- Declarations ---

    fn visit_program(&mut self, program: &Program) {
        for decl in &program.declarations {
            self.visit_declaration(decl);
        }
    }

    fn visit_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclKind::Var(v) => self.visit_var_decl(v, decl.span),
            DeclKind::Const(c) => self.visit_const_decl(c, decl.span),
            DeclKind::Fun(f) => self.visit_fun_decl(f, decl.span),
            DeclKind::Class(c) => self.visit_class_decl(c, decl.span),
            DeclKind::Enum(e) => self.visit_enum_decl(e, decl.span),
            DeclKind::Import(i) => self.visit_import_decl(i, decl.span),
            DeclKind::Spec(t) => self.visit_spec_decl(t, decl.span),
            DeclKind::Statement(s) => self.visit_statement(s),
        }
    }

    fn visit_var_decl(&mut self, decl: &VarDecl, _span: ni_error::Span) {
        self.visit_expr(&decl.initializer);
    }

    fn visit_const_decl(&mut self, decl: &ConstDecl, _span: ni_error::Span) {
        self.visit_expr(&decl.initializer);
    }

    fn visit_fun_decl(&mut self, decl: &FunDecl, _span: ni_error::Span) {
        for param in &decl.params {
            if let Some(ref default) = param.default {
                self.visit_expr(default);
            }
        }
        for stmt in &decl.body {
            self.visit_statement(stmt);
        }
    }

    fn visit_class_decl(&mut self, decl: &ClassDecl, _span: ni_error::Span) {
        for field in &decl.fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
        for method in &decl.methods {
            self.visit_fun_decl(&method.fun, _span);
        }
        for field in &decl.static_fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
        for method in &decl.static_methods {
            self.visit_fun_decl(method, _span);
        }
    }

    fn visit_enum_decl(&mut self, decl: &EnumDecl, _span: ni_error::Span) {
        for variant in &decl.variants {
            if let Some(ref val) = variant.value {
                self.visit_expr(val);
            }
        }
    }

    fn visit_import_decl(&mut self, _decl: &ImportDecl, _span: ni_error::Span) {}

    fn visit_spec_decl(&mut self, decl: &SpecDecl, _span: ni_error::Span) {
        for stmt in &decl.body {
            self.visit_statement(stmt);
        }
        for section in &decl.sections {
            self.visit_spec_section(section);
        }
        if let Some(ref each) = decl.each {
            for item in &each.items {
                self.visit_expr(item);
            }
        }
    }

    fn visit_spec_section(&mut self, section: &SpecSection) {
        for stmt in &section.body {
            self.visit_statement(stmt);
        }
        for child in &section.children {
            self.visit_spec_section(child);
        }
    }

    // --- Statements ---

    fn visit_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.visit_expr(expr),
            StmtKind::VarDecl(v) => self.visit_var_decl(v, stmt.span),
            StmtKind::ConstDecl(c) => self.visit_const_decl(c, stmt.span),
            StmtKind::If(if_stmt) => self.visit_if_stmt(if_stmt),
            StmtKind::While(while_stmt) => self.visit_while_stmt(while_stmt),
            StmtKind::For(for_stmt) => self.visit_for_stmt(for_stmt, stmt.span),
            StmtKind::Match(match_stmt) => self.visit_match_stmt(match_stmt),
            StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.visit_expr(e);
                }
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Pass => {}
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.visit_statement(s);
                }
            }
            StmtKind::Try(try_stmt) => self.visit_try_stmt(try_stmt, stmt.span),
            StmtKind::Fail(expr) => self.visit_expr(expr),
            StmtKind::Assert(expr, msg) => {
                self.visit_expr(expr);
                if let Some(m) = msg {
                    self.visit_expr(m);
                }
            }
        }
    }

    fn visit_if_stmt(&mut self, stmt: &IfStmt) {
        self.visit_expr(&stmt.condition);
        for s in &stmt.then_body {
            self.visit_statement(s);
        }
        for (cond, body) in &stmt.elif_branches {
            self.visit_expr(cond);
            for s in body {
                self.visit_statement(s);
            }
        }
        if let Some(ref else_body) = stmt.else_body {
            for s in else_body {
                self.visit_statement(s);
            }
        }
    }

    fn visit_while_stmt(&mut self, stmt: &WhileStmt) {
        self.visit_expr(&stmt.condition);
        for s in &stmt.body {
            self.visit_statement(s);
        }
    }

    fn visit_for_stmt(&mut self, stmt: &ForStmt, _span: ni_error::Span) {
        self.visit_expr(&stmt.iterable);
        for s in &stmt.body {
            self.visit_statement(s);
        }
    }

    fn visit_match_stmt(&mut self, stmt: &MatchStmt) {
        self.visit_expr(&stmt.subject);
        for case in &stmt.cases {
            for pattern in &case.patterns {
                if let Pattern::Literal(expr) = pattern {
                    self.visit_expr(expr);
                }
            }
            if let Some(ref guard) = case.guard {
                self.visit_expr(guard);
            }
            for s in &case.body {
                self.visit_statement(s);
            }
        }
    }

    fn visit_try_stmt(&mut self, stmt: &TryStmt, _span: ni_error::Span) {
        for s in &stmt.body {
            self.visit_statement(s);
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
    }

    // --- Expressions ---

    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::IntLiteral(_)
            | ExprKind::FloatLiteral(_)
            | ExprKind::StringLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::NoneLiteral
            | ExprKind::SelfExpr => {}
            ExprKind::Identifier(_) => {}
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
                for param in params {
                    if let Some(ref default) = param.default {
                        self.visit_expr(default);
                    }
                }
                for stmt in body {
                    self.visit_statement(stmt);
                }
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
            | ExprKind::Wait(e)
            | ExprKind::Await(e)
            | ExprKind::TryExpr(e)
            | ExprKind::FailExpr(e) => {
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
