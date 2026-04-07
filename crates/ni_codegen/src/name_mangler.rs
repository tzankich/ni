/// Mangle a Ni function name for generated code.
pub fn mangle_fun(name: &str) -> String {
    format!("ni_fun_{}", sanitize(name))
}

/// Mangle a Ni class name.
pub fn mangle_class(name: &str) -> String {
    format!("ni_class_{}", sanitize(name))
}

/// Mangle a method name within a class.
pub fn mangle_method(class_name: &str, method_name: &str) -> String {
    format!(
        "ni_class_{}_method_{}",
        sanitize(class_name),
        sanitize(method_name)
    )
}

/// Mangle a static method name within a class.
pub fn mangle_static_method(class_name: &str, method_name: &str) -> String {
    format!(
        "ni_class_{}_static_{}",
        sanitize(class_name),
        sanitize(method_name)
    )
}

/// Mangle an on_handler in a game construct.
pub fn mangle_on_handler(construct_name: &str, event_name: &str) -> String {
    format!(
        "ni_class_{}_on_{}",
        sanitize(construct_name),
        sanitize(event_name)
    )
}

/// Mangle a state handler in a game construct.
pub fn mangle_state(construct_name: &str, state_name: &str) -> String {
    format!(
        "ni_class_{}_state_{}",
        sanitize(construct_name),
        sanitize(state_name)
    )
}

/// Name of the main entry point function.
pub fn mangle_main() -> String {
    "ni_main".to_string()
}

/// Mangle an enum name.
pub fn mangle_enum(name: &str) -> String {
    format!("ni_enum_{}", sanitize(name))
}

/// Mangle a lambda (anonymous function) with a unique counter.
pub fn mangle_lambda(counter: usize) -> String {
    format!("ni_lambda_{}", counter)
}

/// Sanitize an identifier for use in generated code.
/// Replaces non-alphanumeric characters with underscores.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Mangle a local variable name (Ni allows names that might clash with target keywords).
pub fn mangle_local(name: &str) -> String {
    // Prefix with ni_ to avoid keyword clashes in target language
    let sanitized = sanitize(name);
    if is_rust_keyword(&sanitized) || is_c_keyword(&sanitized) {
        format!("ni_{}", sanitized)
    } else {
        sanitized
    }
}

fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "yield"
    )
}

fn is_c_keyword(name: &str) -> bool {
    matches!(
        name,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "int"
            | "long"
            | "register"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
    )
}
