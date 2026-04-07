mod formatter;
mod printer;

use ni_error::NiResult;

/// Format Ni source code using AST-based formatting.
/// Falls back to token-based formatting if parsing fails.
pub fn format(source: &str) -> NiResult<String> {
    // Try AST-based formatting first
    let lex_result = ni_lexer::lex_with_comments(source)?;
    match ni_parser::parse(lex_result.tokens) {
        Ok(program) => {
            let comments = printer::CommentMap::build(lex_result.comments);
            Ok(printer::Printer::format(&program, comments))
        }
        Err(_) => {
            // Fallback to token-based for unparseable files
            let tokens = ni_lexer::lex(source)?;
            Ok(formatter::format_tokens(&tokens))
        }
    }
}

/// Format using only the token-based formatter (for cases where AST is unavailable).
pub fn format_tokens(source: &str) -> NiResult<String> {
    let tokens = ni_lexer::lex(source)?;
    Ok(formatter::format_tokens(&tokens))
}

#[cfg(test)]
mod tests;
