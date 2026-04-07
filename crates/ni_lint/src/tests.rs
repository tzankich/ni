use crate::lint;

fn lint_source(source: &str) -> Vec<crate::diagnostic::LintDiagnostic> {
    let tokens = ni_lexer::lex(source).expect("lex failed");
    let program = ni_parser::parse(tokens).expect("parse failed");
    lint(&program)
}

fn has_code(diagnostics: &[crate::diagnostic::LintDiagnostic], code: &str) -> bool {
    diagnostics.iter().any(|d| d.code == code)
}

// --- Naming checks ---

#[test]
fn test_var_snake_case_warning() {
    let diags = lint_source("var MyVar = 5");
    assert!(
        has_code(&diags, "NI001"),
        "Expected NI001 for non-snake_case var"
    );
}

#[test]
fn test_var_snake_case_ok() {
    let diags = lint_source("var my_var = 5");
    assert!(!has_code(&diags, "NI001"));
}

#[test]
fn test_fun_snake_case_warning() {
    let diags = lint_source("fun MyFunc():\n    pass");
    assert!(
        has_code(&diags, "NI002"),
        "Expected NI002 for non-snake_case fun"
    );
}

#[test]
fn test_fun_snake_case_ok() {
    let diags = lint_source("fun my_func():\n    pass");
    assert!(!has_code(&diags, "NI002"));
}

#[test]
fn test_const_snake_case_warning() {
    let diags = lint_source("const MyConst = 3");
    assert!(
        has_code(&diags, "NI003"),
        "Expected NI003 for non-snake_case const"
    );
}

#[test]
fn test_const_snake_case_ok() {
    let diags = lint_source("const pi = 3");
    assert!(!has_code(&diags, "NI003"));
}

#[test]
fn test_class_pascal_warning() {
    let diags = lint_source("class my_class:\n    fun init():\n        pass");
    assert!(
        has_code(&diags, "NI004"),
        "Expected NI004 for non-PascalCase class"
    );
}

#[test]
fn test_class_pascal_ok() {
    let diags = lint_source("class MyClass:\n    fun init():\n        pass");
    assert!(!has_code(&diags, "NI004"));
}

#[test]
fn test_enum_pascal_warning() {
    let diags = lint_source("enum my_enum:\n    a = 1");
    assert!(
        has_code(&diags, "NI005"),
        "Expected NI005 for non-PascalCase enum"
    );
}

#[test]
fn test_enum_pascal_ok() {
    let diags = lint_source("enum MyEnum:\n    a = 1");
    assert!(!has_code(&diags, "NI005"));
}

// --- Unused variables ---

#[test]
fn test_unused_var_warning() {
    let diags = lint_source("fun f():\n    var x = 5\n    return 0");
    assert!(has_code(&diags, "NI010"), "Expected NI010 for unused var x");
}

#[test]
fn test_used_var_no_warning() {
    let diags = lint_source("fun f(x):\n    return x");
    assert!(
        !has_code(&diags, "NI010"),
        "Should not warn for used param x"
    );
}

#[test]
fn test_underscore_prefix_no_warning() {
    let diags = lint_source("const _unused = 5");
    assert!(!has_code(&diags, "NI010"), "Should not warn for _unused");
}

#[test]
fn test_unused_in_for() {
    // i is used inside the loop body by the print call, so no unused warning expected
    let diags = lint_source("for i in 0..5:\n    print(i)");
    assert!(!has_code(&diags, "NI010"));
}

#[test]
fn test_comprehensive_no_false_positives() {
    // A well-written program should produce no unused warnings
    let source = "var x = 10\nvar y = 20\nprint(x + y)";
    let diags = lint_source(source);
    assert!(!has_code(&diags, "NI010"));
}
