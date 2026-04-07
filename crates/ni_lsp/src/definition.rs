use ni_error::Span;
use ni_parser::ast::*;
use std::collections::HashMap;

const MAX_DEPTH: usize = 256;

#[derive(Debug, Clone)]
pub struct SymbolDef {
    #[allow(dead_code)]
    pub name: String,
    pub span: Span,
    #[allow(dead_code)]
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Variable,
    Constant,
    Function,
    Class,
    Enum,
    Parameter,
    Field,
    Method,
    Import,
}

#[derive(Debug)]
struct Scope {
    symbols: HashMap<String, SymbolDef>,
    parent: Option<usize>,
    span: Option<Span>,
}

pub struct SymbolTable {
    scopes: Vec<Scope>,
    current: usize,
    depth: usize,
}

impl SymbolTable {
    fn new() -> Self {
        Self {
            scopes: vec![Scope {
                symbols: HashMap::new(),
                parent: None,
                span: None,
            }],
            current: 0,
            depth: 0,
        }
    }

    pub fn build(program: &Program) -> Self {
        let mut table = Self::new();
        for decl in &program.declarations {
            table.visit_declaration(decl);
        }
        table
    }

    fn push_scope(&mut self, span: Option<Span>) {
        let idx = self.scopes.len();
        self.scopes.push(Scope {
            symbols: HashMap::new(),
            parent: Some(self.current),
            span,
        });
        self.current = idx;
    }

    fn pop_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current].parent {
            self.current = parent;
        }
    }

    fn define(&mut self, name: String, span: Span, kind: SymbolKind) {
        self.scopes[self.current]
            .symbols
            .insert(name.clone(), SymbolDef { name, span, kind });
    }

    pub fn find(&self, name: &str, at_scope: usize) -> Option<&SymbolDef> {
        let mut scope_idx = Some(at_scope);
        while let Some(idx) = scope_idx {
            if let Some(sym) = self.scopes[idx].symbols.get(name) {
                return Some(sym);
            }
            scope_idx = self.scopes[idx].parent;
        }
        None
    }

    /// Find the deepest scope containing the given position.
    pub fn scope_at_position(&self, line: usize, col: usize) -> usize {
        let mut best = 0;
        for (idx, scope) in self.scopes.iter().enumerate() {
            if let Some(span) = &scope.span {
                if span_contains_position(span, line, col) {
                    best = idx;
                }
            }
        }
        best
    }

    fn visit_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclKind::Var(v) => {
                self.visit_expr(&v.initializer);
                self.define(v.name.clone(), decl.span, SymbolKind::Variable);
            }
            DeclKind::Const(c) => {
                self.visit_expr(&c.initializer);
                self.define(c.name.clone(), decl.span, SymbolKind::Constant);
            }
            DeclKind::Fun(f) => {
                self.define(f.name.clone(), decl.span, SymbolKind::Function);
                self.push_scope(Some(decl.span));
                for param in &f.params {
                    self.define(param.name.clone(), decl.span, SymbolKind::Parameter);
                    if let Some(ref default) = param.default {
                        self.visit_expr(default);
                    }
                }
                self.visit_body(&f.body);
                self.pop_scope();
            }
            DeclKind::Class(c) => {
                self.define(c.name.clone(), decl.span, SymbolKind::Class);
                self.push_scope(Some(decl.span));
                for field in &c.fields {
                    self.define(field.name.clone(), decl.span, SymbolKind::Field);
                }
                for field in &c.static_fields {
                    self.define(field.name.clone(), decl.span, SymbolKind::Field);
                }
                for method in &c.methods {
                    self.define(method.fun.name.clone(), decl.span, SymbolKind::Method);
                    self.push_scope(Some(decl.span));
                    for param in &method.fun.params {
                        self.define(param.name.clone(), decl.span, SymbolKind::Parameter);
                    }
                    self.visit_body(&method.fun.body);
                    self.pop_scope();
                }
                for sm in &c.static_methods {
                    self.define(sm.name.clone(), decl.span, SymbolKind::Method);
                    self.push_scope(Some(decl.span));
                    for param in &sm.params {
                        self.define(param.name.clone(), decl.span, SymbolKind::Parameter);
                    }
                    self.visit_body(&sm.body);
                    self.pop_scope();
                }
                self.pop_scope();
            }
            DeclKind::Enum(e) => {
                self.define(e.name.clone(), decl.span, SymbolKind::Enum);
            }
            DeclKind::Import(i) => match i {
                ImportDecl::Module { path, alias } => {
                    let name = alias.as_ref().unwrap_or(path.last().unwrap());
                    self.define(name.clone(), decl.span, SymbolKind::Import);
                }
                ImportDecl::From { names, .. } => {
                    for imp in names {
                        let name = imp.alias.as_ref().unwrap_or(&imp.name);
                        self.define(name.clone(), decl.span, SymbolKind::Import);
                    }
                }
                ImportDecl::FromAll { .. } => {}
            },
            DeclKind::Statement(stmt) => {
                self.visit_statement(stmt);
            }
            DeclKind::Spec(_) => {}
        }
    }

    fn visit_body(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.visit_statement(stmt);
        }
    }

    fn visit_statement(&mut self, stmt: &Statement) {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return;
        }
        match &stmt.kind {
            StmtKind::Expr(e) => self.visit_expr(e),
            StmtKind::VarDecl(v) => {
                self.visit_expr(&v.initializer);
                self.define(v.name.clone(), stmt.span, SymbolKind::Variable);
            }
            StmtKind::ConstDecl(c) => {
                self.visit_expr(&c.initializer);
                self.define(c.name.clone(), stmt.span, SymbolKind::Constant);
            }
            StmtKind::If(if_stmt) => {
                self.visit_expr(&if_stmt.condition);
                self.visit_body(&if_stmt.then_body);
                for (cond, body) in &if_stmt.elif_branches {
                    self.visit_expr(cond);
                    self.visit_body(body);
                }
                if let Some(ref body) = if_stmt.else_body {
                    self.visit_body(body);
                }
            }
            StmtKind::While(w) => {
                self.visit_expr(&w.condition);
                self.visit_body(&w.body);
            }
            StmtKind::For(f) => {
                self.push_scope(Some(stmt.span));
                self.define(f.variable.clone(), stmt.span, SymbolKind::Variable);
                if let Some(ref second) = f.second_var {
                    self.define(second.clone(), stmt.span, SymbolKind::Variable);
                }
                self.visit_expr(&f.iterable);
                self.visit_body(&f.body);
                self.pop_scope();
            }
            StmtKind::Match(m) => {
                self.visit_expr(&m.subject);
                for case in &m.cases {
                    self.push_scope(Some(stmt.span));
                    for pat in &case.patterns {
                        if let Pattern::Binding(name) | Pattern::TypeCheck(name, _) = pat {
                            self.define(name.clone(), stmt.span, SymbolKind::Variable);
                        }
                    }
                    if let Some(ref guard) = case.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_body(&case.body);
                    self.pop_scope();
                }
            }
            StmtKind::Return(Some(e)) => self.visit_expr(e),
            StmtKind::Fail(e) => self.visit_expr(e),
            StmtKind::Assert(e, msg) => {
                self.visit_expr(e);
                if let Some(m) = msg {
                    self.visit_expr(m);
                }
            }
            StmtKind::Block(stmts) => {
                self.push_scope(Some(stmt.span));
                self.visit_body(stmts);
                self.pop_scope();
            }
            StmtKind::Try(t) => {
                self.visit_body(&t.body);
                self.push_scope(Some(stmt.span));
                if let Some(ref var) = t.catch_var {
                    self.define(var.clone(), stmt.span, SymbolKind::Variable);
                }
                match &t.catch_body {
                    CatchBody::Block(stmts) => self.visit_body(stmts),
                    CatchBody::Match(cases) => {
                        for case in cases {
                            self.visit_body(&case.body);
                        }
                    }
                }
                self.pop_scope();
            }
            _ => {}
        }
        self.depth -= 1;
    }

    fn visit_expr(&mut self, expr: &Expr) {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return;
        }
        match &expr.kind {
            ExprKind::Lambda { params, body } => {
                self.push_scope(Some(expr.span));
                for param in params {
                    self.define(param.name.clone(), expr.span, SymbolKind::Parameter);
                }
                self.visit_body(body);
                self.pop_scope();
            }
            ExprKind::Call {
                callee,
                args,
                named_args,
            } => {
                self.visit_expr(callee);
                for a in args {
                    self.visit_expr(a);
                }
                for (_, a) in named_args {
                    self.visit_expr(a);
                }
            }
            ExprKind::MethodCall {
                object,
                args,
                named_args,
                ..
            } => {
                self.visit_expr(object);
                for a in args {
                    self.visit_expr(a);
                }
                for (_, a) in named_args {
                    self.visit_expr(a);
                }
            }
            ExprKind::BinaryOp { left, right, .. }
            | ExprKind::Compare { left, right, .. }
            | ExprKind::And(left, right)
            | ExprKind::Or(left, right)
            | ExprKind::Assign {
                target: left,
                value: right,
            }
            | ExprKind::CompoundAssign {
                target: left,
                value: right,
                ..
            }
            | ExprKind::NoneCoalesce(left, right)
            | ExprKind::GetIndex(left, right)
            | ExprKind::Range {
                start: left,
                end: right,
                ..
            } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::SetIndex(a, b, c)
            | ExprKind::IfExpr {
                value: a,
                condition: b,
                else_value: c,
            } => {
                self.visit_expr(a);
                self.visit_expr(b);
                self.visit_expr(c);
            }
            ExprKind::SetField(obj, _, val) => {
                self.visit_expr(obj);
                self.visit_expr(val);
            }
            ExprKind::Negate(e)
            | ExprKind::Not(e)
            | ExprKind::GetField(e, _)
            | ExprKind::SafeNav(e, _)
            | ExprKind::Spawn(e)
            | ExprKind::Await(e)
            | ExprKind::TryExpr(e)
            | ExprKind::FailExpr(e)
            | ExprKind::Wait(e) => {
                self.visit_expr(e);
            }
            ExprKind::Yield(Some(v)) => {
                self.visit_expr(v);
            }
            ExprKind::Yield(None) => {}
            ExprKind::SuperCall { args, .. } => {
                for a in args {
                    self.visit_expr(a);
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
            ExprKind::FStringLiteral(parts) => {
                for part in parts {
                    if let FStringPart::Expr(e) = part {
                        self.visit_expr(e);
                    }
                }
            }
            _ => {}
        }
        self.depth -= 1;
    }
}

/// Find the identifier at (line, col) in the program. Both 1-indexed.
pub fn identifier_at_position(
    program: &Program,
    line: usize,
    col: usize,
) -> Option<(String, Span)> {
    for decl in &program.declarations {
        if let Some(result) = check_decl(decl, line, col) {
            return Some(result);
        }
    }
    None
}

fn check_decl(decl: &Declaration, line: usize, col: usize) -> Option<(String, Span)> {
    match &decl.kind {
        DeclKind::Var(v) => check_expr(&v.initializer, line, col),
        DeclKind::Const(c) => check_expr(&c.initializer, line, col),
        DeclKind::Fun(f) => {
            for param in &f.params {
                if let Some(ref default) = param.default {
                    if let Some(r) = check_expr(default, line, col) {
                        return Some(r);
                    }
                }
            }
            check_body(&f.body, line, col)
        }
        DeclKind::Class(c) => {
            for field in &c.fields {
                if let Some(ref default) = field.default {
                    if let Some(r) = check_expr(default, line, col) {
                        return Some(r);
                    }
                }
            }
            for method in &c.methods {
                if let Some(r) = check_body(&method.fun.body, line, col) {
                    return Some(r);
                }
            }
            for sm in &c.static_methods {
                if let Some(r) = check_body(&sm.body, line, col) {
                    return Some(r);
                }
            }
            None
        }
        DeclKind::Statement(stmt) => check_stmt(stmt, line, col),
        _ => None,
    }
}

fn check_body(stmts: &[Statement], line: usize, col: usize) -> Option<(String, Span)> {
    for stmt in stmts {
        if let Some(r) = check_stmt(stmt, line, col) {
            return Some(r);
        }
    }
    None
}

fn check_stmt(stmt: &Statement, line: usize, col: usize) -> Option<(String, Span)> {
    match &stmt.kind {
        StmtKind::Expr(e) => check_expr(e, line, col),
        StmtKind::VarDecl(v) => check_expr(&v.initializer, line, col),
        StmtKind::ConstDecl(c) => check_expr(&c.initializer, line, col),
        StmtKind::If(if_stmt) => check_expr(&if_stmt.condition, line, col)
            .or_else(|| check_body(&if_stmt.then_body, line, col))
            .or_else(|| {
                if_stmt.elif_branches.iter().find_map(|(cond, body)| {
                    check_expr(cond, line, col).or_else(|| check_body(body, line, col))
                })
            })
            .or_else(|| {
                if_stmt
                    .else_body
                    .as_ref()
                    .and_then(|b| check_body(b, line, col))
            }),
        StmtKind::While(w) => {
            check_expr(&w.condition, line, col).or_else(|| check_body(&w.body, line, col))
        }
        StmtKind::For(f) => {
            check_expr(&f.iterable, line, col).or_else(|| check_body(&f.body, line, col))
        }
        StmtKind::Match(m) => check_expr(&m.subject, line, col).or_else(|| {
            m.cases.iter().find_map(|case| {
                case.guard
                    .as_ref()
                    .and_then(|g| check_expr(g, line, col))
                    .or_else(|| check_body(&case.body, line, col))
            })
        }),
        StmtKind::Return(Some(e)) | StmtKind::Fail(e) => check_expr(e, line, col),
        StmtKind::Assert(e, msg) => {
            check_expr(e, line, col).or_else(|| msg.as_ref().and_then(|m| check_expr(m, line, col)))
        }
        StmtKind::Block(stmts) => check_body(stmts, line, col),
        StmtKind::Try(t) => check_body(&t.body, line, col).or_else(|| match &t.catch_body {
            CatchBody::Block(stmts) => check_body(stmts, line, col),
            CatchBody::Match(cases) => cases
                .iter()
                .find_map(|case| check_body(&case.body, line, col)),
        }),
        _ => None,
    }
}

fn check_expr(expr: &Expr, line: usize, col: usize) -> Option<(String, Span)> {
    // Check if cursor is within this expression's span at all
    if expr.span.line != 0 && !span_contains_position(&expr.span, line, col) {
        return None;
    }

    match &expr.kind {
        ExprKind::Identifier(name) => {
            if span_contains_position(&expr.span, line, col) {
                Some((name.clone(), expr.span))
            } else {
                None
            }
        }
        ExprKind::Call {
            callee,
            args,
            named_args,
        } => check_expr(callee, line, col)
            .or_else(|| args.iter().find_map(|a| check_expr(a, line, col)))
            .or_else(|| {
                named_args
                    .iter()
                    .find_map(|(_, a)| check_expr(a, line, col))
            }),
        ExprKind::MethodCall {
            object,
            args,
            named_args,
            ..
        } => check_expr(object, line, col)
            .or_else(|| args.iter().find_map(|a| check_expr(a, line, col)))
            .or_else(|| {
                named_args
                    .iter()
                    .find_map(|(_, a)| check_expr(a, line, col))
            }),
        ExprKind::BinaryOp { left, right, .. }
        | ExprKind::Compare { left, right, .. }
        | ExprKind::And(left, right)
        | ExprKind::Or(left, right)
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::CompoundAssign {
            target: left,
            value: right,
            ..
        }
        | ExprKind::NoneCoalesce(left, right)
        | ExprKind::GetIndex(left, right)
        | ExprKind::Range {
            start: left,
            end: right,
            ..
        } => check_expr(left, line, col).or_else(|| check_expr(right, line, col)),
        ExprKind::SetIndex(a, b, c)
        | ExprKind::IfExpr {
            value: a,
            condition: b,
            else_value: c,
        } => check_expr(a, line, col)
            .or_else(|| check_expr(b, line, col))
            .or_else(|| check_expr(c, line, col)),
        ExprKind::SetField(obj, _, val) => {
            check_expr(obj, line, col).or_else(|| check_expr(val, line, col))
        }
        ExprKind::GetField(obj, _) | ExprKind::SafeNav(obj, _) => check_expr(obj, line, col),
        ExprKind::Negate(e)
        | ExprKind::Not(e)
        | ExprKind::Spawn(e)
        | ExprKind::Await(e)
        | ExprKind::TryExpr(e)
        | ExprKind::FailExpr(e)
        | ExprKind::Wait(e) => check_expr(e, line, col),
        ExprKind::Yield(val) => val.as_ref().and_then(|v| check_expr(v, line, col)),
        ExprKind::SuperCall { args, .. } => args.iter().find_map(|a| check_expr(a, line, col)),
        ExprKind::List(items) => items.iter().find_map(|i| check_expr(i, line, col)),
        ExprKind::Map(pairs) => pairs
            .iter()
            .find_map(|(k, v)| check_expr(k, line, col).or_else(|| check_expr(v, line, col))),
        ExprKind::Lambda { body, .. } => check_body(body, line, col),
        ExprKind::FStringLiteral(parts) => parts.iter().find_map(|part| {
            if let FStringPart::Expr(e) = part {
                check_expr(e, line, col)
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Check if a span contains the given position (both 1-indexed).
fn span_contains_position(span: &Span, line: usize, col: usize) -> bool {
    if span.end_line == 0 || span.end_line == span.line {
        // Single-line span
        span.line == line && col >= span.column && col < span.end_column
    } else {
        // Multi-line span
        if line < span.line || line > span.end_line {
            return false;
        }
        if line == span.line {
            return col >= span.column;
        }
        if line == span.end_line {
            return col < span.end_column;
        }
        true // line is between start and end
    }
}

/// Find method/field definitions across all classes in the program.
/// If class_hint is Some, only search that class. Otherwise search all.
#[allow(dead_code)]
pub fn find_method_or_field(
    program: &Program,
    class_hint: Option<&str>,
    name: &str,
) -> Option<SymbolDef> {
    for decl in &program.declarations {
        if let DeclKind::Class(c) = &decl.kind {
            if class_hint.is_some_and(|h| h != c.name) {
                continue;
            }
            for method in &c.methods {
                if method.fun.name == name {
                    return Some(SymbolDef {
                        name: name.to_string(),
                        span: decl.span,
                        kind: SymbolKind::Method,
                    });
                }
            }
            for field in &c.fields {
                if field.name == name {
                    return Some(SymbolDef {
                        name: name.to_string(),
                        span: decl.span,
                        kind: SymbolKind::Field,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Program {
        let tokens = ni_lexer::lex(source).unwrap();
        ni_parser::parse(tokens).unwrap()
    }

    #[test]
    fn test_symbol_table_global_var() {
        let program = parse("var x = 10");
        let table = SymbolTable::build(&program);
        let sym = table.find("x", 0).unwrap();
        assert_eq!(sym.name, "x");
        assert_eq!(sym.kind, SymbolKind::Variable);
    }

    #[test]
    fn test_symbol_table_const() {
        let program = parse("const PI = 3.14");
        let table = SymbolTable::build(&program);
        let sym = table.find("PI", 0).unwrap();
        assert_eq!(sym.name, "PI");
        assert_eq!(sym.kind, SymbolKind::Constant);
    }

    #[test]
    fn test_symbol_table_function() {
        let program = parse("fun greet(name):\n    print(name)");
        let table = SymbolTable::build(&program);
        let sym = table.find("greet", 0).unwrap();
        assert_eq!(sym.name, "greet");
        assert_eq!(sym.kind, SymbolKind::Function);
    }

    #[test]
    fn test_symbol_table_function_param() {
        let program = parse("fun greet(name):\n    print(name)");
        let table = SymbolTable::build(&program);
        // param "name" is in scope 1 (function body scope)
        let sym = table.find("name", 1).unwrap();
        assert_eq!(sym.name, "name");
        assert_eq!(sym.kind, SymbolKind::Parameter);
        // Not visible in global scope
        assert!(table.find("name", 0).is_none());
    }

    #[test]
    fn test_symbol_table_class() {
        let program = parse("class Dog:\n    var name = \"Rex\"");
        let table = SymbolTable::build(&program);
        let sym = table.find("Dog", 0).unwrap();
        assert_eq!(sym.kind, SymbolKind::Class);
    }

    #[test]
    fn test_symbol_table_enum() {
        let program = parse("enum Color:\n    red = 0\n    blue = 1");
        let table = SymbolTable::build(&program);
        let sym = table.find("Color", 0).unwrap();
        assert_eq!(sym.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_symbol_table_import() {
        let program = parse("import math");
        let table = SymbolTable::build(&program);
        let sym = table.find("math", 0).unwrap();
        assert_eq!(sym.kind, SymbolKind::Import);
    }

    #[test]
    fn test_symbol_table_for_loop_var() {
        let program = parse("for i in [1, 2, 3]:\n    print(i)");
        let table = SymbolTable::build(&program);
        // "i" is in the for-loop scope, not global
        assert!(table.find("i", 0).is_none());
        let sym = table.find("i", 1).unwrap();
        assert_eq!(sym.kind, SymbolKind::Variable);
    }

    #[test]
    fn test_identifier_at_position() {
        // "var x = foo" -- foo is at line 1, columns 9-12
        let program = parse("var x = foo");
        let result = identifier_at_position(&program, 1, 9);
        assert!(result.is_some());
        let (name, _) = result.unwrap();
        assert_eq!(name, "foo");
    }

    #[test]
    fn test_identifier_at_position_not_found() {
        let program = parse("var x = 42");
        let result = identifier_at_position(&program, 1, 9);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_method_in_class() {
        let program = parse("class Dog:\n    fun bark():\n        print(\"woof\")");
        let result = find_method_or_field(&program, Some("Dog"), "bark");
        assert!(result.is_some());
        assert_eq!(result.unwrap().kind, SymbolKind::Method);
    }

    #[test]
    fn test_find_method_not_found() {
        let program = parse("class Dog:\n    fun bark():\n        print(\"woof\")");
        let result = find_method_or_field(&program, Some("Dog"), "sit");
        assert!(result.is_none());
    }
}
