mod cursor;
mod indent;
mod token;

pub use token::{Token, TokenKind};

use cursor::Cursor;
use indent::IndentProcessor;
use ni_error::{NiResult, Span};

/// A comment extracted from source code.
#[derive(Debug, Clone)]
pub struct Comment {
    pub text: String,
    pub line: usize,       // 1-indexed
    pub column: usize,     // 1-indexed
    pub is_trailing: bool, // true if code precedes the comment on the same line
}

/// Result of lexing with comment extraction.
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
}

/// Extract comments from source code by scanning for `//` outside strings.
fn extract_comments(source: &str) -> Vec<Comment> {
    let mut comments = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // Track position for line/column reporting
    let mut line_num: usize = 1;
    let mut col: usize = 1;    // 1-indexed column at position i
    let mut has_code = false;  // non-whitespace seen on current line before current position

    // String state: None = not in string, Some(ch) = in single-quoted string with that quote char
    let mut in_single: Option<char> = None;
    // Triple-quote state: None = not in triple string, Some(ch) = in triple with that quote char
    let mut in_triple: Option<char> = None;

    while i < len {
        let ch = chars[i];

        if let Some(tq) = in_triple {
            // Inside a triple-quoted string — track newlines, skip until closing triple
            if ch == '\n' {
                line_num += 1;
                col = 1;
                has_code = false;
                i += 1;
                continue;
            }
            if ch == tq && i + 2 < len && chars[i + 1] == tq && chars[i + 2] == tq {
                in_triple = None;
                col += 3;
                i += 3;
                has_code = true;
                continue;
            }
            col += 1;
            i += 1;
            continue;
        }

        if let Some(sq) = in_single {
            // Inside a single-quoted string — look for closing quote or escape
            if ch == '\n' {
                // Unterminated single-line string; reset string state and advance line
                in_single = None;
                line_num += 1;
                col = 1;
                has_code = false;
                i += 1;
                continue;
            }
            if ch == '\\' && i + 1 < len {
                // Skip escaped character
                let next = chars[i + 1];
                col += 2;
                i += 2;
                if next == '\n' {
                    line_num += 1;
                    col = 1;
                    has_code = false;
                }
                continue;
            }
            if ch == sq {
                in_single = None;
            }
            col += 1;
            i += 1;
            continue;
        }

        // Not inside any string
        match ch {
            '\n' => {
                line_num += 1;
                col = 1;
                has_code = false;
                i += 1;
                continue;
            }
            '"' | '\'' | '`' => {
                // Check for triple quotes
                if i + 2 < len && chars[i + 1] == ch && chars[i + 2] == ch {
                    in_triple = Some(ch);
                    col += 3;
                    i += 3;
                    has_code = true;
                    continue;
                }
                in_single = Some(ch);
                has_code = true;
                col += 1;
                i += 1;
                continue;
            }
            '/' if i + 1 < len && chars[i + 1] == '/' => {
                // Comment — collect to end of line
                let comment_col = col;
                i += 2;
                let start = i;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                comments.push(Comment {
                    text,
                    line: line_num,
                    column: comment_col,
                    is_trailing: has_code,
                });
                // Don't advance past the newline here — let the loop handle it
                continue;
            }
            c if !c.is_whitespace() => {
                has_code = true;
                col += 1;
                i += 1;
                continue;
            }
            _ => {
                col += 1;
                i += 1;
                continue;
            }
        }
    }
    comments
}

/// Lex source code and also extract comments.
pub fn lex_with_comments(source: &str) -> NiResult<LexResult> {
    let comments = extract_comments(source);
    let tokens = lex(source)?;
    Ok(LexResult { tokens, comments })
}

/// Returns true if a token kind is a trailing operator that triggers
/// implicit line continuation (the next newline is suppressed).
fn is_continuation_op(kind: &TokenKind) -> bool {
    matches!(
        kind,
        // Binary arithmetic (Star excluded -- ambiguous with `import *`)
        TokenKind::Plus | TokenKind::Minus |
        TokenKind::Slash | TokenKind::Percent |
        // Comparison
        TokenKind::EqualEqual | TokenKind::BangEqual |
        TokenKind::Less | TokenKind::LessEqual |
        TokenKind::Greater | TokenKind::GreaterEqual |
        // Logical
        TokenKind::And | TokenKind::Or |
        // Assignment
        TokenKind::Equal |
        TokenKind::PlusEqual | TokenKind::MinusEqual |
        TokenKind::StarEqual | TokenKind::SlashEqual | TokenKind::PercentEqual |
        // Punctuation that expects more
        TokenKind::Comma | TokenKind::Dot |
        // Range & misc
        TokenKind::Arrow | TokenKind::DotDot | TokenKind::DotDotEqual |
        TokenKind::QuestionDot | TokenKind::QuestionQuestion |
        // Keywords used as binary operators
        TokenKind::In | TokenKind::Is
    )
}

pub fn lex(source: &str) -> NiResult<Vec<Token>> {
    let mut cursor = Cursor::new(source)?;
    let mut raw_tokens = Vec::new();

    while !cursor.is_at_end() {
        cursor.skip_whitespace_same_line();
        if cursor.is_at_end() {
            break;
        }
        let token = cursor.next_token()?;
        // Only update continuation for real tokens, not skip tokens.
        // This lets the flag persist through the skip-Newline so that
        // skip_whitespace_same_line also consumes the leading indent.
        let is_skip = token.kind == TokenKind::Newline && token.lexeme.is_empty();
        if !is_skip {
            cursor.continuation = is_continuation_op(&token.kind);
        }
        raw_tokens.push(token);
    }
    let col = cursor.column();
    raw_tokens.push(Token {
        kind: TokenKind::Eof,
        lexeme: String::new(),
        span: Span::new(source.len(), source.len(), cursor.line(), cursor.line(), col, col),
    });

    // Filter out skip tokens (empty-lexeme Newlines produced inside brackets,
    // blank lines, and trailing-operator continuations) before indent processing.
    raw_tokens.retain(|t| !(t.kind == TokenKind::Newline && t.lexeme.is_empty()));

    let tokens = IndentProcessor::process(raw_tokens)?;
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract just the token kinds for easy assertion.
    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source).unwrap().into_iter().map(|t| t.kind).collect()
    }

    // -- Basic indent/dedent --

    #[test]
    fn single_indent_and_dedent() {
        let src = "if true:\n    x\ny\n";
        let k = kinds(src);
        assert!(k.contains(&TokenKind::Indent));
        assert!(k.contains(&TokenKind::Dedent));
        let indent_pos = k.iter().position(|t| *t == TokenKind::Indent).unwrap();
        let dedent_pos = k.iter().position(|t| *t == TokenKind::Dedent).unwrap();
        assert!(indent_pos < dedent_pos);
    }

    #[test]
    fn nested_indent_double_dedent() {
        let src = "if true:\n    if false:\n        x\ny\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 2);
        assert_eq!(dedents, 2);
    }

    #[test]
    fn eof_while_indented_emits_dedent() {
        let src = "if true:\n    x\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(
            indents, dedents,
            "trailing dedent must be emitted before EOF"
        );
        assert_eq!(*k.last().unwrap(), TokenKind::Eof);
    }

    #[test]
    fn no_indent_flat_code() {
        let src = "const x = 1\nconst y = 2\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent));
        assert!(!k.contains(&TokenKind::Dedent));
    }

    // -- Blank lines --

    #[test]
    fn blank_lines_inside_indented_block() {
        let src = "if true:\n    const x = 1\n\n    const y = 2\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    #[test]
    fn multiple_blank_lines_between_top_level() {
        let src = "const x = 1\n\n\n\nconst y = 2\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent));
        assert!(!k.contains(&TokenKind::Dedent));
    }

    // -- Bracket newline swallowing --

    #[test]
    fn newlines_inside_parens_swallowed() {
        let src = "const x = (\n    1 +\n    2\n)\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent), "no indent inside parens");
        assert!(!k.contains(&TokenKind::Dedent), "no dedent inside parens");
    }

    #[test]
    fn newlines_inside_brackets_swallowed() {
        let src = "const x = [\n    1,\n    2,\n    3\n]\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent), "no indent inside brackets");
        assert!(!k.contains(&TokenKind::Dedent), "no dedent inside brackets");
    }

    #[test]
    fn parens_inside_indented_block() {
        // This is the bug that was found during Phase B: multi-line parens
        // inside an indented block should not confuse the indent processor.
        let src = "if true:\n    const x = (\n        1,\n        2\n    )\n    const y = 3\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1, "only one indent for the if-block");
        assert_eq!(dedents, 1, "only one dedent for the if-block");
    }

    // -- Trailing operator continuation --

    #[test]
    fn trailing_plus_continues() {
        let src = "const x = 1 +\n    2\n";
        let k = kinds(src);
        assert!(
            !k.contains(&TokenKind::Indent),
            "continuation should not indent"
        );
        assert!(
            !k.contains(&TokenKind::Dedent),
            "continuation should not dedent"
        );
    }

    #[test]
    fn trailing_dot_continues() {
        // Method chaining across lines.
        let src = "x.\n    foo()\n";
        let k = kinds(src);
        assert!(
            !k.contains(&TokenKind::Indent),
            "dot continuation should not indent"
        );
    }

    #[test]
    fn trailing_comma_continues() {
        let src = "fun(a,\n    b)\n";
        let k = kinds(src);
        // Also inside parens, but comma alone should trigger continuation too.
        assert!(!k.contains(&TokenKind::Indent));
    }

    #[test]
    fn trailing_and_continues() {
        let src = "if a and\n    b:\n    c\n";
        let k = kinds(src);
        // Should get one indent (for the if-body), not two.
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        assert_eq!(indents, 1);
    }

    #[test]
    fn trailing_assign_continues() {
        let src = "const x =\n    5\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent));
    }

    #[test]
    fn non_operator_does_not_continue() {
        // A line ending with an identifier should NOT continue.
        let src = "const x = 1\nconst y = 2\n";
        let k = kinds(src);
        // Should have a newline between them, no continuation.
        assert!(k.contains(&TokenKind::Newline));
    }

    #[test]
    fn backslash_is_rejected() {
        let src = "const x = 1 + \\\n    2\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Unexpected character"), "{msg}");
    }

    // -- Inconsistent indentation --

    #[test]
    fn inconsistent_indentation_error() {
        let src = "if true:\n    x\n  y\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("indentation"),
            "error should mention indentation: {msg}"
        );
    }

    // -- Same-level continuation --

    #[test]
    fn same_indent_no_extra_tokens() {
        let src = "if true:\n    a\n    b\n    c\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    // -- Edge cases --

    #[test]
    fn single_line_no_newline() {
        let src = "const x = 1";
        let k = kinds(src);
        assert_eq!(*k.last().unwrap(), TokenKind::Eof);
        assert!(!k.contains(&TokenKind::Indent));
    }

    #[test]
    fn empty_input() {
        let k = kinds("");
        assert_eq!(k, vec![TokenKind::Eof]);
    }

    #[test]
    fn comment_only_lines_inside_block() {
        let src = "if true:\n    x\n        // deep comment\n    y\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    #[test]
    fn newline_before_eof_guaranteed() {
        let src = "const x = 1";
        let k = kinds(src);
        let eof_idx = k.len() - 1;
        assert_eq!(k[eof_idx], TokenKind::Eof);
        assert_eq!(k[eof_idx - 1], TokenKind::Newline);
    }

    #[test]
    fn deeply_nested_then_return_to_zero() {
        // Three indent levels (purely whitespace-driven), then back to column 0.
        let src = "if true:\n    if true:\n        if true:\n            x\nd\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 3);
        assert_eq!(dedents, 3);
    }

    #[test]
    fn partial_dedent() {
        let src = "if true:\n    if true:\n        x\n    y\nz\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 2);
        assert_eq!(dedents, 2);
    }

    // -- Bracket depth: braces and nesting --

    #[test]
    fn newlines_inside_braces_swallowed() {
        let src = "const x = {\n    1,\n    2\n}\n";
        let k = kinds(src);
        assert!(!k.contains(&TokenKind::Indent), "no indent inside braces");
        assert!(!k.contains(&TokenKind::Dedent), "no dedent inside braces");
    }

    #[test]
    fn nested_mixed_brackets() {
        // [()\n] -- mixed bracket types should all suppress newlines.
        let src = "const x = [(\n    1,\n    2\n)]\n";
        let k = kinds(src);
        assert!(
            !k.contains(&TokenKind::Indent),
            "no indent inside nested brackets"
        );
        assert!(
            !k.contains(&TokenKind::Dedent),
            "no dedent inside nested brackets"
        );
    }

    // -- Tab rejection --

    #[test]
    fn tabs_rejected() {
        let src = "if true:\n\tx\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Tabs"), "error should mention tabs: {msg}");
    }

    // -- Comment-only lines at various indents --

    #[test]
    fn comment_at_lower_indent_inside_block() {
        // A comment-only line at column 0 between indented lines.
        // skip_whitespace_same_line consumes the comment; the subsequent \n
        // is a regular Newline, but the next line's indent matches the stack.
        let src = "if true:\n    x\n// comment at col 0\n    y\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    #[test]
    fn comment_at_col0_before_dedent() {
        // Comment at column 0 followed by an actual dedent to column 0.
        let src = "if true:\n    x\n// comment\ny\n";
        let k = kinds(src);
        let indents = k.iter().filter(|t| **t == TokenKind::Indent).count();
        let dedents = k.iter().filter(|t| **t == TokenKind::Dedent).count();
        assert_eq!(indents, 1);
        assert_eq!(dedents, 1);
    }

    // -- Unicode confusable detection --

    #[test]
    fn em_dash_gives_helpful_error() {
        let src = "const x = 5 \u{2014} 3\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("em dash"), "should name the character: {msg}");
        assert!(msg.contains("'-'"), "should suggest replacement: {msg}");
    }

    #[test]
    fn en_dash_gives_helpful_error() {
        let src = "const x = 5 \u{2013} 3\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("en dash"), "{msg}");
        assert!(msg.contains("'-'"), "{msg}");
    }

    #[test]
    fn smart_double_quotes_give_helpful_error() {
        let src = "const x = \u{201C}hello\u{201D}\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("double quote"), "{msg}");
        assert!(msg.contains("'\"'"), "should suggest ASCII quote: {msg}");
    }

    #[test]
    fn smart_single_quotes_give_helpful_error() {
        let src = "const x = \u{2018}hello\u{2019}\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("single quote"), "{msg}");
    }

    #[test]
    fn zero_width_space_gives_helpful_error() {
        let src = "const\u{200B}x = 5\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("zero-width space"), "{msg}");
        assert!(msg.contains("remove"), "{msg}");
    }

    #[test]
    fn multiplication_sign_gives_helpful_error() {
        let src = "const x = 5 \u{00D7} 3\n";
        let result = lex(src);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("multiplication"), "{msg}");
        assert!(msg.contains("'*'"), "{msg}");
    }

    // -- Unicode identifiers --

    #[test]
    fn unicode_identifier_accepted() {
        let src = "const caf\u{00E9} = 42\n";
        let k = kinds(src);
        assert!(k.contains(&TokenKind::Const));
        // The identifier should be parsed successfully
        let tokens = lex(src).unwrap();
        let ident = tokens
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::Identifier(_)))
            .unwrap();
        assert_eq!(ident.lexeme, "caf\u{00E9}");
    }

    #[test]
    fn cjk_identifier_accepted() {
        let src = "const \u{4F60}\u{597D} = 1\n";
        let tokens = lex(src).unwrap();
        let ident = tokens
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::Identifier(_)))
            .unwrap();
        assert_eq!(ident.lexeme, "\u{4F60}\u{597D}");
    }

    #[test]
    fn emoji_rejected() {
        // Emoji are not alphabetic, should still be rejected
        let src = "const \u{1F600} = 1\n";
        let result = lex(src);
        assert!(result.is_err());
    }

    #[test]
    fn unicode_in_strings_works() {
        // Unicode inside strings should always be fine
        let src = "const x = \"hello \u{2014} world\"\n";
        let tokens = lex(src).unwrap();
        let str_tok = tokens
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::StringLiteral(_)))
            .unwrap();
        if let TokenKind::StringLiteral(s) = &str_tok.kind {
            assert!(s.contains('\u{2014}'));
        }
    }
}
