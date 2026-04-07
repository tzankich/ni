use crate::ast::*;
use crate::expr::ExprParser;
use ni_error::{NiError, NiResult, Span};
use ni_lexer::{Token, TokenKind};

const MAX_NESTING_DEPTH: usize = 256;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    pub(crate) depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0, depth: 0 }
    }

    pub(crate) fn enter_nesting(&mut self) -> NiResult<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            Err(NiError::parse(
                "Maximum nesting depth exceeded (256)",
                self.current_span(),
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn exit_nesting(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub fn parse_program(&mut self) -> NiResult<Program> {
        self.skip_newlines();
        let mut declarations = Vec::new();
        while !self.is_at_end() {
            declarations.push(self.declaration()?);
            self.skip_newlines();
        }
        let docstring = Self::extract_docstring_from_decls(&mut declarations);
        Ok(Program {
            declarations,
            docstring,
        })
    }

    fn declaration(&mut self) -> NiResult<Declaration> {
        let span = self.current_span();
        let kind = match self.peek_kind() {
            TokenKind::Var => DeclKind::Var(self.var_decl()?),
            TokenKind::Const => DeclKind::Const(self.const_decl()?),
            TokenKind::Fun => DeclKind::Fun(self.fun_decl()?),
            TokenKind::Class => DeclKind::Class(self.class_decl()?),
            TokenKind::Enum => DeclKind::Enum(self.enum_decl()?),
            TokenKind::Import => DeclKind::Import(self.import_decl()?),
            TokenKind::From => DeclKind::Import(self.from_import_decl()?),
            TokenKind::Spec => DeclKind::Spec(self.spec_decl()?),
            _ => DeclKind::Statement(self.statement()?),
        };
        Ok(Declaration { kind, span })
    }

    fn var_decl(&mut self) -> NiResult<VarDecl> {
        self.expect(TokenKind::Var)?;
        let name = self.expect_identifier()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.expression()?;
        self.expect_line_end()?;
        Ok(VarDecl {
            name,
            type_ann: None,
            initializer,
        })
    }

    /// Parse `const identifier = expr` as an immutable binding.
    fn const_decl(&mut self) -> NiResult<ConstDecl> {
        self.expect(TokenKind::Const)?;
        let name = self.expect_identifier()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.expression()?;
        self.expect_line_end()?;
        Ok(ConstDecl {
            name,
            type_ann: None,
            initializer,
        })
    }

    fn fun_decl(&mut self) -> NiResult<FunDecl> {
        self.expect(TokenKind::Fun)?;
        let name = self.expect_identifier()?;
        self.expect(TokenKind::LeftParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RightParen)?;

        let return_type = if self.check(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;
        let mut body = self.block()?;
        let docstring = Self::extract_docstring(&mut body);

        Ok(FunDecl {
            name,
            params,
            return_type,
            body,
            docstring,
        })
    }

    fn class_decl(&mut self) -> NiResult<ClassDecl> {
        self.expect(TokenKind::Class)?;
        let name = self.expect_identifier()?;
        let superclass = if self.check(&TokenKind::Extends) {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon)?;
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut static_fields = Vec::new();
        let mut static_methods = Vec::new();
        // Check for docstring as first item in class body
        let docstring = if let TokenKind::StringLiteral(s) = self.peek_kind() {
            let ds = s.clone();
            self.advance();
            // Consume trailing newline after docstring
            if self.check(&TokenKind::Newline) {
                self.advance();
            }
            Some(ds)
        } else {
            None
        };

        while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            if self.check(&TokenKind::Static) {
                self.advance();
                if self.check(&TokenKind::Var) {
                    let field = self.class_field()?;
                    static_fields.push(field);
                } else if self.check(&TokenKind::Fun) {
                    let fun = self.fun_decl()?;
                    static_methods.push(fun);
                }
            } else if self.check(&TokenKind::Var) {
                let field = self.class_field()?;
                fields.push(field);
            } else if self.check(&TokenKind::Fun) {
                let fun = self.fun_decl()?;
                methods.push(MethodDecl { fun });
            } else {
                return Err(NiError::parse(
                    format!("Unexpected token in class body: {:?}", self.peek_kind()),
                    self.current_span(),
                ));
            }
            self.skip_newlines();
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(ClassDecl {
            name,
            superclass,
            docstring,
            fields,
            methods,
            static_fields,
            static_methods,
        })
    }

    fn class_field(&mut self) -> NiResult<FieldDecl> {
        self.expect(TokenKind::Var)?;
        let name = self.expect_identifier()?;
        let default = if self.check(&TokenKind::Equal) {
            self.advance();
            Some(self.expression()?)
        } else {
            None
        };
        self.expect_line_end()?;
        Ok(FieldDecl {
            name,
            type_ann: None,
            default,
        })
    }

    fn enum_decl(&mut self) -> NiResult<EnumDecl> {
        self.expect(TokenKind::Enum)?;
        let name = self.expect_identifier()?;
        self.expect(TokenKind::Colon)?;
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut variants = Vec::new();
        while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            let vname = self.expect_identifier()?;
            let value = if self.check(&TokenKind::Equal) {
                self.advance();
                Some(self.expression()?)
            } else {
                None
            };
            variants.push(EnumVariant { name: vname, value });
            self.expect_line_end()?;
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(EnumDecl { name, variants })
    }

    fn import_decl(&mut self) -> NiResult<ImportDecl> {
        self.expect(TokenKind::Import)?;
        let mut path = vec![self.expect_identifier()?];
        while self.check(&TokenKind::Dot) {
            self.advance();
            path.push(self.expect_identifier()?);
        }
        let alias = if self.check(&TokenKind::As) {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect_line_end()?;
        Ok(ImportDecl::Module { path, alias })
    }

    fn from_import_decl(&mut self) -> NiResult<ImportDecl> {
        self.expect(TokenKind::From)?;
        let mut path = vec![self.expect_identifier()?];
        while self.check(&TokenKind::Dot) {
            self.advance();
            path.push(self.expect_identifier()?);
        }
        self.expect(TokenKind::Import)?;
        if self.check(&TokenKind::Star) {
            self.advance();
            self.expect_line_end()?;
            return Ok(ImportDecl::FromAll { path });
        }
        let mut names = Vec::new();
        loop {
            let name = self.expect_identifier()?;
            let alias = if self.check(&TokenKind::As) {
                self.advance();
                Some(self.expect_identifier()?)
            } else {
                None
            };
            names.push(ImportName { name, alias });
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        self.expect_line_end()?;
        Ok(ImportDecl::From { path, names })
    }

    fn spec_decl(&mut self) -> NiResult<SpecDecl> {
        self.expect(TokenKind::Spec)?;
        let name = match self.peek_kind() {
            TokenKind::StringLiteral(s) => {
                self.advance();
                s
            }
            _ => {
                return Err(NiError::parse(
                    "Expected string literal for spec name",
                    self.current_span(),
                ));
            }
        };
        // Check for `each` modifier on the spec line: spec "name" each ...:
        let each = if self.check(&TokenKind::Each) {
            Some(self.parse_each_clause()?)
            // parse_each_clause consumed the trailing colon
        } else {
            self.expect(TokenKind::Colon)?;
            None
        };

        // Parse spec body: a mix of statements and BDD sections (given/when/then).
        // Statements before any BDD keyword go into `body`.
        // BDD sections go into `sections`.
        if each.is_none() {
            // Normal spec: colon was just consumed, expect newline+indent
            self.expect(TokenKind::Newline)?;
        }
        // After `each`, parse_each_clause already consumed colon + newlines
        self.expect(TokenKind::Indent)?;

        let mut body = Vec::new();
        let mut sections = Vec::new();

        while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            if self.check(&TokenKind::Given)
                || self.check(&TokenKind::When)
                || self.check(&TokenKind::Then)
            {
                let section_kind = match self.peek_kind() {
                    TokenKind::Given => SpecSectionKind::Given,
                    TokenKind::When => SpecSectionKind::When,
                    TokenKind::Then => SpecSectionKind::Then,
                    _ => unreachable!(),
                };
                sections.push(self.parse_spec_section(section_kind)?);
            } else {
                body.push(self.block_item()?);
            }
            self.skip_newlines();
        }
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(SpecDecl {
            name,
            body,
            sections,
            each,
        })
    }

    fn parse_each_clause(&mut self) -> NiResult<EachClause> {
        self.expect(TokenKind::Each)?;
        let mut items = Vec::new();
        if self.check(&TokenKind::LeftParen) {
            // Parenthesized form for multi-line: each (expr, expr, ...):
            self.advance(); // consume (
            if !self.check(&TokenKind::RightParen) {
                items.push(self.expression()?);
                while self.check(&TokenKind::Comma) {
                    self.advance();
                    if self.check(&TokenKind::RightParen) {
                        break; // trailing comma
                    }
                    items.push(self.expression()?);
                }
            }
            self.expect(TokenKind::RightParen)?;
        } else {
            // Inline form: each expr, expr, ...:
            items.push(self.expression()?);
            while self.check(&TokenKind::Comma) {
                self.advance();
                items.push(self.expression()?);
            }
        }
        self.expect(TokenKind::Colon)?;
        self.skip_newlines();
        Ok(EachClause { items })
    }

    fn parse_spec_section(&mut self, expected_kind: SpecSectionKind) -> NiResult<SpecSection> {
        let kind = match self.peek_kind() {
            TokenKind::Given => {
                self.advance();
                SpecSectionKind::Given
            }
            TokenKind::When => {
                self.advance();
                SpecSectionKind::When
            }
            TokenKind::Then => {
                self.advance();
                SpecSectionKind::Then
            }
            _ => {
                return Err(NiError::parse(
                    format!(
                        "Expected {:?} keyword in spec section, found {:?}",
                        expected_kind,
                        self.peek_kind()
                    ),
                    self.current_span(),
                ));
            }
        };

        let label = match self.peek_kind() {
            TokenKind::StringLiteral(s) => {
                self.advance();
                s
            }
            _ => {
                return Err(NiError::parse(
                    "Expected string literal for spec section label",
                    self.current_span(),
                ));
            }
        };
        self.expect(TokenKind::Colon)?;

        if kind == SpecSectionKind::Then {
            // Then is a leaf -- only statements, no child sections
            let body = self.block()?;
            return Ok(SpecSection {
                kind,
                label,
                body,
                children: vec![],
            });
        }

        // given/when: can contain statements and nested when/then sections
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut body = Vec::new();
        let mut children = Vec::new();
        let child_kind = if kind == SpecSectionKind::Given {
            SpecSectionKind::When
        } else {
            SpecSectionKind::Then
        };

        while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            // Check if next token is a child section keyword
            if (child_kind == SpecSectionKind::When && self.check(&TokenKind::When))
                || (child_kind == SpecSectionKind::Then && self.check(&TokenKind::Then))
                || (kind == SpecSectionKind::When && self.check(&TokenKind::When))
            {
                // Allow nested when inside when (for deeper nesting)
                let section_kind = match self.peek_kind() {
                    TokenKind::When => SpecSectionKind::When,
                    TokenKind::Then => SpecSectionKind::Then,
                    _ => unreachable!(),
                };
                children.push(self.parse_spec_section(section_kind)?);
            } else if self.check(&TokenKind::Then) && kind == SpecSectionKind::Given {
                // Allow then directly under given (skip when)
                children.push(self.parse_spec_section(SpecSectionKind::Then)?);
            } else {
                body.push(self.block_item()?);
            }
            self.skip_newlines();
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(SpecSection {
            kind,
            label,
            body,
            children,
        })
    }

    pub(crate) fn statement(&mut self) -> NiResult<Statement> {
        let span = self.current_span();
        let kind = match self.peek_kind() {
            TokenKind::If => StmtKind::If(self.if_stmt()?),
            TokenKind::While => StmtKind::While(self.while_stmt()?),
            TokenKind::For => StmtKind::For(self.for_stmt()?),
            TokenKind::Match => StmtKind::Match(self.match_stmt()?),
            TokenKind::Return => {
                self.advance();
                let value = if self.check(&TokenKind::Newline)
                    || self.check(&TokenKind::Eof)
                    || self.check(&TokenKind::Dedent)
                {
                    None
                } else {
                    Some(self.expression()?)
                };
                self.expect_line_end()?;
                StmtKind::Return(value)
            }
            TokenKind::Break => {
                self.advance();
                self.expect_line_end()?;
                StmtKind::Break
            }
            TokenKind::Continue => {
                self.advance();
                self.expect_line_end()?;
                StmtKind::Continue
            }
            TokenKind::Pass => {
                self.advance();
                self.expect_line_end()?;
                StmtKind::Pass
            }
            TokenKind::Try => StmtKind::Try(self.try_stmt()?),
            TokenKind::Fail => {
                self.advance();
                let expr = self.expression()?;
                self.expect_line_end()?;
                StmtKind::Fail(expr)
            }
            TokenKind::Assert => {
                self.advance();
                let condition = self.expression()?;
                let message = if self.check(&TokenKind::Comma) {
                    self.advance();
                    Some(self.expression()?)
                } else {
                    None
                };
                self.expect_line_end()?;
                StmtKind::Assert(condition, message)
            }
            _ => {
                let expr = self.expression()?;
                self.expect_line_end()?;
                StmtKind::Expr(expr)
            }
        };
        Ok(Statement { kind, span })
    }

    fn if_stmt(&mut self) -> NiResult<IfStmt> {
        self.expect(TokenKind::If)?;
        let condition = self.expression()?;
        self.expect(TokenKind::Colon)?;
        let then_body = self.block()?;

        let mut elif_branches = Vec::new();
        let mut else_body = None;

        while self.check(&TokenKind::Elif) {
            self.advance();
            let cond = self.expression()?;
            self.expect(TokenKind::Colon)?;
            let body = self.block()?;
            elif_branches.push((cond, body));
        }

        if self.check(&TokenKind::Else) {
            self.advance();
            self.expect(TokenKind::Colon)?;
            else_body = Some(self.block()?);
        }

        Ok(IfStmt {
            condition,
            then_body,
            elif_branches,
            else_body,
        })
    }

    fn while_stmt(&mut self) -> NiResult<WhileStmt> {
        self.expect(TokenKind::While)?;
        let condition = self.expression()?;
        self.expect(TokenKind::Colon)?;
        let body = self.block()?;
        Ok(WhileStmt { condition, body })
    }

    fn for_stmt(&mut self) -> NiResult<ForStmt> {
        self.expect(TokenKind::For)?;
        let variable = self.expect_identifier()?;
        let second_var = if self.check(&TokenKind::Comma) {
            self.advance();
            let first = variable.clone();
            let second = self.expect_identifier()?;
            self.expect(TokenKind::In)?;
            let iterable = self.expression()?;
            self.expect(TokenKind::Colon)?;
            let body = self.block()?;
            return Ok(ForStmt {
                variable: first,
                second_var: Some(second),
                iterable,
                body,
            });
        } else {
            None
        };
        self.expect(TokenKind::In)?;
        let iterable = self.expression()?;
        self.expect(TokenKind::Colon)?;
        let body = self.block()?;
        Ok(ForStmt {
            variable,
            second_var,
            iterable,
            body,
        })
    }

    fn match_stmt(&mut self) -> NiResult<MatchStmt> {
        self.expect(TokenKind::Match)?;
        let subject = self.expression()?;
        self.expect(TokenKind::Colon)?;
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut cases = Vec::new();
        while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
            self.skip_newlines();
            if self.check(&TokenKind::Dedent) || self.is_at_end() {
                break;
            }
            cases.push(self.match_case()?);
            self.skip_newlines();
        }

        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(MatchStmt { subject, cases })
    }

    fn match_case(&mut self) -> NiResult<MatchCase> {
        self.expect(TokenKind::When)?;

        let mut patterns = Vec::new();
        patterns.push(self.parse_pattern()?);

        while self.check(&TokenKind::Comma) {
            self.advance();
            patterns.push(self.parse_pattern()?);
        }

        // Guard
        let guard = if self.check(&TokenKind::If) {
            self.advance();
            Some(self.expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;
        let body = self.block()?;

        Ok(MatchCase {
            patterns,
            guard,
            body,
        })
    }

    fn parse_pattern(&mut self) -> NiResult<Pattern> {
        if self.check_identifier("_") {
            self.advance();
            return Ok(Pattern::Wildcard);
        }

        // Check for binding pattern: `name if` or `name is Type`
        if let TokenKind::Identifier(name) = self.peek_kind() {
            let name = name.clone();
            // Look ahead for `is` (type check pattern)
            if self.peek_offset(1).map(|t| &t.kind) == Some(&TokenKind::Is) {
                self.advance(); // consume identifier
                self.advance(); // consume `is`
                let type_name = self.expect_identifier()?;
                return Ok(Pattern::TypeCheck(name, type_name));
            }
            // If identifier is followed by `if` or `:`, it's a binding
            let next = self.peek_offset(1).map(|t| t.kind.clone());
            if matches!(next, Some(TokenKind::If) | Some(TokenKind::Colon)) {
                self.advance();
                return Ok(Pattern::Binding(name));
            }
        }

        // Literal pattern
        let expr = self.expression()?;
        Ok(Pattern::Literal(expr))
    }

    fn try_stmt(&mut self) -> NiResult<TryStmt> {
        self.expect(TokenKind::Try)?;
        self.expect(TokenKind::Colon)?;
        let body = self.block()?;

        self.expect(TokenKind::Catch)?;
        let catch_var = if !self.check(&TokenKind::Colon) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon)?;

        // Check if catch body uses match-style case branches
        let catch_body = if self.check(&TokenKind::Newline) {
            // Peek ahead to see if first statement in block is `case`
            let saved = self.current;
            self.advance(); // consume newline
            if self.check(&TokenKind::Indent) {
                self.advance(); // consume indent
                if self.check(&TokenKind::When) {
                    // Parse match cases
                    let mut cases = Vec::new();
                    while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
                        self.skip_newlines();
                        if self.check(&TokenKind::Dedent) || self.is_at_end() {
                            break;
                        }
                        cases.push(self.match_case()?);
                        self.skip_newlines();
                    }
                    if self.check(&TokenKind::Dedent) {
                        self.advance();
                    }
                    CatchBody::Match(cases)
                } else {
                    // Not case-based -- restore and parse normally
                    self.current = saved;
                    CatchBody::Block(self.block()?)
                }
            } else {
                // No indent -- restore and parse normally
                self.current = saved;
                CatchBody::Block(self.block()?)
            }
        } else {
            // Single-line catch body
            let stmt = self.statement()?;
            CatchBody::Block(vec![stmt])
        };

        Ok(TryStmt {
            body,
            catch_var,
            catch_body,
        })
    }

    pub(crate) fn block(&mut self) -> NiResult<Vec<Statement>> {
        self.enter_nesting()?;
        let result = self.block_inner();
        self.exit_nesting();
        result
    }

    fn block_inner(&mut self) -> NiResult<Vec<Statement>> {
        // A block can be a single-line statement or an indented block
        if self.check(&TokenKind::Newline) {
            self.advance();
            self.expect(TokenKind::Indent)?;
            let mut stmts = Vec::new();
            while !self.check(&TokenKind::Dedent) && !self.is_at_end() {
                self.skip_newlines();
                if self.check(&TokenKind::Dedent) || self.is_at_end() {
                    break;
                }
                let stmt = self.block_item()?;
                stmts.push(stmt);
                self.skip_newlines();
            }
            if self.check(&TokenKind::Dedent) {
                self.advance();
            }
            Ok(stmts)
        } else {
            // Single-line block (e.g., `if x: do_thing()`)
            let stmt = self.statement()?;
            Ok(vec![stmt])
        }
    }

    fn block_item(&mut self) -> NiResult<Statement> {
        let span = self.current_span();
        match self.peek_kind() {
            TokenKind::Var => {
                let decl = self.var_decl()?;
                Ok(Statement {
                    kind: StmtKind::VarDecl(decl),
                    span,
                })
            }
            TokenKind::Const => {
                let decl = self.const_decl()?;
                Ok(Statement {
                    kind: StmtKind::ConstDecl(decl),
                    span,
                })
            }
            TokenKind::Fun => {
                let decl = self.fun_decl()?;
                // Nested function declaration: compile as var + lambda
                Ok(Statement {
                    kind: StmtKind::VarDecl(VarDecl {
                        name: decl.name.clone(),
                        type_ann: None,
                        initializer: Expr {
                            kind: ExprKind::Lambda {
                                params: decl.params,
                                body: decl.body,
                            },
                            span,
                        },
                    }),
                    span,
                })
            }
            _ => self.statement(),
        }
    }

    fn parse_params(&mut self) -> NiResult<Vec<Param>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RightParen) {
            return Ok(params);
        }

        loop {
            let name = self.expect_identifier()?;
            let type_ann = if self.check(&TokenKind::Colon) {
                self.advance();
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            let default = if self.check(&TokenKind::Equal) {
                self.advance();
                Some(self.expression()?)
            } else {
                None
            };
            params.push(Param {
                name,
                type_ann,
                default,
            });
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        Ok(params)
    }

    pub(crate) fn parse_type_annotation(&mut self) -> NiResult<TypeAnnotation> {
        let name = self.expect_identifier()?;
        let mut type_args = Vec::new();

        if self.check(&TokenKind::LeftBracket) {
            self.advance();
            loop {
                type_args.push(self.parse_type_annotation()?);
                if !self.check(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RightBracket)?;
        }

        // Check for optional `?`
        // We don't have a `?` token, so skip for now

        Ok(TypeAnnotation {
            name,
            optional: false,
            type_args,
        })
    }

    // ---- Docstring extraction ----

    /// Extract a docstring from a function/method body: if the first statement
    /// is a bare string literal, remove it and return its value.
    fn extract_docstring(body: &mut Vec<Statement>) -> Option<String> {
        if let Some(first) = body.first() {
            if let StmtKind::Expr(Expr {
                kind: ExprKind::StringLiteral(s),
                ..
            }) = &first.kind
            {
                let ds = s.clone();
                body.remove(0);
                return Some(ds);
            }
        }
        None
    }

    /// Extract a docstring from top-level declarations: if the first declaration
    /// is a bare string literal expression statement, remove it and return its value.
    fn extract_docstring_from_decls(decls: &mut Vec<Declaration>) -> Option<String> {
        if let Some(first) = decls.first() {
            if let DeclKind::Statement(Statement {
                kind:
                    StmtKind::Expr(Expr {
                        kind: ExprKind::StringLiteral(s),
                        ..
                    }),
                ..
            }) = &first.kind
            {
                let ds = s.clone();
                decls.remove(0);
                return Some(ds);
            }
        }
        None
    }

    // ---- Expression parsing (delegates to Pratt parser) ----

    pub(crate) fn expression(&mut self) -> NiResult<Expr> {
        ExprParser::parse(self)
    }

    // ---- Token navigation ----

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }

    pub(crate) fn peek_kind(&self) -> TokenKind {
        self.peek().kind.clone()
    }

    pub(crate) fn peek_offset(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.current + offset)
    }

    pub(crate) fn current_span(&self) -> Span {
        self.peek().span
    }

    pub(crate) fn advance(&mut self) -> &Token {
        let token = &self.tokens[self.current];
        if self.current < self.tokens.len() - 1 {
            self.current += 1;
        }
        token
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn check_identifier(&self, name: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Identifier(n) if n == name)
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> NiResult<Token> {
        if self.check(&kind) {
            Ok(self.advance().clone())
        } else {
            Err(NiError::parse(
                format!("Expected {:?}, found {:?}", kind, self.peek_kind()),
                self.current_span(),
            ))
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> NiResult<String> {
        let kind = self.peek_kind();
        match kind {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            TokenKind::Spawn => {
                self.advance();
                Ok("spawn".into())
            }
            TokenKind::Type => {
                self.advance();
                Ok("type".into())
            }
            _ => Err(NiError::parse(
                format!("Expected identifier, found {:?}", self.peek_kind()),
                self.current_span(),
            )),
        }
    }

    fn expect_line_end(&mut self) -> NiResult<()> {
        match self.peek_kind() {
            TokenKind::Newline => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            TokenKind::Dedent => Ok(()), // Don't consume dedent
            _ => Err(NiError::parse(
                format!("Expected end of line, found {:?}", self.peek_kind()),
                self.current_span(),
            )),
        }
    }

    pub(crate) fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    pub(crate) fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }
}
