use super::parse_source;
use crate::codegen_rust;

#[test]
fn test_int_literal() {
    let program = parse_source("var x = 42");
    let output = codegen_rust(&program);
    assert!(output.contains("NiValue::Int(42)"), "output:\n{}", output);
}

#[test]
fn test_float_literal() {
    let program = parse_source("var x = 3.14");
    let output = codegen_rust(&program);
    assert!(
        output.contains("NiValue::Float(3.14)"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_string_literal() {
    let program = parse_source("var x = \"hello\"");
    let output = codegen_rust(&program);
    assert!(output.contains("NiValue::String("), "output:\n{}", output);
    assert!(output.contains("\"hello\""), "output:\n{}", output);
}

#[test]
fn test_bool_literal() {
    let program = parse_source("var x = true");
    let output = codegen_rust(&program);
    assert!(
        output.contains("NiValue::Bool(true)"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_none_literal() {
    let program = parse_source("var x = none");
    let output = codegen_rust(&program);
    assert!(output.contains("NiValue::None"), "output:\n{}", output);
}

#[test]
fn test_binary_add() {
    let program = parse_source("var x = 1 + 2");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_add"), "output:\n{}", output);
}

#[test]
fn test_binary_sub() {
    let program = parse_source("var x = 5 - 3");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_sub"), "output:\n{}", output);
}

#[test]
fn test_binary_mul() {
    let program = parse_source("var x = 2 * 3");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_mul"), "output:\n{}", output);
}

#[test]
fn test_binary_div() {
    let program = parse_source("var x = 10 / 2");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_div"), "output:\n{}", output);
}

#[test]
fn test_comparison_eq() {
    let program = parse_source("var x = 1 == 2");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_eq"), "output:\n{}", output);
}

#[test]
fn test_comparison_lt() {
    let program = parse_source("var x = 1 < 2");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_lt"), "output:\n{}", output);
}

#[test]
fn test_and_short_circuit() {
    let program = parse_source("var x = true and false");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_is_truthy"), "output:\n{}", output);
}

#[test]
fn test_or_short_circuit() {
    let program = parse_source("var x = true or false");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_is_truthy"), "output:\n{}", output);
}

#[test]
fn test_not() {
    let program = parse_source("var x = not true");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_not"), "output:\n{}", output);
}

#[test]
fn test_negate() {
    let program = parse_source("var x = -42");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_negate"), "output:\n{}", output);
}

#[test]
fn test_var_decl_mut() {
    let program = parse_source("var x = 10");
    let output = codegen_rust(&program);
    assert!(output.contains("let mut x"), "output:\n{}", output);
}

#[test]
fn test_const_decl_immut() {
    let program = parse_source("const x = 10");
    let output = codegen_rust(&program);
    assert!(output.contains("let x"), "output:\n{}", output);
    assert!(
        !output.contains("let mut x"),
        "const should not be mut, output:\n{}",
        output
    );
}

#[test]
fn test_if_statement() {
    let program = parse_source("if true:\n    var x = 1\n");
    let output = codegen_rust(&program);
    assert!(output.contains("if ni_is_truthy"), "output:\n{}", output);
}

#[test]
fn test_while_loop() {
    let program = parse_source("while true:\n    pass\n");
    let output = codegen_rust(&program);
    assert!(output.contains("loop {"), "output:\n{}", output);
    assert!(output.contains("break"), "output:\n{}", output);
}

#[test]
fn test_for_loop() {
    let program = parse_source("for x in [1, 2, 3]:\n    pass\n");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_get_iterator"), "output:\n{}", output);
    assert!(output.contains("ni_iterator_next"), "output:\n{}", output);
}

#[test]
fn test_function_decl() {
    let program = parse_source("fun add(a, b):\n    return a + b\n");
    let output = codegen_rust(&program);
    assert!(output.contains("pub fn ni_fun_add"), "output:\n{}", output);
    assert!(output.contains("NiResult<NiValue>"), "output:\n{}", output);
    assert!(output.contains("ni_add"), "output:\n{}", output);
    assert!(output.contains("return Ok("), "output:\n{}", output);
}

#[test]
fn test_list_literal() {
    let program = parse_source("var x = [1, 2, 3]");
    let output = codegen_rust(&program);
    assert!(output.contains("NiValue::List"), "output:\n{}", output);
    assert!(output.contains("RefCell::new(vec!["), "output:\n{}", output);
}

#[test]
fn test_map_literal() {
    let program = parse_source("var x = [\"a\": 1, \"b\": 2]");
    let output = codegen_rust(&program);
    assert!(output.contains("NiValue::Map"), "output:\n{}", output);
}

#[test]
fn test_field_access() {
    let program = parse_source("var x = obj.field");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_get_prop"), "output:\n{}", output);
    assert!(output.contains("\"field\""), "output:\n{}", output);
}

#[test]
fn test_method_call() {
    let program = parse_source("var x = obj.method(1, 2)");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_method_call"), "output:\n{}", output);
    assert!(output.contains("\"method\""), "output:\n{}", output);
}

#[test]
fn test_print_call() {
    let program = parse_source("print(\"hello\")");
    let output = codegen_rust(&program);
    assert!(output.contains("vm.print("), "output:\n{}", output);
}

#[test]
fn test_index_access() {
    let program = parse_source("var x = list[0]");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_get_index"), "output:\n{}", output);
}

#[test]
fn test_safe_nav() {
    let program = parse_source("var x = obj?.field");
    let output = codegen_rust(&program);
    assert!(output.contains("is_none()"), "output:\n{}", output);
    assert!(output.contains("ni_get_prop"), "output:\n{}", output);
}

#[test]
fn test_none_coalesce() {
    let program = parse_source("var x = a ?? b");
    let output = codegen_rust(&program);
    assert!(output.contains("is_none()"), "output:\n{}", output);
}

#[test]
fn test_try_expr() {
    let program = parse_source("var x = try dangerous()");
    let output = codegen_rust(&program);
    assert!(
        output.contains("unwrap_or(NiValue::None)"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_fail_expr() {
    let program = parse_source("fail \"error\"");
    let output = codegen_rust(&program);
    assert!(
        output.contains("NiRuntimeError::from_value"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_return_statement() {
    let program = parse_source("fun foo():\n    return 42\n");
    let output = codegen_rust(&program);
    assert!(
        output.contains("return Ok(NiValue::Int(42))"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_break_continue() {
    let program = parse_source("while true:\n    break\n");
    let output = codegen_rust(&program);
    assert!(output.contains("break;"), "output:\n{}", output);
}

#[test]
fn test_class_decl() {
    let program = parse_source(
        "class Dog:\n    var name = \"Rex\"\n    fun bark():\n        print(\"Woof\")\n",
    );
    let output = codegen_rust(&program);
    assert!(
        output.contains("ni_class_Dog_method_bark"),
        "output:\n{}",
        output
    );
    assert!(
        output.contains("NiClassDef::new(\"Dog\")"),
        "output:\n{}",
        output
    );
    assert!(output.contains("NiValue::Class"), "output:\n{}", output);
}

#[test]
fn test_enum_decl() {
    let program = parse_source("enum Color:\n    red\n    green\n    blue\n");
    let output = codegen_rust(&program);
    assert!(output.contains("NiEnumDef"), "output:\n{}", output);
    assert!(output.contains("\"red\""), "output:\n{}", output);
}

#[test]
fn test_match_statement() {
    let program = parse_source("match x:\n    when 1:\n        print(\"one\")\n    when 2:\n        print(\"two\")\n    when _:\n        print(\"other\")\n");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_eq"), "output:\n{}", output);
}

#[test]
fn test_try_catch() {
    let program = parse_source("try:\n    fail \"oops\"\ncatch e:\n    print(e)\n");
    let output = codegen_rust(&program);
    assert!(
        output.contains("match (|| -> NiResult<NiValue>"),
        "output:\n{}",
        output
    );
    assert!(output.contains("Err(_err)"), "output:\n{}", output);
}

#[test]
fn test_assert() {
    let program = parse_source("assert 1 == 1");
    let output = codegen_rust(&program);
    assert!(output.contains("Assertion failed"), "output:\n{}", output);
}

#[test]
fn test_ni_main_wrapper() {
    let program = parse_source("var x = 1");
    let output = codegen_rust(&program);
    assert!(
        output.contains("pub fn ni_main(vm: &mut dyn NiVm) -> NiResult<NiValue>"),
        "output:\n{}",
        output
    );
    assert!(output.contains("Ok(NiValue::None)"), "output:\n{}", output);
}

#[test]
fn test_use_ni_runtime() {
    let program = parse_source("var x = 1");
    let output = codegen_rust(&program);
    assert!(
        output.contains("use ni_runtime::prelude::*"),
        "output:\n{}",
        output
    );
}

#[test]
fn test_range_literal() {
    let program = parse_source("var x = 1..10");
    let output = codegen_rust(&program);
    assert!(output.contains("NiRange"), "output:\n{}", output);
}

#[test]
fn test_lambda() {
    let program = parse_source("const f = fun(x): x + 1");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_lambda_"), "output:\n{}", output);
    assert!(output.contains("NiFunctionRef"), "output:\n{}", output);
}

#[test]
fn test_compound_assign() {
    let program = parse_source("var x = 1\nx += 2");
    let output = codegen_rust(&program);
    assert!(output.contains("ni_add"), "output:\n{}", output);
}

#[test]
fn test_default_params() {
    let program = parse_source("fun greet(name = \"world\"):\n    print(name)\n");
    let output = codegen_rust(&program);
    assert!(output.contains("unwrap_or("), "output:\n{}", output);
    assert!(output.contains("\"world\""), "output:\n{}", output);
}

#[test]
fn test_if_expression() {
    let program = parse_source("var x = 1 if true else 2");
    let output = codegen_rust(&program);
    assert!(output.contains("if ni_is_truthy"), "output:\n{}", output);
}

#[test]
fn test_fstring() {
    let program = parse_source("var name = \"world\"\nvar x = `hello {name}`");
    let output = codegen_rust(&program);
    assert!(output.contains("to_display_string"), "output:\n{}", output);
}

#[test]
fn test_class_inheritance_super_call() {
    let source = "class Animal:\n    fun init(name):\n        self.name = name\n    fun speak():\n        return self.name\n\nclass Dog extends Animal:\n    fun init(name):\n        super.init(name)\n";
    let program = parse_source(source);
    let output = codegen_rust(&program);
    // super.init(name) should call parent method directly
    assert!(
        output.contains("ni_class_Animal_method_init(vm, self_val,"),
        "super call should invoke parent method, output:\n{}",
        output
    );
    // Superclass should be linked in class registration
    assert!(
        output.contains("class.superclass = Some(Rc::new(ni_class_Animal_register()))"),
        "superclass should be linked, output:\n{}",
        output
    );
}

#[test]
fn test_class_inheritance_super_no_args() {
    let source = "class Base:\n    fun reset():\n        self.x = 0\n\nclass Child extends Base:\n    fun reset():\n        super.reset()\n        self.y = 0\n";
    let program = parse_source(source);
    let output = codegen_rust(&program);
    assert!(
        output.contains("ni_class_Base_method_reset(vm, self_val, &[])?"),
        "super call with no args, output:\n{}",
        output
    );
}

#[test]
fn test_class_inheritance_inherited_method() {
    let source = "class Animal:\n    fun speak():\n        return \"hello\"\n\nclass Dog extends Animal:\n    fun fetch():\n        return \"ball\"\n";
    let program = parse_source(source);
    let output = codegen_rust(&program);
    // Dog's class should have superclass linked so find_method() can walk the chain
    assert!(
        output.contains("ni_class_Animal_register()"),
        "parent register should be referenced, output:\n{}",
        output
    );
}
