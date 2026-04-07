use super::parse_source;
use crate::codegen_c;

#[test]
fn test_c_int_literal() {
    let program = parse_source("var x = 42");
    let output = codegen_c(&program);
    assert!(output.contains("ni_int(42)"), "output:\n{}", output);
}

#[test]
fn test_c_float_literal() {
    let program = parse_source("var x = 3.14");
    let output = codegen_c(&program);
    assert!(output.contains("ni_float(3.14)"), "output:\n{}", output);
}

#[test]
fn test_c_string_literal() {
    let program = parse_source("var x = \"hello\"");
    let output = codegen_c(&program);
    assert!(
        output.contains("ni_string(\"hello\")"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_c_bool_literal() {
    let program = parse_source("var x = true");
    let output = codegen_c(&program);
    assert!(output.contains("ni_bool(1)"), "output:\n{}", output);
}

#[test]
fn test_c_none_literal() {
    let program = parse_source("var x = none");
    let output = codegen_c(&program);
    assert!(output.contains("NI_NONE"), "output:\n{}", output);
}

#[test]
fn test_c_binary_add() {
    let program = parse_source("var x = 1 + 2");
    let output = codegen_c(&program);
    assert!(output.contains("ni_add("), "output:\n{}", output);
}

#[test]
fn test_c_comparison() {
    let program = parse_source("var x = 1 == 2");
    let output = codegen_c(&program);
    assert!(output.contains("ni_eq("), "output:\n{}", output);
}

#[test]
fn test_c_less_than() {
    let program = parse_source("var x = 1 < 2");
    let output = codegen_c(&program);
    assert!(output.contains("ni_less_than("), "output:\n{}", output);
}

#[test]
fn test_c_if_statement() {
    let program = parse_source("if true:\n    var x = 1\n");
    let output = codegen_c(&program);
    assert!(output.contains("if (ni_is_truthy("), "output:\n{}", output);
}

#[test]
fn test_c_while_loop() {
    let program = parse_source("while true:\n    pass\n");
    let output = codegen_c(&program);
    assert!(
        output.contains("while (ni_is_truthy("),
        "output:\n{}",
        output
    );
}

#[test]
fn test_c_for_loop() {
    let program = parse_source("for x in [1, 2, 3]:\n    pass\n");
    let output = codegen_c(&program);
    assert!(output.contains("ni_get_iterator("), "output:\n{}", output);
    assert!(output.contains("ni_iterator_next("), "output:\n{}", output);
}

#[test]
fn test_c_function_decl() {
    let program = parse_source("fun add(a, b):\n    return a + b\n");
    let output = codegen_c(&program);
    assert!(
        output.contains("NiValue ni_fun_add("),
        "output:\n{}",
        output
    );
    assert!(output.contains("NiVm* vm"), "output:\n{}", output);
    assert!(output.contains("ni_add("), "output:\n{}", output);
}

#[test]
fn test_c_class_decl() {
    let program = parse_source(
        "class Dog:\n    var name = \"Rex\"\n    fun bark():\n        print(\"Woof\")\n",
    );
    let output = codegen_c(&program);
    assert!(
        output.contains("ni_class_Dog_method_bark"),
        "output:\n{}",
        output
    );
    assert!(output.contains("VTable"), "output:\n{}", output);
}

#[test]
fn test_c_list_literal() {
    let program = parse_source("var x = [1, 2, 3]");
    let output = codegen_c(&program);
    assert!(output.contains("ni_list("), "output:\n{}", output);
}

#[test]
fn test_c_map_literal() {
    let program = parse_source("var x = [\"a\": 1]");
    let output = codegen_c(&program);
    assert!(output.contains("ni_map("), "output:\n{}", output);
}

#[test]
fn test_c_field_access() {
    let program = parse_source("var x = obj.field");
    let output = codegen_c(&program);
    assert!(output.contains("ni_get_prop("), "output:\n{}", output);
}

#[test]
fn test_c_method_call() {
    let program = parse_source("var x = obj.method(1)");
    let output = codegen_c(&program);
    assert!(output.contains("ni_method_call("), "output:\n{}", output);
}

#[test]
fn test_c_print() {
    let program = parse_source("print(\"hello\")");
    let output = codegen_c(&program);
    assert!(output.contains("ni_print(vm,"), "output:\n{}", output);
}

#[test]
fn test_c_ni_main() {
    let program = parse_source("var x = 1");
    let output = codegen_c(&program);
    assert!(
        output.contains("NiValue ni_main(NiVm* vm)"),
        "output:\n{}",
        output
    );
    assert!(output.contains("return NI_NONE;"), "output:\n{}", output);
}

#[test]
fn test_c_includes() {
    let program = parse_source("var x = 1");
    let output = codegen_c(&program);
    assert!(
        output.contains("#include \"ni_runtime.h\""),
        "output:\n{}",
        output
    );
    assert!(output.contains("#include <stdio.h>"), "output:\n{}", output);
}

#[test]
fn test_c_safe_nav() {
    let program = parse_source("var x = obj?.field");
    let output = codegen_c(&program);
    assert!(output.contains("ni_is_none"), "output:\n{}", output);
    assert!(output.contains("NI_NONE"), "output:\n{}", output);
}

#[test]
fn test_c_none_coalesce() {
    let program = parse_source("var x = a ?? b");
    let output = codegen_c(&program);
    assert!(output.contains("ni_is_none"), "output:\n{}", output);
}

#[test]
fn test_c_return() {
    let program = parse_source("fun foo():\n    return 42\n");
    let output = codegen_c(&program);
    assert!(output.contains("return ni_int(42)"), "output:\n{}", output);
}

#[test]
fn test_c_break_continue() {
    let program = parse_source("while true:\n    break\n");
    let output = codegen_c(&program);
    assert!(output.contains("break;"), "output:\n{}", output);
}

#[test]
fn test_c_match() {
    let program = parse_source(
        "match x:\n    when 1:\n        print(\"one\")\n    when _:\n        print(\"other\")\n",
    );
    let output = codegen_c(&program);
    assert!(output.contains("ni_eq("), "output:\n{}", output);
}

#[test]
fn test_c_try_catch() {
    let program = parse_source("try:\n    fail \"oops\"\ncatch e:\n    print(e)\n");
    let output = codegen_c(&program);
    assert!(
        output.contains("ni_push_error_handler"),
        "output:\n{}",
        output
    );
    assert!(
        output.contains("ni_pop_error_handler"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_c_enum_decl() {
    let program = parse_source("enum Color:\n    red\n    green\n    blue\n");
    let output = codegen_c(&program);
    assert!(output.contains("ni_enum_Color_red"), "output:\n{}", output);
    assert!(
        output.contains("ni_enum_Color_green"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_c_forward_declarations() {
    let program = parse_source("fun foo():\n    pass\n");
    let output = codegen_c(&program);
    assert!(
        output.contains("Forward declarations"),
        "output:\n{}",
        output
    );
    assert!(
        output.contains("NiValue ni_fun_foo("),
        "output:\n{}",
        output
    );
}

#[test]
fn test_c_range() {
    let program = parse_source("var x = 1..10");
    let output = codegen_c(&program);
    assert!(output.contains("ni_make_range("), "output:\n{}", output);
}

#[test]
fn test_c_lambda() {
    let program = parse_source("const f = fun(x): x + 1");
    let output = codegen_c(&program);
    assert!(output.contains("ni_lambda_"), "output:\n{}", output);
    assert!(output.contains("ni_make_function("), "output:\n{}", output);
}

#[test]
fn test_c_class_inheritance_super_call() {
    let source = "class Animal:\n    fun init(name):\n        self.name = name\n    fun speak():\n        return self.name\n\nclass Dog extends Animal:\n    fun init(name):\n        super.init(name)\n";
    let program = parse_source(source);
    let output = codegen_c(&program);
    // super.init(name) should call parent method directly
    assert!(
        output.contains("ni_class_Animal_method_init(vm, self_val,"),
        "super call should invoke parent method, output:\n{}",
        output
    );
}

#[test]
fn test_c_class_inheritance_super_no_args() {
    let source = "class Base:\n    fun reset():\n        self.x = 0\n\nclass Child extends Base:\n    fun reset():\n        super.reset()\n        self.y = 0\n";
    let program = parse_source(source);
    let output = codegen_c(&program);
    assert!(
        output.contains("ni_class_Base_method_reset(vm, self_val, NULL, 0)"),
        "super call with no args, output:\n{}",
        output
    );
}
