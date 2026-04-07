use ni_lexer::Comment;
use ni_parser::ast::*;
use std::collections::HashMap;

/// Maps comments to source lines for emission during formatting.
pub struct CommentMap {
    /// Leading comments keyed by the line they precede (or appear on).
    leading: HashMap<usize, Vec<Comment>>,
    /// Trailing comments keyed by the line they appear on.
    trailing: HashMap<usize, Comment>,
}

impl CommentMap {
    pub fn build(comments: Vec<Comment>) -> Self {
        let mut leading: HashMap<usize, Vec<Comment>> = HashMap::new();
        let mut trailing: HashMap<usize, Comment> = HashMap::new();

        for comment in comments {
            if comment.is_trailing {
                trailing.insert(comment.line, comment);
            } else {
                leading.entry(comment.line).or_default().push(comment);
            }
        }

        Self { leading, trailing }
    }

    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            leading: HashMap::new(),
            trailing: HashMap::new(),
        }
    }
}

pub struct Printer {
    output: String,
    indent: usize,
    comments: CommentMap,
    /// Track which leading comment lines have been emitted
    next_leading_line: usize,
}

impl Printer {
    pub fn format(program: &Program, comments: CommentMap) -> String {
        let mut printer = Self {
            output: String::new(),
            indent: 0,
            comments,
            next_leading_line: 1,
        };
        printer.print_program(program);

        // Ensure trailing newline
        if !printer.output.is_empty() && !printer.output.ends_with('\n') {
            printer.output.push('\n');
        }
        printer.output
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn begin_line(&mut self) {
        for _ in 0..self.indent {
            self.write("    ");
        }
    }

    fn emit_leading_comments(&mut self, up_to_line: usize) {
        // Emit all leading comments from next_leading_line up to (and including) the line
        // just before the declaration
        for line in self.next_leading_line..up_to_line {
            if let Some(comments) = self.comments.leading.remove(&line) {
                for c in comments {
                    self.begin_line();
                    self.write("//");
                    self.write(&c.text);
                    self.newline();
                }
            }
        }
        self.next_leading_line = up_to_line;
    }

    fn emit_trailing_comment(&mut self, line: usize) {
        if let Some(comment) = self.comments.trailing.remove(&line) {
            self.write("  //");
            self.write(&comment.text);
        }
    }

    // ---- Program ----

    fn print_program(&mut self, program: &Program) {
        // Emit docstring if present
        if let Some(ref doc) = program.docstring {
            self.write("\"\"\"");
            self.write(doc);
            self.write("\"\"\"");
            self.newline();
        }

        let mut prev_category: Option<DeclCategory> = None;

        for decl in &program.declarations {
            let category = categorize_decl(&decl.kind);

            // Blank line between different categories at top level
            if let Some(ref prev) = prev_category {
                if should_blank_between(prev, &category) {
                    self.newline();
                }
            }

            self.emit_leading_comments(decl.span.line);
            self.print_declaration(decl);
            prev_category = Some(category);
        }
    }

    fn print_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclKind::Var(v) => {
                self.begin_line();
                self.write("var ");
                self.write(&v.name);
                if let Some(ref ty) = v.type_ann {
                    self.write(": ");
                    self.print_type_ann(ty);
                }
                self.write(" = ");
                self.print_expr(&v.initializer);
                self.emit_trailing_comment(decl.span.line);
                self.newline();
            }
            DeclKind::Const(c) => {
                self.begin_line();
                self.write("const ");
                self.write(&c.name);
                if let Some(ref ty) = c.type_ann {
                    self.write(": ");
                    self.print_type_ann(ty);
                }
                self.write(" = ");
                self.print_expr(&c.initializer);
                self.emit_trailing_comment(decl.span.line);
                self.newline();
            }
            DeclKind::Fun(f) => self.print_fun_decl(f, decl.span),
            DeclKind::Class(c) => self.print_class_decl(c, decl.span),
            DeclKind::Enum(e) => self.print_enum_decl(e, decl.span),
            DeclKind::Import(i) => self.print_import(i, decl.span),
            DeclKind::Spec(s) => self.print_spec_decl(s, decl.span),
            DeclKind::Statement(s) => self.print_statement(s),
        }
    }

    // ---- Functions ----

    fn print_fun_decl(&mut self, f: &FunDecl, span: ni_error::Span) {
        self.begin_line();
        self.write("fun ");
        self.write(&f.name);
        self.write("(");
        self.print_params(&f.params);
        self.write(")");
        if let Some(ref ret) = f.return_type {
            self.write(" -> ");
            self.print_type_ann(ret);
        }
        self.write(":");
        self.emit_trailing_comment(span.line);
        self.newline();
        self.indent += 1;
        if let Some(ref doc) = f.docstring {
            self.begin_line();
            self.write("\"\"\"");
            self.write(doc);
            self.write("\"\"\"");
            self.newline();
        }
        self.print_body(&f.body);
        self.indent -= 1;
    }

    fn print_params(&mut self, params: &[Param]) {
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&param.name);
            if let Some(ref ty) = param.type_ann {
                self.write(": ");
                self.print_type_ann(ty);
            }
            if let Some(ref default) = param.default {
                self.write(" = ");
                self.print_expr(default);
            }
        }
    }

    fn print_type_ann(&mut self, ty: &TypeAnnotation) {
        self.write(&ty.name);
        if !ty.type_args.is_empty() {
            self.write("[");
            for (i, arg) in ty.type_args.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.print_type_ann(arg);
            }
            self.write("]");
        }
        if ty.optional {
            self.write("?");
        }
    }

    // ---- Classes ----

    fn print_class_decl(&mut self, c: &ClassDecl, span: ni_error::Span) {
        self.begin_line();
        self.write("class ");
        self.write(&c.name);
        if let Some(ref parent) = c.superclass {
            self.write(" extends ");
            self.write(parent);
        }
        self.write(":");
        self.emit_trailing_comment(span.line);
        self.newline();
        self.indent += 1;

        if let Some(ref doc) = c.docstring {
            self.begin_line();
            self.write("\"\"\"");
            self.write(doc);
            self.write("\"\"\"");
            self.newline();
        }

        // Fields
        for field in &c.fields {
            self.print_field(field, false);
        }
        for field in &c.static_fields {
            self.print_field(field, true);
        }

        // Blank line between fields and methods
        if (!c.fields.is_empty() || !c.static_fields.is_empty())
            && (!c.methods.is_empty() || !c.static_methods.is_empty())
        {
            self.newline();
        }

        // Methods with blank lines between them
        let all_methods: Vec<(&FunDecl, bool)> = c
            .methods
            .iter()
            .map(|m| (&m.fun, false))
            .chain(c.static_methods.iter().map(|m| (m, true)))
            .collect();

        for (i, (method, is_static)) in all_methods.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            if *is_static {
                self.begin_line();
                self.write("static ");
                // Adjust: remove the begin_line from print_fun_decl
                self.write("fun ");
                self.write(&method.name);
                self.write("(");
                self.print_params(&method.params);
                self.write(")");
                if let Some(ref ret) = method.return_type {
                    self.write(" -> ");
                    self.print_type_ann(ret);
                }
                self.write(":");
                self.newline();
                self.indent += 1;
                if let Some(ref doc) = method.docstring {
                    self.begin_line();
                    self.write("\"\"\"");
                    self.write(doc);
                    self.write("\"\"\"");
                    self.newline();
                }
                self.print_body(&method.body);
                self.indent -= 1;
            } else {
                self.print_fun_decl(method, span);
            }
        }

        self.indent -= 1;
    }

    fn print_field(&mut self, field: &FieldDecl, is_static: bool) {
        self.begin_line();
        if is_static {
            self.write("static ");
        }
        self.write("var ");
        self.write(&field.name);
        if let Some(ref ty) = field.type_ann {
            self.write(": ");
            self.print_type_ann(ty);
        }
        if let Some(ref default) = field.default {
            self.write(" = ");
            self.print_expr(default);
        }
        self.newline();
    }

    // ---- Enums ----

    fn print_enum_decl(&mut self, e: &EnumDecl, span: ni_error::Span) {
        self.begin_line();
        self.write("enum ");
        self.write(&e.name);
        self.write(":");
        self.emit_trailing_comment(span.line);
        self.newline();
        self.indent += 1;
        for variant in &e.variants {
            self.begin_line();
            self.write(&variant.name);
            if let Some(ref val) = variant.value {
                self.write(" = ");
                self.print_expr(val);
            }
            self.newline();
        }
        self.indent -= 1;
    }

    // ---- Imports ----

    fn print_import(&mut self, imp: &ImportDecl, span: ni_error::Span) {
        self.begin_line();
        match imp {
            ImportDecl::Module { path, alias } => {
                self.write("import ");
                self.write(&path.join("."));
                if let Some(a) = alias {
                    self.write(" as ");
                    self.write(a);
                }
            }
            ImportDecl::From { path, names } => {
                self.write("from ");
                self.write(&path.join("."));
                self.write(" import ");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&name.name);
                    if let Some(ref alias) = name.alias {
                        self.write(" as ");
                        self.write(alias);
                    }
                }
            }
            ImportDecl::FromAll { path } => {
                self.write("from ");
                self.write(&path.join("."));
                self.write(" import *");
            }
        }
        self.emit_trailing_comment(span.line);
        self.newline();
    }

    // ---- Specs ----

    fn print_spec_decl(&mut self, spec: &SpecDecl, span: ni_error::Span) {
        self.begin_line();
        self.write("spec \"");
        self.write(&spec.name);
        self.write("\":");
        self.emit_trailing_comment(span.line);
        self.newline();
        self.indent += 1;
        // Structured sections
        for section in &spec.sections {
            self.print_spec_section(section);
        }
        // Flat body
        self.print_body(&spec.body);
        self.indent -= 1;
    }

    fn print_spec_section(&mut self, section: &SpecSection) {
        self.begin_line();
        let keyword = match section.kind {
            SpecSectionKind::Given => "given",
            SpecSectionKind::When => "when",
            SpecSectionKind::Then => "then",
        };
        self.write(keyword);
        self.write(" \"");
        self.write(&section.label);
        self.write("\":");
        self.newline();
        self.indent += 1;
        self.print_body(&section.body);
        for child in &section.children {
            self.print_spec_section(child);
        }
        self.indent -= 1;
    }

    // ---- Statements ----

    fn print_body(&mut self, stmts: &[Statement]) {
        if stmts.is_empty() {
            self.begin_line();
            self.write("pass");
            self.newline();
            return;
        }
        for stmt in stmts {
            self.emit_leading_comments(stmt.span.line);
            self.print_statement(stmt);
        }
    }

    fn print_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StmtKind::Expr(e) => {
                self.begin_line();
                self.print_expr(e);
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::VarDecl(v) => {
                self.begin_line();
                self.write("var ");
                self.write(&v.name);
                if let Some(ref ty) = v.type_ann {
                    self.write(": ");
                    self.print_type_ann(ty);
                }
                self.write(" = ");
                self.print_expr(&v.initializer);
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::ConstDecl(c) => {
                self.begin_line();
                self.write("const ");
                self.write(&c.name);
                if let Some(ref ty) = c.type_ann {
                    self.write(": ");
                    self.print_type_ann(ty);
                }
                self.write(" = ");
                self.print_expr(&c.initializer);
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::If(if_stmt) => {
                self.begin_line();
                self.write("if ");
                self.print_expr(&if_stmt.condition);
                self.write(":");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
                self.indent += 1;
                self.print_body(&if_stmt.then_body);
                self.indent -= 1;

                for (cond, body) in &if_stmt.elif_branches {
                    self.begin_line();
                    self.write("elif ");
                    self.print_expr(cond);
                    self.write(":");
                    self.newline();
                    self.indent += 1;
                    self.print_body(body);
                    self.indent -= 1;
                }

                if let Some(ref body) = if_stmt.else_body {
                    self.begin_line();
                    self.write("else:");
                    self.newline();
                    self.indent += 1;
                    self.print_body(body);
                    self.indent -= 1;
                }
            }
            StmtKind::While(w) => {
                self.begin_line();
                self.write("while ");
                self.print_expr(&w.condition);
                self.write(":");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
                self.indent += 1;
                self.print_body(&w.body);
                self.indent -= 1;
            }
            StmtKind::For(f) => {
                self.begin_line();
                self.write("for ");
                self.write(&f.variable);
                if let Some(ref second) = f.second_var {
                    self.write(", ");
                    self.write(second);
                }
                self.write(" in ");
                self.print_expr(&f.iterable);
                self.write(":");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
                self.indent += 1;
                self.print_body(&f.body);
                self.indent -= 1;
            }
            StmtKind::Match(m) => {
                self.begin_line();
                self.write("match ");
                self.print_expr(&m.subject);
                self.write(":");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
                self.indent += 1;
                for case in &m.cases {
                    self.begin_line();
                    self.write("when ");
                    for (i, pat) in case.patterns.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.print_pattern(pat);
                    }
                    if let Some(ref guard) = case.guard {
                        self.write(" if ");
                        self.print_expr(guard);
                    }
                    self.write(":");
                    self.newline();
                    self.indent += 1;
                    self.print_body(&case.body);
                    self.indent -= 1;
                }
                self.indent -= 1;
            }
            StmtKind::Return(val) => {
                self.begin_line();
                self.write("return");
                if let Some(v) = val {
                    self.write(" ");
                    self.print_expr(v);
                }
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::Break => {
                self.begin_line();
                self.write("break");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::Continue => {
                self.begin_line();
                self.write("continue");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::Pass => {
                self.begin_line();
                self.write("pass");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::Block(stmts) => {
                self.print_body(stmts);
            }
            StmtKind::Try(t) => {
                self.begin_line();
                self.write("try:");
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
                self.indent += 1;
                self.print_body(&t.body);
                self.indent -= 1;
                self.begin_line();
                self.write("catch");
                if let Some(ref var) = t.catch_var {
                    self.write(" ");
                    self.write(var);
                }
                self.write(":");
                self.newline();
                self.indent += 1;
                match &t.catch_body {
                    CatchBody::Block(stmts) => self.print_body(stmts),
                    CatchBody::Match(cases) => {
                        for case in cases {
                            self.begin_line();
                            self.write("when ");
                            for (i, pat) in case.patterns.iter().enumerate() {
                                if i > 0 {
                                    self.write(", ");
                                }
                                self.print_pattern(pat);
                            }
                            self.write(":");
                            self.newline();
                            self.indent += 1;
                            self.print_body(&case.body);
                            self.indent -= 1;
                        }
                    }
                }
                self.indent -= 1;
            }
            StmtKind::Fail(e) => {
                self.begin_line();
                self.write("fail ");
                self.print_expr(e);
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
            StmtKind::Assert(e, msg) => {
                self.begin_line();
                self.write("assert ");
                self.print_expr(e);
                if let Some(m) = msg {
                    self.write(", ");
                    self.print_expr(m);
                }
                self.emit_trailing_comment(stmt.span.line);
                self.newline();
            }
        }
    }

    fn print_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Literal(e) => self.print_expr(e),
            Pattern::Wildcard => self.write("_"),
            Pattern::Binding(name) => self.write(name),
            Pattern::TypeCheck(name, ty) => {
                self.write(name);
                self.write(" is ");
                self.write(ty);
            }
        }
    }

    // ---- Expressions ----

    fn print_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::IntLiteral(n) => self.write(&n.to_string()),
            ExprKind::FloatLiteral(f) => self.write(&format_float(*f)),
            ExprKind::StringLiteral(s) => {
                self.write("\"");
                self.write(&escape_string(s));
                self.write("\"");
            }
            ExprKind::FStringLiteral(parts) => {
                self.write("`");
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => self.write(s),
                        FStringPart::Expr(e) => {
                            self.write("{");
                            self.print_expr(e);
                            self.write("}");
                        }
                    }
                }
                self.write("`");
            }
            ExprKind::BoolLiteral(b) => self.write(if *b { "true" } else { "false" }),
            ExprKind::NoneLiteral => self.write("none"),
            ExprKind::Identifier(name) => self.write(name),
            ExprKind::SelfExpr => self.write("self"),

            ExprKind::Negate(e) => {
                self.write("-");
                self.print_expr(e);
            }
            ExprKind::Not(e) => {
                self.write("not ");
                self.print_expr(e);
            }

            ExprKind::BinaryOp { left, op, right } => {
                self.print_expr(left);
                self.write(" ");
                self.write(binop_str(op));
                self.write(" ");
                self.print_expr(right);
            }
            ExprKind::Compare { left, op, right } => {
                self.print_expr(left);
                self.write(" ");
                self.write(cmpop_str(op));
                self.write(" ");
                self.print_expr(right);
            }
            ExprKind::And(l, r) => {
                self.print_expr(l);
                self.write(" and ");
                self.print_expr(r);
            }
            ExprKind::Or(l, r) => {
                self.print_expr(l);
                self.write(" or ");
                self.print_expr(r);
            }

            ExprKind::Assign { target, value } => {
                self.print_expr(target);
                self.write(" = ");
                self.print_expr(value);
            }
            ExprKind::CompoundAssign { target, op, value } => {
                self.print_expr(target);
                self.write(" ");
                self.write(binop_str(op));
                self.write("= ");
                self.print_expr(value);
            }

            ExprKind::GetField(obj, field) => {
                self.print_expr(obj);
                self.write(".");
                self.write(field);
            }
            ExprKind::SetField(obj, field, val) => {
                self.print_expr(obj);
                self.write(".");
                self.write(field);
                self.write(" = ");
                self.print_expr(val);
            }
            ExprKind::GetIndex(obj, idx) => {
                self.print_expr(obj);
                self.write("[");
                self.print_expr(idx);
                self.write("]");
            }
            ExprKind::SetIndex(obj, idx, val) => {
                self.print_expr(obj);
                self.write("[");
                self.print_expr(idx);
                self.write("] = ");
                self.print_expr(val);
            }
            ExprKind::SafeNav(obj, field) => {
                self.print_expr(obj);
                self.write("?.");
                self.write(field);
            }
            ExprKind::NoneCoalesce(left, right) => {
                self.print_expr(left);
                self.write(" ?? ");
                self.print_expr(right);
            }

            ExprKind::Call {
                callee,
                args,
                named_args,
            } => {
                self.print_expr(callee);
                self.write("(");
                self.print_arg_list(args, named_args);
                self.write(")");
            }
            ExprKind::MethodCall {
                object,
                method,
                args,
                named_args,
            } => {
                self.print_expr(object);
                self.write(".");
                self.write(method);
                self.write("(");
                self.print_arg_list(args, named_args);
                self.write(")");
            }
            ExprKind::SuperCall { method, args } => {
                self.write("super.");
                self.write(method);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(arg);
                }
                self.write(")");
            }

            ExprKind::List(items) => {
                self.write("[");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(item);
                }
                self.write("]");
            }
            ExprKind::Map(entries) => {
                if entries.is_empty() {
                    self.write("[:]");
                } else {
                    self.write("[");
                    for (i, (k, v)) in entries.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.print_expr(k);
                        self.write(": ");
                        self.print_expr(v);
                    }
                    self.write("]");
                }
            }

            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                self.print_expr(start);
                if *inclusive {
                    self.write("..=");
                } else {
                    self.write("..");
                }
                self.print_expr(end);
            }

            ExprKind::Lambda { params, body } => {
                self.write("fun(");
                self.print_params(params);
                self.write("):");
                if body.len() == 1 {
                    if let StmtKind::Expr(e) = &body[0].kind {
                        self.write(" ");
                        self.print_expr(e);
                    } else if let StmtKind::Return(Some(e)) = &body[0].kind {
                        self.write(" ");
                        self.print_expr(e);
                    } else {
                        self.newline();
                        self.indent += 1;
                        self.print_body(body);
                        self.indent -= 1;
                    }
                } else {
                    self.newline();
                    self.indent += 1;
                    self.print_body(body);
                    self.indent -= 1;
                }
            }

            ExprKind::IfExpr {
                value,
                condition,
                else_value,
            } => {
                self.print_expr(value);
                self.write(" if ");
                self.print_expr(condition);
                self.write(" else ");
                self.print_expr(else_value);
            }

            ExprKind::Spawn(e) => {
                self.write("spawn ");
                self.print_expr(e);
            }
            ExprKind::Yield(val) => {
                self.write("yield");
                if let Some(v) = val {
                    self.write(" ");
                    self.print_expr(v);
                }
            }
            ExprKind::Wait(e) => {
                self.write("wait ");
                self.print_expr(e);
            }
            ExprKind::Await(e) => {
                self.write("await ");
                self.print_expr(e);
            }
            ExprKind::TryExpr(e) => {
                self.write("try ");
                self.print_expr(e);
            }
            ExprKind::FailExpr(e) => {
                self.write("fail ");
                self.print_expr(e);
            }
        }
    }

    fn print_arg_list(&mut self, args: &[Expr], named_args: &[(String, Expr)]) {
        let mut first = true;
        for arg in args {
            if !first {
                self.write(", ");
            }
            self.print_expr(arg);
            first = false;
        }
        for (name, val) in named_args {
            if !first {
                self.write(", ");
            }
            self.write(name);
            self.write(" = ");
            self.print_expr(val);
            first = false;
        }
    }
}

// ---- Helpers ----

#[derive(PartialEq)]
enum DeclCategory {
    Import,
    VarConst,
    Function,
    Class,
    Enum,
    Spec,
    Statement,
}

fn categorize_decl(kind: &DeclKind) -> DeclCategory {
    match kind {
        DeclKind::Import(_) => DeclCategory::Import,
        DeclKind::Var(_) | DeclKind::Const(_) => DeclCategory::VarConst,
        DeclKind::Fun(_) => DeclCategory::Function,
        DeclKind::Class(_) => DeclCategory::Class,
        DeclKind::Enum(_) => DeclCategory::Enum,
        DeclKind::Spec(_) => DeclCategory::Spec,
        DeclKind::Statement(_) => DeclCategory::Statement,
    }
}

fn should_blank_between(prev: &DeclCategory, next: &DeclCategory) -> bool {
    // Always blank before function, class, enum, spec
    matches!(
        next,
        DeclCategory::Function | DeclCategory::Class | DeclCategory::Enum | DeclCategory::Spec
    ) ||
    // Blank after function, class, enum, spec
    matches!(
        prev,
        DeclCategory::Function | DeclCategory::Class | DeclCategory::Enum | DeclCategory::Spec
    ) ||
    // Blank between imports and non-imports
    (*prev == DeclCategory::Import && *next != DeclCategory::Import)
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
    }
}

fn cmpop_str(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "==",
        CmpOp::NotEq => "!=",
        CmpOp::Lt => "<",
        CmpOp::Gt => ">",
        CmpOp::LtEq => "<=",
        CmpOp::GtEq => ">=",
        CmpOp::Is => "is",
        CmpOp::In => "in",
    }
}

fn format_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
