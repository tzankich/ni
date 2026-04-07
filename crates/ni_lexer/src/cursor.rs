use crate::token::{Token, TokenKind};
use ni_error::{NiError, NiResult, Span};

pub struct Cursor<'a> {
    source: &'a str,
    chars: Vec<char>,
    byte_offsets: Vec<usize>,
    pos: usize,
    line: usize,
    column: usize,
    bracket_depth: usize,
    pub continuation: bool,
}

impl<'a> Cursor<'a> {
    pub fn new(source: &'a str) -> NiResult<Self> {
        const MAX_SOURCE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
        if source.len() > MAX_SOURCE_SIZE {
            return Err(NiError::syntax(
                format!(
                    "Source file too large: {} bytes (max {} bytes)",
                    source.len(),
                    MAX_SOURCE_SIZE
                ),
                Span::new(0, 0, 1, 1, 0, 0),
            ));
        }

        let bytes = source.as_bytes();

        // Detect UTF-16 BOMs (raw bytes, before attempting to iterate chars)
        if bytes.len() >= 2
            && ((bytes[0] == 0xFF && bytes[1] == 0xFE) || (bytes[0] == 0xFE && bytes[1] == 0xFF))
        {
            return Err(NiError::syntax(
                "Source appears to be UTF-16 encoded. Ni requires UTF-8.",
                Span::new(0, 2, 1, 1, 1, 2),
            ));
        }

        // Handle UTF-8 BOM (U+FEFF = EF BB BF)
        let (source, skip) =
            if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
                (&source[3..], 0) // skip BOM bytes
            } else {
                (source, 0)
            };
        let _ = skip;

        let chars: Vec<char> = source.chars().collect();

        // Precompute byte offsets: byte_offsets[i] = byte offset of char at index i
        let mut byte_offsets = Vec::with_capacity(chars.len() + 1);
        let mut offset = 0;
        for ch in &chars {
            byte_offsets.push(offset);
            offset += ch.len_utf8();
        }
        byte_offsets.push(offset); // sentinel: byte_offsets[chars.len()] = source.len()

        Ok(Self {
            source,
            chars,
            byte_offsets,
            pos: 0,
            line: 1,
            column: 1,
            bracket_depth: 0,
            continuation: false,
        })
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> char {
        if self.pos < self.chars.len() {
            self.chars[self.pos]
        } else {
            '\0'
        }
    }

    fn peek_next(&self) -> char {
        if self.pos + 1 < self.chars.len() {
            self.chars[self.pos + 1]
        } else {
            '\0'
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.chars[self.pos];
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        ch
    }

    fn byte_pos(&self) -> usize {
        self.byte_offsets[self.pos]
    }

    /// Current character position (0-indexed)
    fn char_pos(&self) -> usize {
        self.pos
    }

    pub fn skip_whitespace_same_line(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                ' ' if self.column > 1 || self.bracket_depth > 0 || self.continuation => {
                    self.advance();
                }
                '\t' if self.bracket_depth > 0 => {
                    self.advance();
                }
                '/' if self.peek_next() == '/' => {
                    while !self.is_at_end() && self.peek() != '\n' {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    /// Helper to create a Span with proper end_column from char positions.
    fn make_span(
        &self,
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        start_col: usize,
        start_char: usize,
    ) -> Span {
        let char_len = self.pos - start_char;
        let end_col = start_col + char_len;
        Span::new(start_byte, end_byte, start_line, self.line, start_col, end_col)
    }

    pub fn next_token(&mut self) -> NiResult<Token> {
        let start_pos = self.byte_pos();
        let start_char = self.char_pos();
        let start_line = self.line;
        let start_col = self.column;
        let ch = self.advance();

        let kind = match ch {
            '\t' => {
                return Err(NiError::syntax(
                    "Tabs are not allowed; use 4 spaces for indentation",
                    Span::new(
                        start_pos,
                        start_pos + 1,
                        start_line,
                        start_line,
                        start_col,
                        start_col + 1,
                    ),
                ));
            }

            ' ' if start_col == 1 && self.bracket_depth == 0 => {
                // Line-leading whitespace: measure indent
                let mut spaces = 1;
                while !self.is_at_end() && self.peek() == ' ' {
                    self.advance();
                    spaces += 1;
                }
                // Skip blank lines and comment-only lines
                if !self.is_at_end()
                    && (self.peek() == '\n' || (self.peek() == '/' && self.peek_next() == '/'))
                {
                    if self.peek() == '/' {
                        while !self.is_at_end() && self.peek() != '\n' {
                            self.advance();
                        }
                    }
                    if !self.is_at_end() {
                        self.advance(); // consume \n
                    }
                    return self.make_skip_token(start_pos, start_line, start_col, start_char);
                }
                let end_pos = self.byte_pos();
                return Ok(Token {
                    kind: TokenKind::IntLiteral(spaces as i64), // Repurposed temporarily; IndentProcessor handles this
                    lexeme: " ".repeat(spaces),
                    span: self.make_span(start_pos, end_pos, start_line, start_col, start_char),
                });
            }

            '\n' => {
                // Skip consecutive newlines
                while !self.is_at_end() && self.peek() == '\n' {
                    self.advance();
                }
                if self.bracket_depth > 0 || self.continuation {
                    return self.make_skip_token(start_pos, start_line, start_col, start_char);
                }
                TokenKind::Newline
            }

            '\r' => {
                if !self.is_at_end() && self.peek() == '\n' {
                    self.advance();
                }
                if self.bracket_depth > 0 || self.continuation {
                    return self.make_skip_token(start_pos, start_line, start_col, start_char);
                }
                TokenKind::Newline
            }

            '(' => {
                self.bracket_depth += 1;
                TokenKind::LeftParen
            }
            ')' => {
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
                TokenKind::RightParen
            }
            '[' => {
                self.bracket_depth += 1;
                TokenKind::LeftBracket
            }
            ']' => {
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
                TokenKind::RightBracket
            }
            '{' => {
                self.bracket_depth += 1;
                TokenKind::LeftBrace
            }
            '}' => {
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
                TokenKind::RightBrace
            }

            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            ':' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::ColonEqual
                } else {
                    TokenKind::Colon
                }
            }
            '@' => TokenKind::At,

            '+' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::MinusEqual
                } else if !self.is_at_end() && self.peek() == '>' {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::PercentEqual
                } else {
                    TokenKind::Percent
                }
            }

            '=' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }

            '!' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                }
            }

            '<' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }

            '>' => {
                if !self.is_at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }

            '?' => {
                if !self.is_at_end() && self.peek() == '.' {
                    self.advance();
                    TokenKind::QuestionDot
                } else if !self.is_at_end() && self.peek() == '?' {
                    self.advance();
                    TokenKind::QuestionQuestion
                } else {
                    return Err(NiError::syntax(
                        "Unexpected character '?'",
                        Span::new(
                            start_pos,
                            start_pos + 1,
                            start_line,
                            start_line,
                            start_col,
                            start_col + 1,
                        ),
                    ));
                }
            }

            '.' => {
                if !self.is_at_end() && self.peek() == '.' {
                    self.advance();
                    if !self.is_at_end() && self.peek() == '=' {
                        self.advance();
                        TokenKind::DotDotEqual
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }

            '"' | '\'' => self.lex_string(ch, start_pos, start_line, start_col)?,

            '`' => self.lex_backtick_string(start_pos, start_line, start_col)?,

            '0'..='9' => self.lex_number(ch, start_pos, start_line, start_col)?,

            'a'..='z' | 'A'..='Z' | '_' => {
                self.lex_identifier(ch, start_pos, start_line, start_col)?
            }

            _ => {
                // Non-ASCII Unicode letter -- allow as identifier start
                if ch.is_alphabetic() {
                    self.lex_identifier(ch, start_pos, start_line, start_col)?
                } else if let Some((name, suggestion)) = Self::unicode_confusable(ch) {
                    // Common AI/copy-paste confusables -- give a helpful error
                    let msg = if suggestion.is_empty() {
                        format!("Unexpected invisible character (Unicode {name}); remove it")
                    } else {
                        format!("Unexpected '{ch}' (Unicode {name}); did you mean '{suggestion}'?")
                    };
                    return Err(NiError::syntax(
                        msg,
                        Span::new(
                            start_pos,
                            start_pos + ch.len_utf8(),
                            start_line,
                            start_line,
                            start_col,
                            start_col + 1,
                        ),
                    ));
                } else {
                    return Err(NiError::syntax(
                        format!("Unexpected character '{}'", ch),
                        Span::new(
                            start_pos,
                            start_pos + ch.len_utf8(),
                            start_line,
                            start_line,
                            start_col,
                            start_col + 1,
                        ),
                    ));
                }
            }
        };

        let end_pos = self.byte_pos();
        let lexeme = self.source[start_pos..end_pos].to_string();
        Ok(Token {
            kind,
            lexeme,
            span: self.make_span(start_pos, end_pos, start_line, start_col, start_char),
        })
    }

    fn make_skip_token(
        &self,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
        start_char: usize,
    ) -> NiResult<Token> {
        // Return a newline-like token that gets filtered
        Ok(Token {
            kind: TokenKind::Newline,
            lexeme: String::new(),
            span: self.make_span(
                start_pos,
                self.byte_pos(),
                start_line,
                start_col,
                start_char,
            ),
        })
    }

    /// Map common Unicode confusable characters to (name, ASCII replacement).
    /// These typically sneak in via AI-generated code or rich-text copy-paste.
    fn unicode_confusable(ch: char) -> Option<(&'static str, &'static str)> {
        match ch {
            // Dashes that look like minus
            '\u{2014}' => Some(("em dash", "-")),
            '\u{2013}' => Some(("en dash", "-")),
            '\u{2012}' => Some(("figure dash", "-")),
            '\u{2015}' => Some(("horizontal bar", "-")),
            '\u{2212}' => Some(("minus sign", "-")),
            // Smart quotes
            '\u{2018}' => Some(("left single quote", "'")),
            '\u{2019}' => Some(("right single quote", "'")),
            '\u{201C}' => Some(("left double quote", "\"")),
            '\u{201D}' => Some(("right double quote", "\"")),
            // Invisible whitespace
            '\u{00A0}' => Some(("non-breaking space", " ")),
            '\u{200B}' => Some(("zero-width space", "")),
            '\u{200C}' => Some(("zero-width non-joiner", "")),
            '\u{200D}' => Some(("zero-width joiner", "")),
            '\u{FEFF}' => Some(("byte order mark", "")),
            // Math operators
            '\u{00D7}' => Some(("multiplication sign", "*")),
            '\u{00F7}' => Some(("division sign", "/")),
            '\u{2260}' => Some(("not-equal sign", "!=")),
            '\u{2264}' => Some(("less-than-or-equal sign", "<=")),
            '\u{2265}' => Some(("greater-than-or-equal sign", ">=")),
            '\u{2026}' => Some(("ellipsis", "...")),
            // Misc
            '\u{2044}' => Some(("fraction slash", "/")),
            '\u{FF1A}' => Some(("fullwidth colon", ":")),
            '\u{FF08}' => Some(("fullwidth left paren", "(")),
            '\u{FF09}' => Some(("fullwidth right paren", ")")),
            _ => None,
        }
    }

    fn lex_string(
        &mut self,
        quote: char,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
    ) -> NiResult<TokenKind> {
        // Check for triple-quote
        if !self.is_at_end()
            && self.peek() == quote
            && self.pos + 1 < self.chars.len()
            && self.chars[self.pos + 1] == quote
        {
            self.advance();
            self.advance();
            return self.lex_triple_string(quote, start_pos, start_line, start_col);
        }

        let mut value = String::new();

        while !self.is_at_end() && self.peek() != quote {
            if self.peek() == '\n' {
                return Err(NiError::syntax(
                    "Unterminated string literal",
                    Span::new(
                        start_pos,
                        self.byte_pos(),
                        start_line,
                        self.line,
                        start_col,
                        self.column,
                    ),
                ));
            }
            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(NiError::syntax(
                        "Unterminated string literal",
                        Span::new(
                            start_pos,
                            self.byte_pos(),
                            start_line,
                            self.line,
                            start_col,
                            self.column,
                        ),
                    ));
                }
                let esc = self.advance();
                let escaped = match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '\'' => '\'',
                    '"' => '"',
                    '{' => '{',
                    '}' => '}',
                    '`' => '`',
                    '0' => '\0',
                    _ => {
                        return Err(NiError::syntax(
                            format!("Unknown escape sequence '\\{}'", esc),
                            Span::new(
                                start_pos,
                                self.byte_pos(),
                                start_line,
                                self.line,
                                start_col,
                                self.column,
                            ),
                        ));
                    }
                };
                value.push(escaped);
            } else {
                value.push(self.advance());
            }
        }

        if self.is_at_end() {
            return Err(NiError::syntax(
                "Unterminated string literal",
                Span::new(
                    start_pos,
                    self.byte_pos(),
                    start_line,
                    self.line,
                    start_col,
                    self.column,
                ),
            ));
        }
        self.advance(); // closing quote

        Ok(TokenKind::StringLiteral(value))
    }

    fn lex_triple_string(
        &mut self,
        quote: char,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
    ) -> NiResult<TokenKind> {
        let mut value = String::new();

        loop {
            if self.is_at_end() {
                return Err(NiError::syntax(
                    "Unterminated triple-quoted string",
                    Span::new(
                        start_pos,
                        self.byte_pos(),
                        start_line,
                        self.line,
                        start_col,
                        self.column,
                    ),
                ));
            }
            if self.peek() == quote
                && self.pos + 1 < self.chars.len()
                && self.chars[self.pos + 1] == quote
                && self.pos + 2 < self.chars.len()
                && self.chars[self.pos + 2] == quote
            {
                self.advance();
                self.advance();
                self.advance();
                break;
            }
            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(NiError::syntax(
                        "Unterminated triple-quoted string",
                        Span::new(
                            start_pos,
                            self.byte_pos(),
                            start_line,
                            self.line,
                            start_col,
                            self.column,
                        ),
                    ));
                }
                let esc = self.advance();
                match esc {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '\'' => value.push('\''),
                    '"' => value.push('"'),
                    '{' => value.push('{'),
                    '}' => value.push('}'),
                    '`' => value.push('`'),
                    '0' => value.push('\0'),
                    _ => {
                        value.push('\\');
                        value.push(esc);
                    }
                }
            } else {
                value.push(self.advance());
            }
        }

        // Trim leading/trailing newlines and common indentation
        let trimmed = Self::dedent_triple_string(&value);
        Ok(TokenKind::StringLiteral(trimmed))
    }

    fn dedent_triple_string(s: &str) -> String {
        let s = s.strip_prefix('\n').unwrap_or(s);
        let s = s.strip_suffix('\n').unwrap_or(s);
        let lines: Vec<&str> = s.lines().collect();
        if lines.is_empty() {
            return String::new();
        }
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);
        lines
            .iter()
            .map(|l| {
                let ws_chars = l.chars().take_while(|c| c.is_whitespace()).count();
                if ws_chars >= min_indent {
                    let byte_offset = l
                        .char_indices()
                        .nth(min_indent)
                        .map(|(i, _)| i)
                        .unwrap_or(l.len());
                    &l[byte_offset..]
                } else {
                    l.trim()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // === Backtick format strings (interpolation) ===

    fn lex_backtick_string(
        &mut self,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
    ) -> NiResult<TokenKind> {
        // Check for triple-backtick
        if !self.is_at_end()
            && self.peek() == '`'
            && self.pos + 1 < self.chars.len()
            && self.chars[self.pos + 1] == '`'
        {
            self.advance();
            self.advance();
            return self.lex_triple_backtick_string(start_pos, start_line, start_col);
        }

        let mut interpolation_parts: Vec<(String, bool)> = Vec::new(); // (text, is_expr)
        let mut has_interpolation = false;
        let mut current_text = String::new();

        while !self.is_at_end() && self.peek() != '`' {
            if self.peek() == '\n' {
                return Err(NiError::syntax(
                    "Unterminated backtick string",
                    Span::new(
                        start_pos,
                        self.byte_pos(),
                        start_line,
                        self.line,
                        start_col,
                        self.column,
                    ),
                ));
            }
            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(NiError::syntax(
                        "Unterminated backtick string",
                        Span::new(
                            start_pos,
                            self.byte_pos(),
                            start_line,
                            self.line,
                            start_col,
                            self.column,
                        ),
                    ));
                }
                let esc = self.advance();
                let escaped = match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '`' => '`',
                    '{' => '{',
                    '}' => '}',
                    '0' => '\0',
                    _ => {
                        return Err(NiError::syntax(
                            format!("Unknown escape sequence '\\{}'", esc),
                            Span::new(
                                start_pos,
                                self.byte_pos(),
                                start_line,
                                self.line,
                                start_col,
                                self.column,
                            ),
                        ));
                    }
                };
                current_text.push(escaped);
            } else if self.peek() == '{' {
                has_interpolation = true;
                self.advance(); // consume {
                if !current_text.is_empty() {
                    interpolation_parts.push((current_text.clone(), false));
                    current_text.clear();
                }
                let expr = self.scan_interpolation_expr(start_pos, start_line, start_col)?;
                interpolation_parts.push((expr, true));
            } else {
                current_text.push(self.advance());
            }
        }

        if self.is_at_end() {
            return Err(NiError::syntax(
                "Unterminated backtick string",
                Span::new(
                    start_pos,
                    self.byte_pos(),
                    start_line,
                    self.line,
                    start_col,
                    self.column,
                ),
            ));
        }
        self.advance(); // closing backtick

        if has_interpolation {
            if !current_text.is_empty() {
                interpolation_parts.push((current_text, false));
            }
            Ok(TokenKind::StringLiteral(format!(
                "__fstring__:{}",
                Self::encode_fstring_parts(&interpolation_parts)
            )))
        } else {
            Ok(TokenKind::StringLiteral(current_text))
        }
    }

    fn lex_triple_backtick_string(
        &mut self,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
    ) -> NiResult<TokenKind> {
        let mut interpolation_parts: Vec<(String, bool)> = Vec::new();
        let mut has_interpolation = false;
        let mut current_text = String::new();

        loop {
            if self.is_at_end() {
                return Err(NiError::syntax(
                    "Unterminated triple-backtick string",
                    Span::new(
                        start_pos,
                        self.byte_pos(),
                        start_line,
                        self.line,
                        start_col,
                        self.column,
                    ),
                ));
            }
            if self.peek() == '`'
                && self.pos + 1 < self.chars.len()
                && self.chars[self.pos + 1] == '`'
                && self.pos + 2 < self.chars.len()
                && self.chars[self.pos + 2] == '`'
            {
                self.advance();
                self.advance();
                self.advance();
                break;
            }
            if self.peek() == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err(NiError::syntax(
                        "Unterminated triple-backtick string",
                        Span::new(
                            start_pos,
                            self.byte_pos(),
                            start_line,
                            self.line,
                            start_col,
                            self.column,
                        ),
                    ));
                }
                let esc = self.advance();
                match esc {
                    'n' => current_text.push('\n'),
                    't' => current_text.push('\t'),
                    'r' => current_text.push('\r'),
                    '\\' => current_text.push('\\'),
                    '`' => current_text.push('`'),
                    '{' => current_text.push('{'),
                    '}' => current_text.push('}'),
                    '0' => current_text.push('\0'),
                    _ => {
                        current_text.push('\\');
                        current_text.push(esc);
                    }
                }
            } else if self.peek() == '{' {
                has_interpolation = true;
                self.advance(); // consume {
                if !current_text.is_empty() {
                    interpolation_parts.push((current_text.clone(), false));
                    current_text.clear();
                }
                let expr = self.scan_interpolation_expr(start_pos, start_line, start_col)?;
                interpolation_parts.push((expr, true));
            } else {
                current_text.push(self.advance());
            }
        }

        if has_interpolation {
            if !current_text.is_empty() {
                interpolation_parts.push((current_text, false));
            }
            // Strip leading newline from first literal part and trailing newline from last
            if let Some((ref mut first_text, false)) = interpolation_parts.first_mut() {
                if first_text.starts_with('\n') {
                    first_text.remove(0);
                }
            }
            if let Some((ref mut last_text, false)) = interpolation_parts.last_mut() {
                if last_text.ends_with('\n') {
                    last_text.pop();
                }
            }
            Ok(TokenKind::StringLiteral(format!(
                "__fstring__:{}",
                Self::encode_fstring_parts(&interpolation_parts)
            )))
        } else {
            let trimmed = Self::dedent_triple_string(&current_text);
            Ok(TokenKind::StringLiteral(trimmed))
        }
    }

    /// Read chars from after opening `{` to matching `}`, tracking brace depth.
    fn scan_interpolation_expr(
        &mut self,
        start_pos: usize,
        start_line: usize,
        start_col: usize,
    ) -> NiResult<String> {
        let mut expr = String::new();
        let mut brace_depth = 1;
        let mut in_string: Option<char> = None;
        while !self.is_at_end() && brace_depth > 0 {
            let c = self.advance();
            if let Some(quote) = in_string {
                // Inside a string literal — only look for the closing quote
                if c == '\\' && !self.is_at_end() {
                    expr.push(c);
                    expr.push(self.advance());
                    continue;
                }
                if c == quote {
                    in_string = None;
                }
                expr.push(c);
                continue;
            }
            match c {
                '"' | '\'' => {
                    in_string = Some(c);
                    expr.push(c);
                }
                '{' => {
                    brace_depth += 1;
                    expr.push(c);
                }
                '}' => {
                    brace_depth -= 1;
                    if brace_depth > 0 {
                        expr.push(c);
                    }
                }
                _ => expr.push(c),
            }
        }
        if brace_depth > 0 {
            return Err(NiError::syntax(
                "Unterminated interpolation expression",
                Span::new(
                    start_pos,
                    self.byte_pos(),
                    start_line,
                    self.line,
                    start_col,
                    self.column,
                ),
            ));
        }
        Ok(expr)
    }

    fn encode_fstring_parts(parts: &[(String, bool)]) -> String {
        parts
            .iter()
            .map(|(text, is_expr)| {
                if *is_expr {
                    format!("{{EXPR:{}}}", text)
                } else {
                    text.replace('{', "\\{").replace('}', "\\}")
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn lex_number(
        &mut self,
        first: char,
        _start_pos: usize,
        _start_line: usize,
        _start_col: usize,
    ) -> NiResult<TokenKind> {
        if first == '0' && !self.is_at_end() {
            match self.peek() {
                'x' | 'X' => {
                    self.advance();
                    return self.lex_hex();
                }
                'b' | 'B' => {
                    self.advance();
                    return self.lex_binary();
                }
                _ => {}
            }
        }

        let mut is_float = false;
        while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == '_') {
            self.advance();
        }

        if !self.is_at_end() && self.peek() == '.' && self.peek_next() != '.' {
            is_float = true;
            self.advance(); // consume .
            while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == '_') {
                self.advance();
            }
        }

        // Scientific notation
        if !self.is_at_end() && (self.peek() == 'e' || self.peek() == 'E') {
            is_float = true;
            self.advance();
            if !self.is_at_end() && (self.peek() == '+' || self.peek() == '-') {
                self.advance();
            }
            while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == '_') {
                self.advance();
            }
        }

        let end_pos = self.byte_pos();
        // _start_pos is already a byte position (from byte_pos()), use it directly
        let text: String = self.source[_start_pos..end_pos]
            .chars()
            .filter(|c| *c != '_')
            .collect();

        if is_float {
            let val: f64 = text.parse().map_err(|_| {
                NiError::syntax(
                    "Invalid float literal",
                    Span::new(_start_pos, end_pos, _start_line, self.line, _start_col, self.column),
                )
            })?;
            Ok(TokenKind::FloatLiteral(val))
        } else {
            let val: i64 = text.parse().map_err(|_| {
                NiError::syntax(
                    "Invalid integer literal",
                    Span::new(_start_pos, end_pos, _start_line, self.line, _start_col, self.column),
                )
            })?;
            Ok(TokenKind::IntLiteral(val))
        }
    }

    fn lex_hex(&mut self) -> NiResult<TokenKind> {
        let start = self.byte_pos();
        while !self.is_at_end() && (self.peek().is_ascii_hexdigit() || self.peek() == '_') {
            self.advance();
        }
        let text: String = self.source[start..self.byte_pos()]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        let val = i64::from_str_radix(&text, 16).map_err(|_| {
            NiError::syntax(
                "Invalid hex literal",
                Span::new(start, self.byte_pos(), self.line, self.line, self.column, self.column),
            )
        })?;
        Ok(TokenKind::IntLiteral(val))
    }

    fn lex_binary(&mut self) -> NiResult<TokenKind> {
        let start = self.byte_pos();
        while !self.is_at_end() && (self.peek() == '0' || self.peek() == '1' || self.peek() == '_')
        {
            self.advance();
        }
        let text: String = self.source[start..self.byte_pos()]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        let val = i64::from_str_radix(&text, 2).map_err(|_| {
            NiError::syntax(
                "Invalid binary literal",
                Span::new(start, self.byte_pos(), self.line, self.line, self.column, self.column),
            )
        })?;
        Ok(TokenKind::IntLiteral(val))
    }

    fn lex_identifier(
        &mut self,
        first: char,
        _start_pos: usize,
        _start_line: usize,
        _start_col: usize,
    ) -> NiResult<TokenKind> {
        let mut name = String::new();
        name.push(first);
        while !self.is_at_end() && (self.peek().is_alphanumeric() || self.peek() == '_') {
            name.push(self.advance());
        }

        let kind = match name.as_str() {
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "fun" => TokenKind::Fun,
            "class" => TokenKind::Class,
            "extends" => TokenKind::Extends,
            "enum" => TokenKind::Enum,
            "import" => TokenKind::Import,
            "from" => TokenKind::From,
            "as" => TokenKind::As,
            "return" => TokenKind::Return,
            "static" => TokenKind::Static,
            "super" => TokenKind::Super,
            "spec" => TokenKind::Spec,
            "given" => TokenKind::Given,
            "when" => TokenKind::When,
            "then" => TokenKind::Then,
            "each" => TokenKind::Each,
            "if" => TokenKind::If,
            "elif" => TokenKind::Elif,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "while" => TokenKind::While,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "match" => TokenKind::Match,
            "case" => TokenKind::Case, // reserved; match/catch arms now use `when`
            "trait" => TokenKind::Trait,
            "abstract" => TokenKind::Abstract,
            "private" => TokenKind::Private,
            "defer" => TokenKind::Defer,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "type" => TokenKind::Type,
            "pass" => TokenKind::Pass,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "fail" => TokenKind::Fail,
            "assert" => TokenKind::Assert,
            "yield" => TokenKind::Yield,
            "wait" => TokenKind::Wait,
            "spawn" => TokenKind::Spawn,
            "fiber" => TokenKind::Fiber,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "is" => TokenKind::Is,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "none" | "null" => TokenKind::None,
            "self" => TokenKind::SelfKw,
            _ => TokenKind::Identifier(name),
        };

        Ok(kind)
    }
}
