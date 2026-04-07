use crate::token::{Token, TokenKind};
use ni_error::{NiError, NiResult};

pub struct IndentProcessor;

impl IndentProcessor {
    pub fn process(raw_tokens: Vec<Token>) -> NiResult<Vec<Token>> {
        let mut output = Vec::new();
        let mut indent_stack: Vec<usize> = vec![0];
        let mut i = 0;
        let mut at_line_start = true;

        while i < raw_tokens.len() {
            let token = &raw_tokens[i];

            match &token.kind {
                TokenKind::Newline => {
                    // Emit newline, mark that we're at line start
                    if !output.is_empty() {
                        // Avoid duplicate newlines
                        if let Some(last) = output.last() {
                            let last: &Token = last;
                            if !matches!(last.kind, TokenKind::Newline | TokenKind::Indent) {
                                output.push(token.clone());
                            }
                        }
                    }
                    at_line_start = true;
                    i += 1;
                }

                TokenKind::IntLiteral(spaces)
                    if at_line_start && token.lexeme.chars().all(|c| c == ' ') =>
                {
                    // This is an indent measurement token from the cursor
                    let spaces = *spaces as usize;
                    let current_indent = *indent_stack.last().unwrap();

                    if spaces > current_indent {
                        // Check 4-space alignment
                        if (spaces - current_indent) != 4 {
                            // Allow non-4-space for now but could warn
                        }
                        indent_stack.push(spaces);
                        output.push(Token {
                            kind: TokenKind::Indent,
                            lexeme: String::new(),
                            span: token.span,
                        });
                    } else if spaces < current_indent {
                        while *indent_stack.last().unwrap() > spaces {
                            indent_stack.pop();
                            output.push(Token {
                                kind: TokenKind::Dedent,
                                lexeme: String::new(),
                                span: token.span,
                            });
                        }
                        if *indent_stack.last().unwrap() != spaces {
                            return Err(NiError::syntax("Inconsistent indentation", token.span));
                        }
                    }
                    // If spaces == current_indent, no indent/dedent needed

                    at_line_start = false;
                    i += 1;
                }

                _ => {
                    if at_line_start {
                        // No leading whitespace -- check for dedents back to 0
                        let current_indent = *indent_stack.last().unwrap();
                        if current_indent > 0 && !matches!(token.kind, TokenKind::Eof) {
                            while *indent_stack.last().unwrap() > 0 {
                                indent_stack.pop();
                                output.push(Token {
                                    kind: TokenKind::Dedent,
                                    lexeme: String::new(),
                                    span: token.span,
                                });
                            }
                        }
                        at_line_start = false;
                    }
                    output.push(token.clone());
                    i += 1;
                }
            }
        }

        // Emit remaining dedents
        let eof_span = output.last().map(|t| t.span).unwrap_or_default();
        while indent_stack.len() > 1 {
            indent_stack.pop();
            // Insert before EOF
            let eof_idx = output.len().saturating_sub(1);
            output.insert(
                eof_idx,
                Token {
                    kind: TokenKind::Dedent,
                    lexeme: String::new(),
                    span: eof_span,
                },
            );
        }

        // Ensure we end with a newline before EOF for clean parsing
        if output.len() >= 2 {
            let before_eof = output.len() - 1;
            if !matches!(
                output[before_eof.saturating_sub(1)].kind,
                TokenKind::Newline | TokenKind::Dedent
            ) && matches!(output[before_eof].kind, TokenKind::Eof)
            {
                output.insert(
                    before_eof,
                    Token {
                        kind: TokenKind::Newline,
                        lexeme: String::new(),
                        span: eof_span,
                    },
                );
            }
        }

        Ok(output)
    }
}
