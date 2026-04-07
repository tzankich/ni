use crate::format;

#[test]
fn test_simple_statement() {
    let result = format("var x = 5").unwrap();
    assert_eq!(result, "var x = 5\n");
}

#[test]
fn test_operator_spacing() {
    let result = format("var x = 1+2").unwrap();
    assert_eq!(result, "var x = 1 + 2\n");
}

#[test]
fn test_indentation_normalized() {
    let source = "if x > 5:\n    print(x)";
    let result = format(source).unwrap();
    assert_eq!(result, "if x > 5:\n    print(x)\n");
}

#[test]
fn test_function_formatting() {
    let source = "fun add(a, b):\n    return a + b";
    let result = format(source).unwrap();
    assert_eq!(result, "fun add(a, b):\n    return a + b\n");
}

#[test]
fn test_idempotency() {
    let source = "var x = 10\nvar y = 20\nprint(x + y)\n";
    let first = format(source).unwrap();
    let second = format(&first).unwrap();
    assert_eq!(first, second, "Formatting should be idempotent");
}

#[test]
fn test_syntax_error_returns_err() {
    // Unterminated string should fail to lex
    let result = format("var x = \"unterminated");
    assert!(result.is_err());
}

#[test]
fn test_comma_spacing() {
    let source = "var items = [1, 2, 3]";
    let result = format(source).unwrap();
    assert_eq!(result, "var items = [1, 2, 3]\n");
}

#[test]
fn test_class_formatting() {
    let source = "class Point:\n    fun init(x, y):\n        self.x = x\n        self.y = y";
    let result = format(source).unwrap();
    assert!(result.contains("class Point:"));
    assert!(result.contains("    fun init(x, y):"));
    assert!(result.contains("        self.x = x"));
}

// ---- New AST formatter tests ----

#[test]
fn test_blank_line_before_function() {
    let source = "var x = 1\nfun foo():\n    pass";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = 1\n\nfun foo():\n    pass\n");
}

#[test]
fn test_blank_line_before_class() {
    let source = "var x = 1\nclass Dog:\n    var name = \"Rex\"";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = 1\n\nclass Dog:\n    var name = \"Rex\"\n");
}

#[test]
fn test_blank_line_between_functions() {
    let source = "fun a():\n    pass\nfun b():\n    pass";
    let result = format(source).unwrap();
    assert_eq!(result, "fun a():\n    pass\n\nfun b():\n    pass\n");
}

#[test]
fn test_class_fields_and_methods() {
    let source = "class Dog:\n    var name = \"Rex\"\n    fun bark():\n        print(\"woof\")";
    let result = format(source).unwrap();
    assert_eq!(
        result,
        "class Dog:\n    var name = \"Rex\"\n\n    fun bark():\n        print(\"woof\")\n"
    );
}

#[test]
fn test_class_inheritance() {
    let source = "class Puppy extends Dog:\n    fun play():\n        pass";
    let result = format(source).unwrap();
    assert!(result.contains("class Puppy extends Dog:"));
}

#[test]
fn test_enum_formatting() {
    let source = "enum Color:\n    red = 0\n    blue = 1";
    let result = format(source).unwrap();
    assert_eq!(result, "enum Color:\n    red = 0\n    blue = 1\n");
}

#[test]
fn test_import_formatting() {
    let source = "import math";
    let result = format(source).unwrap();
    assert_eq!(result, "import math\n");
}

#[test]
fn test_from_import() {
    let source = "from math import sqrt, floor";
    let result = format(source).unwrap();
    assert_eq!(result, "from math import sqrt, floor\n");
}

#[test]
fn test_const_declaration() {
    let source = "const PI = 3.14";
    let result = format(source).unwrap();
    assert_eq!(result, "const PI = 3.14\n");
}

#[test]
fn test_if_elif_else() {
    let source = "if x > 10:\n    print(\"big\")\nelif x > 5:\n    print(\"medium\")\nelse:\n    print(\"small\")";
    let result = format(source).unwrap();
    assert!(result.contains("if x > 10:"));
    assert!(result.contains("elif x > 5:"));
    assert!(result.contains("else:"));
}

#[test]
fn test_while_loop() {
    let source = "while x > 0:\n    x = x - 1";
    let result = format(source).unwrap();
    assert_eq!(result, "while x > 0:\n    x = x - 1\n");
}

#[test]
fn test_for_loop() {
    let source = "for i in [1, 2, 3]:\n    print(i)";
    let result = format(source).unwrap();
    assert_eq!(result, "for i in [1, 2, 3]:\n    print(i)\n");
}

#[test]
fn test_for_loop_two_vars() {
    let source = "for k, v in items:\n    print(k)";
    let result = format(source).unwrap();
    assert_eq!(result, "for k, v in items:\n    print(k)\n");
}

#[test]
fn test_return_statement() {
    let source = "fun foo():\n    return 42";
    let result = format(source).unwrap();
    assert!(result.contains("    return 42"));
}

#[test]
fn test_method_call() {
    let source = "list.append(1)";
    let result = format(source).unwrap();
    assert_eq!(result, "list.append(1)\n");
}

#[test]
fn test_range_expression() {
    let source = "var r = 1..10";
    let result = format(source).unwrap();
    assert_eq!(result, "var r = 1..10\n");
}

#[test]
fn test_range_inclusive() {
    let source = "var r = 1..=10";
    let result = format(source).unwrap();
    assert_eq!(result, "var r = 1..=10\n");
}

#[test]
fn test_map_literal() {
    let source = "var m = [\"a\": 1, \"b\": 2]";
    let result = format(source).unwrap();
    assert_eq!(result, "var m = [\"a\": 1, \"b\": 2]\n");
}

#[test]
fn test_empty_map() {
    let source = "var m = [:]";
    let result = format(source).unwrap();
    assert_eq!(result, "var m = [:]\n");
}

#[test]
fn test_none_coalesce() {
    let source = "var x = a ?? b";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = a ?? b\n");
}

#[test]
fn test_safe_nav() {
    let source = "var x = obj?.name";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = obj?.name\n");
}

#[test]
fn test_try_catch() {
    let source = "try:\n    risky()\ncatch e:\n    print(e)";
    let result = format(source).unwrap();
    assert!(result.contains("try:"));
    assert!(result.contains("catch e:"));
    assert!(result.contains("    risky()"));
    assert!(result.contains("    print(e)"));
}

#[test]
fn test_assert() {
    let source = "assert x > 0, \"must be positive\"";
    let result = format(source).unwrap();
    assert_eq!(result, "assert x > 0, \"must be positive\"\n");
}

#[test]
fn test_fail() {
    let source = "fail \"something went wrong\"";
    let result = format(source).unwrap();
    assert_eq!(result, "fail \"something went wrong\"\n");
}

#[test]
fn test_spawn_yield() {
    let source = "spawn foo\nyield 42";
    let result = format(source).unwrap();
    assert_eq!(result, "spawn foo\nyield 42\n");
}

#[test]
fn test_bare_yield() {
    let source = "yield";
    let result = format(source).unwrap();
    assert_eq!(result, "yield\n");
}

#[test]
fn test_wait() {
    let source = "wait 1.5";
    let result = format(source).unwrap();
    assert_eq!(result, "wait 1.5\n");
}

#[test]
fn test_lambda() {
    let source = "var f = fun(x): x + 1";
    let result = format(source).unwrap();
    assert_eq!(result, "var f = fun(x): x + 1\n");
}

#[test]
fn test_ternary_expression() {
    let source = "var x = a if cond else b";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = a if cond else b\n");
}

#[test]
fn test_string_interpolation() {
    let source = "var s = `hello {name}`";
    let result = format(source).unwrap();
    assert_eq!(result, "var s = `hello {name}`\n");
}

#[test]
fn test_boolean_logic() {
    let source = "var x = a and b or c";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = a and b or c\n");
}

#[test]
fn test_not_expression() {
    let source = "var x = not true";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = not true\n");
}

#[test]
fn test_comment_preservation_leading() {
    let source = "// This is a comment\nvar x = 5";
    let result = format(source).unwrap();
    assert!(result.contains("// This is a comment"));
    assert!(result.contains("var x = 5"));
}

#[test]
fn test_comment_preservation_trailing() {
    let source = "var x = 5  // inline comment";
    let result = format(source).unwrap();
    assert!(result.contains("var x = 5"));
    assert!(result.contains("// inline comment"));
}

#[test]
fn test_match_formatting() {
    let source = "match x:\n    when 1:\n        print(\"one\")\n    when _:\n        print(\"other\")";
    let result = format(source).unwrap();
    assert!(result.contains("match x:"));
    assert!(result.contains("    when 1:"));
    assert!(result.contains("        print(\"one\")"));
    assert!(result.contains("    when _:"));
}

#[test]
fn test_idempotency_complex() {
    let source = r#"import math

const PI = 3.14
var x = 10

fun greet(name):
    print(`hello {name}`)

class Dog:
    var name = "Rex"

    fun bark():
        print("woof")
"#;
    let first = format(source).unwrap();
    let second = format(&first).unwrap();
    assert_eq!(
        first, second,
        "Complex formatting should be idempotent:\nFirst:\n{}\nSecond:\n{}",
        first, second
    );
}

#[test]
fn test_empty_function_body() {
    let source = "fun nothing():\n    pass";
    let result = format(source).unwrap();
    assert_eq!(result, "fun nothing():\n    pass\n");
}

#[test]
fn test_nested_if() {
    let source = "if a:\n    if b:\n        print(\"nested\")";
    let result = format(source).unwrap();
    assert_eq!(result, "if a:\n    if b:\n        print(\"nested\")\n");
}

#[test]
fn test_super_call() {
    let source = "class Child extends Parent:\n    fun init():\n        super.init()";
    let result = format(source).unwrap();
    assert!(result.contains("super.init()"));
}

#[test]
fn test_index_expression() {
    let source = "var x = items[0]";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = items[0]\n");
}

#[test]
fn test_field_access() {
    let source = "var x = self.name";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = self.name\n");
}

#[test]
fn test_negate() {
    let source = "var x = -5";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = -5\n");
}

#[test]
fn test_break_continue() {
    let source = "while true:\n    break";
    let result = format(source).unwrap();
    assert!(result.contains("    break"));
}

#[test]
fn test_compound_assign() {
    let source = "x += 1";
    let result = format(source).unwrap();
    assert_eq!(result, "x += 1\n");
}

#[test]
fn test_self_expression() {
    let source = "class Foo:\n    fun get():\n        return self";
    let result = format(source).unwrap();
    assert!(result.contains("return self"));
}

#[test]
fn test_none_literal() {
    let source = "var x = none";
    let result = format(source).unwrap();
    assert_eq!(result, "var x = none\n");
}

#[test]
fn test_match_has_when_keyword() {
    let source = "match x:\n    when 1:\n        print(\"one\")\n    when _:\n        print(\"other\")";
    let result = format(source).unwrap();
    assert!(result.contains("when 1:"), "Match case should have 'when' keyword");
    assert!(result.contains("when _:"), "Default case should have 'when' keyword");
}

#[test]
fn test_match_multi_pattern_comma() {
    let source = "match x:\n    when 1, 2, 3:\n        print(\"small\")";
    let result = format(source).unwrap();
    assert!(result.contains("1, 2, 3"), "Multi-pattern should use comma separator, not pipe");
    assert!(!result.contains("|"), "Should not contain pipe separator");
}

#[test]
fn test_match_formatting_corrected() {
    let source = "match x:\n    when 1:\n        print(\"one\")\n    when _:\n        print(\"other\")";
    let result = format(source).unwrap();
    // Verify it round-trips through format correctly
    let result2 = format(&result).unwrap();
    assert_eq!(result, result2, "Formatted match should be idempotent");
}

#[test]
fn test_type_annotation_with_type_args() {
    let source = "fun f(x: List[int]):\n    pass";
    let result = format(source).unwrap();
    assert!(result.contains("List[int]"), "Type arguments should be preserved: got {}", result);
}
