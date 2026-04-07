use tower_lsp::lsp_types::*;

use ni_parser::Program;

/// Result of analyzing a source file.
pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
    pub program: Option<Program>,
}

/// Run the full analysis pipeline: lex -> parse -> lint.
/// Produces LSP diagnostics at each stage; stops on lex/parse errors
/// but still returns partial diagnostics.
pub fn analyze(source: &str) -> AnalysisResult {
    let mut diagnostics = Vec::new();

    // Stage 1: Lex
    let tokens = match ni_lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(e) => {
            diagnostics.push(ni_error_to_diagnostic(&e));
            return AnalysisResult {
                diagnostics,
                program: None,
            };
        }
    };

    // Stage 2: Parse
    let program = match ni_parser::parse(tokens) {
        Ok(program) => program,
        Err(e) => {
            diagnostics.push(ni_error_to_diagnostic(&e));
            return AnalysisResult {
                diagnostics,
                program: None,
            };
        }
    };

    // Stage 3: Lint
    let lint_results = ni_lint::lint(&program);
    for lint_diag in &lint_results {
        let severity = match lint_diag.severity {
            ni_lint::diagnostic::Severity::Warning => DiagnosticSeverity::WARNING,
            ni_lint::diagnostic::Severity::Error => DiagnosticSeverity::ERROR,
        };
        let mut message = lint_diag.message.clone();
        if let Some(ref sug) = lint_diag.suggestion {
            message.push_str(&format!("\nHint: {sug}"));
        }
        diagnostics.push(Diagnostic {
            range: span_to_range(lint_diag.span),
            severity: Some(severity),
            code: Some(NumberOrString::String(lint_diag.code.to_string())),
            source: Some("ni-lint".to_string()),
            message,
            ..Default::default()
        });
    }

    AnalysisResult {
        diagnostics,
        program: Some(program),
    }
}

fn ni_error_to_diagnostic(e: &ni_error::NiError) -> Diagnostic {
    let range = match e.span {
        Some(span) => span_to_range(span),
        None => Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        },
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("ni".to_string()),
        message: e.message.clone(),
        ..Default::default()
    }
}

fn span_to_range(span: ni_error::Span) -> Range {
    let line = span.line.saturating_sub(1) as u32;
    let col = span.column.saturating_sub(1) as u32;
    let end_line = span.end_line.saturating_sub(1) as u32;
    let end_col = if span.end_column > 0 {
        (span.end_column - 1) as u32
    } else {
        col + 1
    };
    Range {
        start: Position::new(line, col),
        end: Position::new(end_line, end_col),
    }
}
