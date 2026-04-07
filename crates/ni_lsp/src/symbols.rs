use tower_lsp::lsp_types::*;

use ni_parser::ast::*;

/// Extract document symbols for the outline panel.
pub fn document_symbols(program: &Program) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for decl in &program.declarations {
        if let Some(sym) = decl_to_symbol(&decl.kind, decl.span) {
            symbols.push(sym);
        }
    }

    symbols
}

#[allow(deprecated)] // DocumentSymbol::deprecated is deprecated itself
fn decl_to_symbol(kind: &DeclKind, span: ni_error::Span) -> Option<DocumentSymbol> {
    let range = span_to_range(span);
    match kind {
        DeclKind::Var(v) => Some(DocumentSymbol {
            name: v.name.clone(),
            detail: Some("variable".to_string()),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        }),
        DeclKind::Const(c) => Some(DocumentSymbol {
            name: c.name.clone(),
            detail: Some("constant".to_string()),
            kind: SymbolKind::CONSTANT,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        }),
        DeclKind::Fun(f) => {
            let detail = format!(
                "({})",
                f.params
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Some(DocumentSymbol {
                name: f.name.clone(),
                detail: Some(detail),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            })
        }
        DeclKind::Class(c) => {
            let children: Vec<DocumentSymbol> = c
                .methods
                .iter()
                .map(|m| {
                    let detail = format!(
                        "({})",
                        m.fun
                            .params
                            .iter()
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    DocumentSymbol {
                        name: m.fun.name.clone(),
                        detail: Some(detail),
                        kind: SymbolKind::METHOD,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    }
                })
                .collect();
            Some(DocumentSymbol {
                name: c.name.clone(),
                detail: c.superclass.as_ref().map(|s| format!("extends {s}")),
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: if children.is_empty() {
                    None
                } else {
                    Some(children)
                },
            })
        }
        DeclKind::Enum(e) => {
            let children: Vec<DocumentSymbol> = e
                .variants
                .iter()
                .map(|v| DocumentSymbol {
                    name: v.name.clone(),
                    detail: None,
                    kind: SymbolKind::ENUM_MEMBER,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
                .collect();
            Some(DocumentSymbol {
                name: e.name.clone(),
                detail: Some("enum".to_string()),
                kind: SymbolKind::ENUM,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: if children.is_empty() {
                    None
                } else {
                    Some(children)
                },
            })
        }
        DeclKind::Statement(_) | DeclKind::Import(_) | DeclKind::Spec(_) => None,
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
