use ni_lexer::{Token, TokenKind};

const INDENT: &str = "    ";

/// Whether a token is a binary operator that should be surrounded by spaces.
fn is_binary_op(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::Less
            | TokenKind::LessEqual
            | TokenKind::Greater
            | TokenKind::GreaterEqual
            | TokenKind::PlusEqual
            | TokenKind::MinusEqual
            | TokenKind::StarEqual
            | TokenKind::SlashEqual
            | TokenKind::PercentEqual
            | TokenKind::And
            | TokenKind::Or
            | TokenKind::Is
            | TokenKind::In
            | TokenKind::QuestionQuestion
    )
}

fn is_close_bracket(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace
    )
}

fn is_open_bracket(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace
    )
}

/// Whether this token can precede `(` / `[` without a space (call/index syntax).
fn is_callable(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::RightParen
            | TokenKind::RightBracket
            | TokenKind::SelfKw
            | TokenKind::Super
    )
}

/// No space should appear before these tokens.
fn no_space_before(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Dot
            | TokenKind::QuestionDot
            | TokenKind::DotDot
            | TokenKind::DotDotEqual
    ) || is_close_bracket(kind)
}

/// Whether +/- after this previous token should be treated as unary (no space).
fn implies_unary_next(kind: &TokenKind) -> bool {
    is_open_bracket(kind)
        || is_binary_op(kind)
        || matches!(
            kind,
            TokenKind::Comma | TokenKind::Equal | TokenKind::Return | TokenKind::Colon
        )
}

/// Whether a token starts a block declaration that should have a blank line before it.
fn is_block_declaration(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Fun | TokenKind::Class | TokenKind::Enum)
}

pub fn format_tokens(tokens: &[Token]) -> String {
    let mut output = String::new();
    let mut depth: usize = 0;
    let mut i = 0;
    let len = tokens.len();
    let mut at_line_start = true;
    // Insert blank line before the next top-level block declaration
    let mut prev_was_top_level = false;
    let mut prev_kind: Option<TokenKind> = None;
    let mut prev_was_unary = false;

    while i < len {
        let token = &tokens[i];

        match &token.kind {
            TokenKind::Newline => {
                if !at_line_start {
                    output.push('\n');
                    // Look ahead past Indent/Dedent to compute next depth
                    let mut peek = i + 1;
                    let mut next_depth = depth;
                    while peek < len {
                        match &tokens[peek].kind {
                            TokenKind::Indent => {
                                next_depth += 1;
                                peek += 1;
                            }
                            TokenKind::Dedent => {
                                next_depth = next_depth.saturating_sub(1);
                                peek += 1;
                            }
                            TokenKind::Newline => break,
                            _ => break,
                        }
                    }
                    depth = next_depth;
                    i = peek;
                    for _ in 0..depth {
                        output.push_str(INDENT);
                    }
                    at_line_start = true;
                    if depth == 0 {
                        prev_was_top_level = true;
                    }
                    continue;
                } else {
                    i += 1;
                    continue;
                }
            }
            TokenKind::Indent => {
                depth += 1;
                i += 1;
                continue;
            }
            TokenKind::Dedent => {
                depth = depth.saturating_sub(1);
                i += 1;
                continue;
            }
            TokenKind::Eof => break,
            _ => {}
        }

        // Blank line before top-level block declarations (fun, class, enum, etc.)
        if prev_was_top_level
            && depth == 0
            && is_block_declaration(&token.kind)
            && !output.is_empty()
        {
            if !at_line_start {
                output.push('\n');
                at_line_start = true;
            }
            if !output.ends_with("\n\n") {
                output.push('\n');
            }
        }
        prev_was_top_level = false;

        // Decide spacing before this token
        if !at_line_start {
            let space = if prev_was_unary {
                // No space after unary operator
                false
            } else if (token.kind == TokenKind::Minus || token.kind == TokenKind::Plus)
                && prev_kind.as_ref().is_none_or(implies_unary_next)
            {
                // Unary +/- : no space before
                false
            } else if is_binary_op(&token.kind) {
                !output.ends_with(' ')
            } else if no_space_before(&token.kind) {
                false
            } else if is_open_bracket(&token.kind) {
                match &prev_kind {
                    Some(pk) if is_callable(pk) => false,
                    _ => !output.ends_with(' ') && !output.ends_with('\n'),
                }
            } else {
                !output.ends_with(' ')
                    && !output.ends_with('\n')
                    && !output.ends_with('(')
                    && !output.ends_with('[')
                    && !output.ends_with('{')
                    && !output.ends_with('.')
            };
            if space {
                output.push(' ');
            }
        }

        at_line_start = false;

        // Emit token
        output.push_str(&token.lexeme);

        // Spacing after token
        if is_binary_op(&token.kind) {
            // Don't add space after if next token makes this unary
            // (The space-before logic on the next token handles this)
            // But for true binary ops, add a trailing space
            // Peek at next meaningful token to decide
            let mut next_i = i + 1;
            while next_i < len
                && matches!(
                    tokens[next_i].kind,
                    TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
                )
            {
                next_i += 1;
            }
            let is_unary = if token.kind == TokenKind::Minus || token.kind == TokenKind::Plus {
                prev_kind.as_ref().is_none_or(implies_unary_next)
            } else {
                false
            };
            if !is_unary {
                output.push(' ');
            }
        } else if token.kind == TokenKind::Comma {
            output.push(' ');
        }

        // Track unary state for next iteration
        prev_was_unary = (token.kind == TokenKind::Minus || token.kind == TokenKind::Plus)
            && prev_kind.as_ref().is_none_or(implies_unary_next);

        prev_kind = Some(token.kind.clone());
        i += 1;
    }

    // Ensure trailing newline
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }

    output
}
