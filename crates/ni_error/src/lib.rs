use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,      // byte offset
    pub end: usize,        // byte offset
    pub line: usize,       // 1-indexed start line
    pub end_line: usize,   // 1-indexed end line
    pub column: usize,     // 1-indexed start column (char-based)
    pub end_column: usize, // 1-indexed end column (char-based)
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, end_line: usize, column: usize, end_column: usize) -> Self {
        Self {
            start,
            end,
            line,
            end_line,
            column,
            end_column,
        }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            end_line: self.end_line.max(other.end_line),
            column: if self.line <= other.line {
                self.column
            } else {
                other.column
            },
            end_column: if self.end >= other.end {
                self.end_column
            } else {
                other.end_column
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct NiError {
    pub message: String,
    pub span: Option<Span>,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Parse,
    Compile,
    Runtime,
    Type,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Syntax => write!(f, "SyntaxError"),
            ErrorKind::Parse => write!(f, "ParseError"),
            ErrorKind::Compile => write!(f, "CompileError"),
            ErrorKind::Runtime => write!(f, "RuntimeError"),
            ErrorKind::Type => write!(f, "TypeError"),
        }
    }
}

impl NiError {
    pub fn syntax(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            kind: ErrorKind::Syntax,
        }
    }

    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            kind: ErrorKind::Parse,
        }
    }

    pub fn compile(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            kind: ErrorKind::Compile,
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            kind: ErrorKind::Runtime,
        }
    }

    pub fn format_with_source(&self, source: &str) -> String {
        let mut result = format!("{}: {}", self.kind, self.message);
        if let Some(span) = self.span {
            result.push_str(&format!(
                "\n  --> line {}, column {}",
                span.line, span.column
            ));
            if let Some(line_str) = source.lines().nth(span.line.saturating_sub(1)) {
                result.push_str(&format!("\n   | {}", line_str));
                let caret_col = span.column.saturating_sub(1);
                let line_char_len = line_str.chars().count();
                let underline_len = if span.end_column > span.column {
                    span.end_column - span.column
                } else {
                    // Fallback: count chars in the span, not bytes.
                    source
                        .get(span.start..span.end)
                        .map(|s| s.chars().count())
                        .unwrap_or(1)
                        .max(1)
                };
                result.push_str(&format!(
                    "\n   | {}{}",
                    " ".repeat(caret_col),
                    "^".repeat(underline_len.min(line_char_len.saturating_sub(caret_col)))
                ));
            }
        }
        result
    }
}

impl fmt::Display for NiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)?;
        if let Some(span) = self.span {
            write!(f, " (line {}, col {})", span.line, span.column)?;
        }
        Ok(())
    }
}

impl std::error::Error for NiError {}

pub type NiResult<T> = Result<T, NiError>;
