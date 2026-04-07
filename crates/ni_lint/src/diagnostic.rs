use ni_error::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub span: Span,
    pub message: String,
    pub severity: Severity,
    pub code: &'static str,
    pub suggestion: Option<String>,
}

impl LintDiagnostic {
    pub fn warning(code: &'static str, message: String, span: Span) -> Self {
        Self {
            span,
            message,
            severity: Severity::Warning,
            code,
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    pub fn format_with_source(&self, source: &str) -> String {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        let mut result = format!("{level}[{}]: {}", self.code, self.message);
        result.push_str(&format!(
            "\n  --> line {}, column {}",
            self.span.line, self.span.column
        ));
        if let Some(line_str) = source.lines().nth(self.span.line.saturating_sub(1)) {
            result.push_str(&format!("\n   | {line_str}"));
            let caret_col = self.span.column.saturating_sub(1);
            let underline_len = (self.span.end - self.span.start).max(1);
            result.push_str(&format!(
                "\n   | {}{}",
                " ".repeat(caret_col),
                "^".repeat(underline_len.min(line_str.len().saturating_sub(caret_col)))
            ));
        }
        if let Some(ref sug) = self.suggestion {
            result.push_str(&format!("\n   = help: {sug}"));
        }
        result
    }
}
