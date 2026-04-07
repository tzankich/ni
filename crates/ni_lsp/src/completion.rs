use tower_lsp::lsp_types::*;

use ni_parser::ast::*;

/// Return static completion items (keywords + builtins).
pub fn keyword_completions() -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let keywords = [
        // Control flow
        ("if", "if ${1:condition}:\n    $0", "Check a condition"),
        (
            "elif",
            "elif ${1:condition}:\n    $0",
            "Check another condition",
        ),
        ("else", "else:\n    $0", "The fallback branch"),
        (
            "while",
            "while ${1:condition}:\n    $0",
            "Repeat while true",
        ),
        (
            "for",
            "for ${1:item} in ${2:collection}:\n    $0",
            "Loop through items",
        ),
        ("break", "break", "Exit a loop early"),
        ("continue", "continue", "Skip to next iteration"),
        (
            "match",
            "match ${1:value}:\n    when ${2:pattern}:\n        $0",
            "Compare against patterns",
        ),
        ("pass", "pass", "Do nothing (placeholder)"),
        ("return", "return $0", "Send a value back from a function"),
        // Declarations
        ("var", "var ${1:name} = $0", "Create a variable"),
        ("const", "const ${1:NAME} = $0", "Create a constant"),
        (
            "fun",
            "fun ${1:name}(${2:params}):\n    $0",
            "Define a function",
        ),
        (
            "class",
            "class ${1:Name}:\n    fun init(${2:params}):\n        $0",
            "Define a class",
        ),
        ("enum", "enum ${1:Name}:\n    $0", "Define named constants"),
        ("import", "import $0", "Load code from another file"),
        // Coroutines
        ("spawn", "spawn $0", "Run in the background"),
        ("wait", "wait $0", "Pause this script"),
        // Error handling
        (
            "try",
            "try:\n    $0\ncatch ${1:e}:\n    pass",
            "Handle errors safely",
        ),
        ("fail", "fail $0", "Trigger an error"),
        ("assert", "assert $0", "Check that something is true"),
        // Operators/values
        ("and", "and ", "Both must be true"),
        ("or", "or ", "Either can be true"),
        ("not", "not ", "Flip true/false"),
        ("true", "true", "Boolean true"),
        ("false", "false", "Boolean false"),
        ("none", "none", "No value"),
        ("self", "self", "The current object"),
        ("super", "super", "The parent class"),
    ];

    for (label, snippet, detail) in keywords {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(detail.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    let builtins = [
        ("print", "print($0)", "Show text on screen"),
        ("len", "len($0)", "Count items in a collection"),
        ("type_of", "type_of($0)", "Get the type of a value"),
        ("to_string", "to_string($0)", "Convert to text"),
        ("to_int", "to_int($0)", "Convert to integer"),
        ("to_float", "to_float($0)", "Convert to decimal"),
        ("abs", "abs($0)", "Absolute value"),
        ("min", "min($1, $2)", "Smaller of two values"),
        ("max", "max($1, $2)", "Larger of two values"),
        ("range", "range($0)", "Create a number sequence"),
        ("input", "input($0)", "Ask the user for text"),
    ];

    let modules = [
        ("math", "math", "Math standard library (sqrt, pow, PI, ...)"),
        (
            "random",
            "random",
            "Random number generation (int, float, bool, ...)",
        ),
    ];

    for (label, snippet, detail) in builtins {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(detail.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    for (label, snippet, detail) in modules {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(detail.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    items
}

/// Extract identifiers from the AST for contextual completion.
pub fn identifiers_from_program(program: &Program) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for decl in &program.declarations {
        collect_decl_identifiers(&decl.kind, &mut items, &mut seen);
    }

    items
}

fn collect_decl_identifiers(
    kind: &DeclKind,
    items: &mut Vec<CompletionItem>,
    seen: &mut std::collections::HashSet<String>,
) {
    match kind {
        DeclKind::Var(v) => add_ident(items, seen, &v.name, CompletionItemKind::VARIABLE),
        DeclKind::Const(c) => add_ident(items, seen, &c.name, CompletionItemKind::CONSTANT),
        DeclKind::Fun(f) => {
            add_ident(items, seen, &f.name, CompletionItemKind::FUNCTION);
            for stmt in &f.body {
                collect_stmt_identifiers(&stmt.kind, items, seen);
            }
        }
        DeclKind::Class(c) => {
            add_ident(items, seen, &c.name, CompletionItemKind::CLASS);
            for method in &c.methods {
                add_ident(items, seen, &method.fun.name, CompletionItemKind::METHOD);
            }
        }
        DeclKind::Enum(e) => {
            add_ident(items, seen, &e.name, CompletionItemKind::ENUM);
            for variant in &e.variants {
                add_ident(items, seen, &variant.name, CompletionItemKind::ENUM_MEMBER);
            }
        }
        DeclKind::Statement(s) => collect_stmt_identifiers(&s.kind, items, seen),
        DeclKind::Import(_) | DeclKind::Spec(_) => {}
    }
}

fn collect_stmt_identifiers(
    kind: &StmtKind,
    items: &mut Vec<CompletionItem>,
    seen: &mut std::collections::HashSet<String>,
) {
    match kind {
        StmtKind::VarDecl(v) => add_ident(items, seen, &v.name, CompletionItemKind::VARIABLE),
        StmtKind::ConstDecl(c) => add_ident(items, seen, &c.name, CompletionItemKind::CONSTANT),
        StmtKind::For(f) => {
            add_ident(items, seen, &f.variable, CompletionItemKind::VARIABLE);
            for stmt in &f.body {
                collect_stmt_identifiers(&stmt.kind, items, seen);
            }
        }
        StmtKind::If(if_stmt) => {
            for s in &if_stmt.then_body {
                collect_stmt_identifiers(&s.kind, items, seen);
            }
            for (_, body) in &if_stmt.elif_branches {
                for s in body {
                    collect_stmt_identifiers(&s.kind, items, seen);
                }
            }
            if let Some(ref else_body) = if_stmt.else_body {
                for s in else_body {
                    collect_stmt_identifiers(&s.kind, items, seen);
                }
            }
        }
        StmtKind::While(w) => {
            for s in &w.body {
                collect_stmt_identifiers(&s.kind, items, seen);
            }
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_stmt_identifiers(&s.kind, items, seen);
            }
        }
        _ => {}
    }
}

fn add_ident(
    items: &mut Vec<CompletionItem>,
    seen: &mut std::collections::HashSet<String>,
    name: &str,
    kind: CompletionItemKind,
) {
    if seen.insert(name.to_string()) {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(kind),
            ..Default::default()
        });
    }
}
