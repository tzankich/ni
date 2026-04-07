use crate::ast::*;
use crate::parser::Parser;
use ni_error::{NiError, NiResult, Span};
use ni_lexer::TokenKind;

// Binding powers (Pratt parser)
const BP_NONE: u8 = 0;
const BP_ASSIGNMENT: u8 = 2; // = += -= etc
const BP_OR: u8 = 4; // or
const BP_AND: u8 = 6; // and
const BP_EQUALITY: u8 = 8; // == != is in
const BP_COMPARISON: u8 = 10; // < > <= >=
const BP_RANGE: u8 = 12; // .. ..=
const BP_ADDITION: u8 = 14; // + -
const BP_MULTIPLY: u8 = 16; // * / %
const BP_UNARY: u8 = 18; // - not
const BP_CALL: u8 = 20; // () [] . ?.
const BP_TERNARY: u8 = 3; // if/else ternary

pub struct ExprParser;

impl ExprParser {
    pub fn parse(parser: &mut Parser) -> NiResult<Expr> {
        Self::parse_bp(parser, BP_NONE)
    }

    fn parse_bp(parser: &mut Parser, min_bp: u8) -> NiResult<Expr> {
        let mut left = Self::parse_prefix(parser)?;

        loop {
            // Check for ternary `if` expression: `value if condition else other`
            if parser.check(&TokenKind::If) && min_bp < BP_TERNARY {
                parser.advance();
                let condition = Self::parse_bp(parser, BP_TERNARY)?;
                parser.expect(TokenKind::Else)?;
                let else_value = Self::parse_bp(parser, BP_NONE)?;
                left = Expr {
                    span: left.span.merge(else_value.span),
                    kind: ExprKind::IfExpr {
                        value: Box::new(left),
                        condition: Box::new(condition),
                        else_value: Box::new(else_value),
                    },
                };
                continue;
            }

            let Some((bp, assoc)) = Self::infix_bp(parser) else {
                break;
            };

            let effective_bp = match assoc {
                Assoc::Left => bp,
                Assoc::Right => bp - 1,
            };

            if effective_bp < min_bp {
                break;
            }

            left = Self::parse_infix(parser, left, bp)?;
        }

        Ok(left)
    }

    fn parse_prefix(parser: &mut Parser) -> NiResult<Expr> {
        parser.enter_nesting()?;
        let result = Self::parse_prefix_inner(parser);
        parser.exit_nesting();
        result
    }

    fn parse_prefix_inner(parser: &mut Parser) -> NiResult<Expr> {
        let span = parser.current_span();
        match parser.peek_kind() {
            TokenKind::IntLiteral(n) => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::IntLiteral(n),
                    span,
                })
            }
            TokenKind::FloatLiteral(n) => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::FloatLiteral(n),
                    span,
                })
            }
            TokenKind::StringLiteral(s) => {
                parser.advance();
                if let Some(content) = s.strip_prefix("__fstring__:") {
                    let parts = Self::parse_fstring_parts(parser, content, span)?;
                    Ok(Expr {
                        kind: ExprKind::FStringLiteral(parts),
                        span,
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::StringLiteral(s),
                        span,
                    })
                }
            }
            TokenKind::True => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::BoolLiteral(true),
                    span,
                })
            }
            TokenKind::False => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::BoolLiteral(false),
                    span,
                })
            }
            TokenKind::None => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::NoneLiteral,
                    span,
                })
            }
            TokenKind::SelfKw => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::SelfExpr,
                    span,
                })
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::Identifier(name),
                    span,
                })
            }
            TokenKind::Type => {
                parser.advance();
                Ok(Expr {
                    kind: ExprKind::Identifier("type".to_string()),
                    span,
                })
            }
            TokenKind::Super => {
                parser.advance();
                parser.expect(TokenKind::Dot)?;
                let method = parser.expect_identifier()?;
                if parser.check(&TokenKind::LeftParen) {
                    parser.advance();
                    let args = Self::parse_args(parser)?;
                    parser.expect(TokenKind::RightParen)?;
                    Ok(Expr {
                        span: span.merge(parser.current_span()),
                        kind: ExprKind::SuperCall { method, args },
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::SuperCall {
                            method,
                            args: Vec::new(),
                        },
                        span,
                    })
                }
            }
            TokenKind::Minus => {
                parser.advance();
                let operand = Self::parse_bp(parser, BP_UNARY)?;
                Ok(Expr {
                    span: span.merge(operand.span),
                    kind: ExprKind::Negate(Box::new(operand)),
                })
            }
            TokenKind::Not => {
                parser.advance();
                let operand = Self::parse_bp(parser, BP_UNARY)?;
                Ok(Expr {
                    span: span.merge(operand.span),
                    kind: ExprKind::Not(Box::new(operand)),
                })
            }
            TokenKind::LeftParen => {
                parser.advance();
                let expr = Self::parse_bp(parser, BP_NONE)?;
                parser.expect(TokenKind::RightParen)?;
                Ok(expr)
            }
            TokenKind::LeftBracket => {
                parser.advance();
                // Check for map literal: ["key": value, ...]
                if parser.check(&TokenKind::Colon) {
                    // Empty map [:]
                    parser.advance();
                    parser.expect(TokenKind::RightBracket)?;
                    return Ok(Expr {
                        kind: ExprKind::Map(Vec::new()),
                        span,
                    });
                }
                if parser.check(&TokenKind::RightBracket) {
                    parser.advance();
                    return Ok(Expr {
                        kind: ExprKind::List(Vec::new()),
                        span,
                    });
                }

                let first = Self::parse_bp(parser, BP_NONE)?;
                if parser.check(&TokenKind::Colon) {
                    // Map literal
                    parser.advance();
                    let first_val = Self::parse_bp(parser, BP_NONE)?;
                    let mut entries = vec![(first, first_val)];
                    while parser.check(&TokenKind::Comma) {
                        parser.advance();
                        if parser.check(&TokenKind::RightBracket) {
                            break;
                        }
                        let key = Self::parse_bp(parser, BP_NONE)?;
                        parser.expect(TokenKind::Colon)?;
                        let val = Self::parse_bp(parser, BP_NONE)?;
                        entries.push((key, val));
                    }
                    parser.expect(TokenKind::RightBracket)?;
                    Ok(Expr {
                        kind: ExprKind::Map(entries),
                        span,
                    })
                } else {
                    // List literal
                    let mut elements = vec![first];
                    while parser.check(&TokenKind::Comma) {
                        parser.advance();
                        if parser.check(&TokenKind::RightBracket) {
                            break;
                        }
                        elements.push(Self::parse_bp(parser, BP_NONE)?);
                    }
                    parser.expect(TokenKind::RightBracket)?;
                    Ok(Expr {
                        kind: ExprKind::List(elements),
                        span,
                    })
                }
            }
            TokenKind::LeftBrace => {
                parser.advance();
                if parser.check(&TokenKind::RightBrace) {
                    // Empty map: {}
                    parser.advance();
                    return Ok(Expr {
                        kind: ExprKind::Map(Vec::new()),
                        span,
                    });
                }
                let first = Self::parse_bp(parser, BP_NONE)?;
                parser.expect(TokenKind::Colon)?;
                let first_val = Self::parse_bp(parser, BP_NONE)?;
                let mut entries = vec![(first, first_val)];
                while parser.check(&TokenKind::Comma) {
                    parser.advance();
                    if parser.check(&TokenKind::RightBrace) {
                        break;
                    } // trailing comma
                    let key = Self::parse_bp(parser, BP_NONE)?;
                    parser.expect(TokenKind::Colon)?;
                    let val = Self::parse_bp(parser, BP_NONE)?;
                    entries.push((key, val));
                }
                parser.expect(TokenKind::RightBrace)?;
                Ok(Expr {
                    kind: ExprKind::Map(entries),
                    span,
                })
            }
            TokenKind::Fun => {
                // Lambda: fun(params): body
                parser.advance();
                parser.expect(TokenKind::LeftParen)?;
                let params = Self::parse_lambda_params(parser)?;
                parser.expect(TokenKind::RightParen)?;
                parser.expect(TokenKind::Colon)?;

                // Could be single expression or block
                let body = if parser.check(&TokenKind::Newline) {
                    parser.block()?
                } else {
                    let expr = Self::parse_bp(parser, BP_NONE)?;
                    let expr_span = expr.span;
                    vec![Statement {
                        kind: StmtKind::Return(Some(expr)),
                        span: expr_span,
                    }]
                };

                Ok(Expr {
                    kind: ExprKind::Lambda { params, body },
                    span,
                })
            }
            TokenKind::Spawn => {
                parser.advance();
                let call = Self::parse_bp(parser, BP_CALL)?;
                Ok(Expr {
                    span: span.merge(call.span),
                    kind: ExprKind::Spawn(Box::new(call)),
                })
            }
            TokenKind::Yield => {
                parser.advance();
                // yield with no value vs yield <expr>
                let value = match parser.peek_kind() {
                    TokenKind::Newline | TokenKind::Eof | TokenKind::Dedent => None,
                    _ => {
                        let expr = Self::parse_bp(parser, BP_ASSIGNMENT)?;
                        Some(Box::new(expr))
                    }
                };
                Ok(Expr {
                    span: span.merge(value.as_ref().map(|v| v.span).unwrap_or(span)),
                    kind: ExprKind::Yield(value),
                })
            }
            TokenKind::Wait => {
                parser.advance();
                let duration = Self::parse_bp(parser, BP_CALL)?;
                Ok(Expr {
                    span: span.merge(duration.span),
                    kind: ExprKind::Wait(Box::new(duration)),
                })
            }
            TokenKind::Await => {
                parser.advance();
                let inner = Self::parse_bp(parser, BP_CALL)?;
                Ok(Expr {
                    span: span.merge(inner.span),
                    kind: ExprKind::Await(Box::new(inner)),
                })
            }
            TokenKind::Try => {
                parser.advance();
                let inner = Self::parse_bp(parser, BP_UNARY)?;
                Ok(Expr {
                    span: span.merge(inner.span),
                    kind: ExprKind::TryExpr(Box::new(inner)),
                })
            }
            TokenKind::Fail => {
                parser.advance();
                let value = Self::parse_bp(parser, BP_UNARY)?;
                Ok(Expr {
                    span: span.merge(value.span),
                    kind: ExprKind::FailExpr(Box::new(value)),
                })
            }
            _ => Err(NiError::parse(
                format!("Expected expression, found {:?}", parser.peek_kind()),
                span,
            )),
        }
    }

    fn infix_bp(parser: &Parser) -> Option<(u8, Assoc)> {
        match parser.peek_kind() {
            // Assignment
            TokenKind::Equal => Some((BP_ASSIGNMENT, Assoc::Right)),
            TokenKind::PlusEqual
            | TokenKind::MinusEqual
            | TokenKind::StarEqual
            | TokenKind::SlashEqual
            | TokenKind::PercentEqual => Some((BP_ASSIGNMENT, Assoc::Right)),

            // Logic
            TokenKind::Or => Some((BP_OR, Assoc::Left)),
            TokenKind::And => Some((BP_AND, Assoc::Left)),

            // None coalescing
            TokenKind::QuestionQuestion => Some((BP_OR, Assoc::Left)),

            // Equality
            TokenKind::EqualEqual | TokenKind::BangEqual | TokenKind::Is | TokenKind::In => {
                Some((BP_EQUALITY, Assoc::Left))
            }

            // Comparison
            TokenKind::Less
            | TokenKind::Greater
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual => Some((BP_COMPARISON, Assoc::Left)),

            // Range
            TokenKind::DotDot | TokenKind::DotDotEqual => Some((BP_RANGE, Assoc::Left)),

            // Arithmetic
            TokenKind::Plus | TokenKind::Minus => Some((BP_ADDITION, Assoc::Left)),
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => {
                Some((BP_MULTIPLY, Assoc::Left))
            }

            // Postfix
            TokenKind::LeftParen
            | TokenKind::LeftBracket
            | TokenKind::Dot
            | TokenKind::QuestionDot => Some((BP_CALL, Assoc::Left)),

            _ => None,
        }
    }

    fn parse_infix(parser: &mut Parser, left: Expr, _bp: u8) -> NiResult<Expr> {
        let span = left.span;
        match parser.peek_kind() {
            // Assignment
            TokenKind::Equal => {
                parser.advance();
                let value = Self::parse_bp(parser, BP_ASSIGNMENT - 1)?;
                Ok(Expr {
                    span: span.merge(value.span),
                    kind: ExprKind::Assign {
                        target: Box::new(left),
                        value: Box::new(value),
                    },
                })
            }

            TokenKind::PlusEqual => Self::compound_assign(parser, left, BinOp::Add),
            TokenKind::MinusEqual => Self::compound_assign(parser, left, BinOp::Sub),
            TokenKind::StarEqual => Self::compound_assign(parser, left, BinOp::Mul),
            TokenKind::SlashEqual => Self::compound_assign(parser, left, BinOp::Div),
            TokenKind::PercentEqual => Self::compound_assign(parser, left, BinOp::Mod),

            // Logic
            TokenKind::And => {
                parser.advance();
                let right = Self::parse_bp(parser, BP_AND)?;
                Ok(Expr {
                    span: span.merge(right.span),
                    kind: ExprKind::And(Box::new(left), Box::new(right)),
                })
            }
            TokenKind::Or => {
                parser.advance();
                let right = Self::parse_bp(parser, BP_OR)?;
                Ok(Expr {
                    span: span.merge(right.span),
                    kind: ExprKind::Or(Box::new(left), Box::new(right)),
                })
            }

            // None coalescing
            TokenKind::QuestionQuestion => {
                parser.advance();
                let right = Self::parse_bp(parser, BP_OR)?;
                Ok(Expr {
                    span: span.merge(right.span),
                    kind: ExprKind::NoneCoalesce(Box::new(left), Box::new(right)),
                })
            }

            // Comparison
            TokenKind::EqualEqual => Self::comparison(parser, left, CmpOp::Eq),
            TokenKind::BangEqual => Self::comparison(parser, left, CmpOp::NotEq),
            TokenKind::Less => Self::comparison(parser, left, CmpOp::Lt),
            TokenKind::Greater => Self::comparison(parser, left, CmpOp::Gt),
            TokenKind::LessEqual => Self::comparison(parser, left, CmpOp::LtEq),
            TokenKind::GreaterEqual => Self::comparison(parser, left, CmpOp::GtEq),
            TokenKind::Is => Self::comparison(parser, left, CmpOp::Is),
            TokenKind::In => Self::comparison(parser, left, CmpOp::In),

            // Range
            TokenKind::DotDot => {
                parser.advance();
                let right = Self::parse_bp(parser, BP_RANGE + 1)?;
                Ok(Expr {
                    span: span.merge(right.span),
                    kind: ExprKind::Range {
                        start: Box::new(left),
                        end: Box::new(right),
                        inclusive: false,
                    },
                })
            }
            TokenKind::DotDotEqual => {
                parser.advance();
                let right = Self::parse_bp(parser, BP_RANGE + 1)?;
                Ok(Expr {
                    span: span.merge(right.span),
                    kind: ExprKind::Range {
                        start: Box::new(left),
                        end: Box::new(right),
                        inclusive: true,
                    },
                })
            }

            // Arithmetic
            TokenKind::Plus => Self::binary(parser, left, BinOp::Add, BP_ADDITION),
            TokenKind::Minus => Self::binary(parser, left, BinOp::Sub, BP_ADDITION),
            TokenKind::Star => Self::binary(parser, left, BinOp::Mul, BP_MULTIPLY),
            TokenKind::Slash => Self::binary(parser, left, BinOp::Div, BP_MULTIPLY),
            TokenKind::Percent => Self::binary(parser, left, BinOp::Mod, BP_MULTIPLY),

            // Call
            TokenKind::LeftParen => {
                parser.advance();
                let (args, named_args) = Self::parse_call_args(parser)?;
                let end_span = parser.current_span();
                parser.expect(TokenKind::RightParen)?;
                // Check if this is a method call (left is GetField)
                match left.kind {
                    ExprKind::GetField(obj, method) => Ok(Expr {
                        span: span.merge(end_span),
                        kind: ExprKind::MethodCall {
                            object: obj,
                            method,
                            args,
                            named_args,
                        },
                    }),
                    _ => Ok(Expr {
                        span: span.merge(end_span),
                        kind: ExprKind::Call {
                            callee: Box::new(left),
                            args,
                            named_args,
                        },
                    }),
                }
            }

            // Index
            TokenKind::LeftBracket => {
                parser.advance();
                let index = Self::parse_bp(parser, BP_NONE)?;
                let end_span = parser.current_span();
                parser.expect(TokenKind::RightBracket)?;
                Ok(Expr {
                    span: span.merge(end_span),
                    kind: ExprKind::GetIndex(Box::new(left), Box::new(index)),
                })
            }

            // Member access
            TokenKind::Dot => {
                parser.advance();
                let name = parser.expect_identifier()?;
                Ok(Expr {
                    span: span.merge(parser.current_span()),
                    kind: ExprKind::GetField(Box::new(left), name),
                })
            }

            // Safe navigation
            TokenKind::QuestionDot => {
                parser.advance();
                let name = parser.expect_identifier()?;
                Ok(Expr {
                    span: span.merge(parser.current_span()),
                    kind: ExprKind::SafeNav(Box::new(left), name),
                })
            }

            _ => Err(NiError::parse(
                format!("Unexpected infix token: {:?}", parser.peek_kind()),
                parser.current_span(),
            )),
        }
    }

    fn binary(parser: &mut Parser, left: Expr, op: BinOp, bp: u8) -> NiResult<Expr> {
        let span = left.span;
        parser.advance();
        let right = Self::parse_bp(parser, bp + 1)?;
        Ok(Expr {
            span: span.merge(right.span),
            kind: ExprKind::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
        })
    }

    fn comparison(parser: &mut Parser, left: Expr, op: CmpOp) -> NiResult<Expr> {
        let span = left.span;
        parser.advance();
        let right = Self::parse_bp(parser, BP_EQUALITY + 1)?;
        Ok(Expr {
            span: span.merge(right.span),
            kind: ExprKind::Compare {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
        })
    }

    fn compound_assign(parser: &mut Parser, left: Expr, op: BinOp) -> NiResult<Expr> {
        let span = left.span;
        parser.advance();
        let value = Self::parse_bp(parser, BP_ASSIGNMENT - 1)?;
        Ok(Expr {
            span: span.merge(value.span),
            kind: ExprKind::CompoundAssign {
                target: Box::new(left),
                op,
                value: Box::new(value),
            },
        })
    }

    fn parse_args(parser: &mut Parser) -> NiResult<Vec<Expr>> {
        let mut args = Vec::new();
        if parser.check(&TokenKind::RightParen) {
            return Ok(args);
        }
        loop {
            args.push(Self::parse_bp(parser, BP_NONE)?);
            if !parser.check(&TokenKind::Comma) {
                break;
            }
            parser.advance();
        }
        Ok(args)
    }

    fn parse_call_args(parser: &mut Parser) -> NiResult<(Vec<Expr>, Vec<(String, Expr)>)> {
        let mut args = Vec::new();
        let mut named_args = Vec::new();

        if parser.check(&TokenKind::RightParen) {
            return Ok((args, named_args));
        }

        loop {
            // Check for named argument: `name = expr`
            if let TokenKind::Identifier(name) = parser.peek_kind() {
                if parser.peek_offset(1).map(|t| &t.kind) == Some(&TokenKind::Equal) {
                    let name = name.clone();
                    parser.advance(); // identifier
                    parser.advance(); // =
                    let value = Self::parse_bp(parser, BP_NONE)?;
                    named_args.push((name, value));
                    if !parser.check(&TokenKind::Comma) {
                        break;
                    }
                    parser.advance();
                    continue;
                }
            }
            if !named_args.is_empty() {
                return Err(NiError::parse(
                    "Positional arguments must come before named arguments",
                    parser.current_span(),
                ));
            }
            args.push(Self::parse_bp(parser, BP_NONE)?);
            if !parser.check(&TokenKind::Comma) {
                break;
            }
            parser.advance();
        }

        Ok((args, named_args))
    }

    fn parse_lambda_params(parser: &mut Parser) -> NiResult<Vec<Param>> {
        let mut params = Vec::new();
        if parser.check(&TokenKind::RightParen) {
            return Ok(params);
        }
        loop {
            let name = parser.expect_identifier()?;
            let type_ann = if parser.check(&TokenKind::Colon) {
                parser.advance();
                Some(parser.parse_type_annotation()?)
            } else {
                None
            };
            let default = if parser.check(&TokenKind::Equal) {
                parser.advance();
                Some(Self::parse_bp(parser, BP_NONE)?)
            } else {
                None
            };
            params.push(Param {
                name,
                type_ann,
                default,
            });
            if !parser.check(&TokenKind::Comma) {
                break;
            }
            parser.advance();
        }
        Ok(params)
    }

    fn parse_fstring_parts(
        parser: &mut Parser,
        content: &str,
        _span: Span,
    ) -> NiResult<Vec<FStringPart>> {
        let mut parts = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = content.chars().collect();

        while i < chars.len() {
            if i + 5 < chars.len()
                && chars[i] == '{'
                && chars[i + 1] == 'E'
                && chars[i + 2] == 'X'
                && chars[i + 3] == 'P'
                && chars[i + 4] == 'R'
                && chars[i + 5] == ':'
            {
                // Find the closing }
                let start = i + 6;
                let mut depth = 1;
                let mut end = start;
                while end < chars.len() && depth > 0 {
                    if chars[end] == '{' {
                        depth += 1;
                    }
                    if chars[end] == '}' {
                        depth -= 1;
                    }
                    if depth > 0 {
                        end += 1;
                    }
                }
                let expr_str: String = chars[start..end].iter().collect();
                // Parse the expression
                let tokens = ni_lexer::lex(&expr_str)?;
                let mut sub_parser = Parser::new(tokens);
                sub_parser.depth = parser.depth; // inherit depth to prevent f-string recursion bombs
                let expr = ExprParser::parse(&mut sub_parser)?;
                parts.push(FStringPart::Expr(expr));
                i = end + 1;
            } else if chars[i] == '\\' && i + 1 < chars.len() {
                match chars[i + 1] {
                    '{' => {
                        parts.push(FStringPart::Literal("{".to_string()));
                        i += 2;
                    }
                    '}' => {
                        parts.push(FStringPart::Literal("}".to_string()));
                        i += 2;
                    }
                    _ => {
                        parts.push(FStringPart::Literal(chars[i].to_string()));
                        i += 1;
                    }
                }
            } else {
                // Collect plain text
                let start = i;
                while i < chars.len() && chars[i] != '{' && chars[i] != '\\' {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                if !text.is_empty() {
                    // Merge with previous literal if possible
                    if let Some(FStringPart::Literal(ref mut prev)) = parts.last_mut() {
                        prev.push_str(&text);
                    } else {
                        parts.push(FStringPart::Literal(text));
                    }
                }
            }
        }

        if parts.is_empty() {
            parts.push(FStringPart::Literal(String::new()));
        }

        Ok(parts)
    }
}

enum Assoc {
    Left,
    Right,
}
