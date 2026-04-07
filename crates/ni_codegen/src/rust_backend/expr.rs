use super::RustCodeGen;
use crate::name_mangler;
use ni_parser::*;

pub(crate) fn emit_expr(gen: &mut RustCodeGen, expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::IntLiteral(n) => format!("NiValue::Int({})", n),
        ExprKind::FloatLiteral(f) => {
            if f.fract() == 0.0 && !f.is_infinite() && !f.is_nan() {
                format!("NiValue::Float({:.1})", f)
            } else {
                // Use to_string for exact representation
                let s = format!("{}", f);
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    format!("NiValue::Float({})", s)
                } else {
                    format!("NiValue::Float({}.0)", s)
                }
            }
        }
        ExprKind::StringLiteral(s) => {
            let escaped = escape_string(s);
            format!("NiValue::String(Rc::new(\"{}\".to_string()))", escaped)
        }
        ExprKind::BoolLiteral(b) => format!("NiValue::Bool({})", b),
        ExprKind::NoneLiteral => "NiValue::None".to_string(),

        ExprKind::Identifier(name) => {
            // Check if it's "self" in a class context
            if name == "self" && gen.current_class.is_some() {
                return "self_val.clone()".to_string();
            }
            // Known functions: reference as NiValue::Function wrapper
            if gen.known_functions.contains(name.as_str()) {
                let mangled = name_mangler::mangle_fun(name);
                return format!("NiValue::Function(NiFunctionRef {{ name: \"{}\".into(), func: Rc::new(|vm, args| {}(vm, args)) }})", name, mangled);
            }
            // Known classes: reference as the class value
            if gen.known_classes.contains(name.as_str()) {
                let register_fn = name_mangler::mangle_class(name);
                return format!("NiValue::Class(Rc::new({}_register()))", register_fn);
            }
            // Known enums: reference as the enum value
            if gen.known_enums.contains(name.as_str()) {
                return name_mangler::mangle_local(name);
            }
            name_mangler::mangle_local(name)
        }

        ExprKind::SelfExpr => "self_val.clone()".to_string(),

        // Unary
        ExprKind::Negate(inner) => {
            let val = emit_expr(gen, inner);
            format!("ni_negate(&{})?", val)
        }
        ExprKind::Not(inner) => {
            let val = emit_expr(gen, inner);
            format!("ni_not(&{})", val)
        }

        // Binary arithmetic
        ExprKind::BinaryOp { left, op, right } => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let func = match op {
                BinOp::Add => "ni_add",
                BinOp::Sub => "ni_sub",
                BinOp::Mul => "ni_mul",
                BinOp::Div => "ni_div",
                BinOp::Mod => "ni_mod",
            };
            format!("{}(&{}, &{})?", func, l, r)
        }

        // Logical short-circuit
        ExprKind::And(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            format!(
                "{{ let {} = {}; if ni_is_truthy(&{}) {{ {} }} else {{ {} }} }}",
                tmp, l, tmp, r, tmp
            )
        }
        ExprKind::Or(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            format!(
                "{{ let {} = {}; if ni_is_truthy(&{}) {{ {} }} else {{ {} }} }}",
                tmp, l, tmp, tmp, r
            )
        }

        // Comparison
        ExprKind::Compare { left, op, right } => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            match op {
                CmpOp::Eq => format!("ni_eq(&{}, &{})", l, r),
                CmpOp::NotEq => format!("ni_neq(&{}, &{})", l, r),
                CmpOp::Lt => format!("ni_lt(&{}, &{})?", l, r),
                CmpOp::Gt => format!("ni_gt(&{}, &{})?", l, r),
                CmpOp::LtEq => format!("ni_lte(&{}, &{})?", l, r),
                CmpOp::GtEq => format!("ni_gte(&{}, &{})?", l, r),
                CmpOp::Is => {
                    // `is` checks type identity
                    if let ExprKind::Identifier(type_name) = &right.kind {
                        format!("ni_is(&{}, \"{}\")", l, type_name)
                    } else {
                        let r_str = emit_expr(gen, right);
                        format!("ni_eq(&{}, &{})", l, r_str)
                    }
                }
                CmpOp::In => format!("ni_in(&{}, &{})?", l, r),
            }
        }

        // Assignment
        ExprKind::Assign { target, value } => {
            let val = emit_expr(gen, value);
            match &target.kind {
                ExprKind::Identifier(name) => {
                    let mangled = name_mangler::mangle_local(name);
                    format!("{{ {} = {}; {}.clone() }}", mangled, val, mangled)
                }
                ExprKind::GetField(obj, field) => {
                    let obj_val = emit_expr(gen, obj);
                    format!(
                        "{{ let _sv = {}; ni_set_prop(&{}, \"{}\", _sv.clone())?; _sv }}",
                        val, obj_val, field
                    )
                }
                ExprKind::GetIndex(obj, idx) => {
                    let obj_val = emit_expr(gen, obj);
                    let idx_val = emit_expr(gen, idx);
                    format!(
                        "{{ let _sv = {}; ni_set_index(&{}, &{}, _sv.clone())?; _sv }}",
                        val, obj_val, idx_val
                    )
                }
                _ => format!("/* unsupported assign target */ {}", val),
            }
        }

        ExprKind::CompoundAssign { target, op, value } => {
            let val = emit_expr(gen, value);
            let func = match op {
                BinOp::Add => "ni_add",
                BinOp::Sub => "ni_sub",
                BinOp::Mul => "ni_mul",
                BinOp::Div => "ni_div",
                BinOp::Mod => "ni_mod",
            };
            match &target.kind {
                ExprKind::Identifier(name) => {
                    let mangled = name_mangler::mangle_local(name);
                    format!(
                        "{{ {} = {}(&{}, &{})?; {}.clone() }}",
                        mangled, func, mangled, val, mangled
                    )
                }
                _ => "/* unsupported compound assign target */ NiValue::None".to_string(),
            }
        }

        // Field access
        ExprKind::GetField(obj, field) => {
            let obj_val = emit_expr(gen, obj);
            format!("ni_get_prop(&{}, \"{}\")?", obj_val, field)
        }
        ExprKind::SetField(obj, field, value) => {
            let obj_val = emit_expr(gen, obj);
            let val = emit_expr(gen, value);
            format!(
                "{{ let _sv = {}; ni_set_prop(&{}, \"{}\", _sv.clone())?; _sv }}",
                val, obj_val, field
            )
        }

        // Index access
        ExprKind::GetIndex(obj, index) => {
            let obj_val = emit_expr(gen, obj);
            let idx_val = emit_expr(gen, index);
            format!("ni_get_index(&{}, &{})?", obj_val, idx_val)
        }
        ExprKind::SetIndex(obj, index, value) => {
            let obj_val = emit_expr(gen, obj);
            let idx_val = emit_expr(gen, index);
            let val = emit_expr(gen, value);
            format!(
                "{{ let _sv = {}; ni_set_index(&{}, &{}, _sv.clone())?; _sv }}",
                val, obj_val, idx_val
            )
        }

        // Safe navigation ?.
        ExprKind::SafeNav(obj, field) => {
            let obj_val = emit_expr(gen, obj);
            let tmp = gen.fresh_temp();
            format!("{{ let {} = {}; if {}.is_none() {{ NiValue::None }} else {{ ni_get_prop(&{}, \"{}\")? }} }}",
                    tmp, obj_val, tmp, tmp, field)
        }

        // None coalesce ??
        ExprKind::NoneCoalesce(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            format!(
                "{{ let {} = {}; if {}.is_none() {{ {} }} else {{ {} }} }}",
                tmp, l, tmp, r, tmp
            )
        }

        // Function call
        ExprKind::Call {
            callee,
            args,
            named_args: _,
        } => {
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            // For direct identifier calls, use the function directly
            match &callee.kind {
                ExprKind::Identifier(name) if name == "print" => {
                    if args_strs.is_empty() {
                        "{{ vm.print(\"\"); NiValue::None }}".to_string()
                    } else {
                        let print_parts: Vec<String> = args_strs
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let tmp = gen.fresh_temp();
                                if i > 0 {
                                    format!("let {} = {}; vm.print(\" \"); vm.print(&{}.to_display_string());", tmp, a, tmp)
                                } else {
                                    format!("let {} = {}; vm.print(&{}.to_display_string());", tmp, a, tmp)
                                }
                            })
                            .collect();
                        format!("{{ {} vm.print(\"\n\"); NiValue::None }}", print_parts.join(" "))
                    }
                }
                ExprKind::Identifier(name) if name == "len" => {
                    if args_strs.is_empty() {
                        "NiValue::Int(0)".to_string()
                    } else {
                        let arg = &args_strs[0];
                        format!("ni_method_call(vm, &{}, \"len\", &[])?", arg)
                    }
                }
                ExprKind::Identifier(name) if name == "type_of" => {
                    if args_strs.is_empty() {
                        "NiValue::String(Rc::new(\"none\".to_string()))".to_string()
                    } else {
                        format!(
                            "NiValue::String(Rc::new({}.type_name().to_lowercase()))",
                            args_strs[0]
                        )
                    }
                }
                ExprKind::Identifier(name) if name == "to_string" => {
                    if args_strs.is_empty() {
                        "NiValue::String(Rc::new(\"none\".to_string()))".to_string()
                    } else {
                        format!(
                            "NiValue::String(Rc::new({}.to_display_string()))",
                            args_strs[0]
                        )
                    }
                }
                ExprKind::Identifier(name) if name == "to_int" => {
                    if args_strs.is_empty() {
                        "NiValue::Int(0)".to_string()
                    } else {
                        let arg = &args_strs[0];
                        let tmp = gen.fresh_temp();
                        format!("{{ let {} = {}; match &{} {{ NiValue::Int(n) => NiValue::Int(*n), NiValue::Float(f) => NiValue::Int(*f as i64), NiValue::Bool(b) => NiValue::Int(if *b {{ 1 }} else {{ 0 }}), NiValue::String(s) => s.parse::<i64>().map(NiValue::Int).unwrap_or(NiValue::None), _ => NiValue::None }} }}", tmp, arg, tmp)
                    }
                }
                ExprKind::Identifier(name) if name == "to_float" => {
                    if args_strs.is_empty() {
                        "NiValue::Float(0.0)".to_string()
                    } else {
                        let arg = &args_strs[0];
                        let tmp = gen.fresh_temp();
                        format!("{{ let {} = {}; match &{} {{ NiValue::Int(n) => NiValue::Float(*n as f64), NiValue::Float(f) => NiValue::Float(*f), NiValue::String(s) => s.parse::<f64>().map(NiValue::Float).unwrap_or(NiValue::None), _ => NiValue::None }} }}", tmp, arg, tmp)
                    }
                }
                ExprKind::Identifier(name) if name == "range" => match args_strs.len() {
                    1 => {
                        format!("{{ let _end = {}; if let NiValue::Int(e) = _end {{ NiValue::Range(NiRange {{ start: 0, end: e, inclusive: false, step: 1 }}) }} else {{ return Err(NiRuntimeError::new(\"range() requires Int arguments\")); }} }}", args_strs[0])
                    }
                    2 => {
                        format!("{{ let _start = {}; let _end = {}; if let (NiValue::Int(s), NiValue::Int(e)) = (&_start, &_end) {{ NiValue::Range(NiRange {{ start: *s, end: *e, inclusive: false, step: 1 }}) }} else {{ return Err(NiRuntimeError::new(\"range() requires Int arguments\")); }} }}", args_strs[0], args_strs[1])
                    }
                    3 => {
                        format!("{{ let _start = {}; let _end = {}; let _step = {}; if let (NiValue::Int(s), NiValue::Int(e), NiValue::Int(st)) = (&_start, &_end, &_step) {{ if *st == 0 {{ return Err(NiRuntimeError::new(\"range() step must not be zero\")); }} NiValue::Range(NiRange {{ start: *s, end: *e, inclusive: false, step: *st }}) }} else {{ return Err(NiRuntimeError::new(\"range() requires Int arguments\")); }} }}", args_strs[0], args_strs[1], args_strs[2])
                    }
                    _ => "NiValue::None".to_string(),
                },
                // Known user-defined function: call directly
                ExprKind::Identifier(name) if gen.known_functions.contains(name.as_str()) => {
                    let mangled = name_mangler::mangle_fun(name);
                    let args_array = format!("&[{}]", args_strs.join(", "));
                    format!("{}(vm, {})?", mangled, args_array)
                }
                // Known user-defined class: instantiate via ni_call on class value
                ExprKind::Identifier(name) if gen.known_classes.contains(name.as_str()) => {
                    let register_fn = name_mangler::mangle_class(name);
                    let args_array = format!("&[{}]", args_strs.join(", "));
                    format!(
                        "ni_call(vm, &NiValue::Class(Rc::new({}_register())), {})?",
                        register_fn, args_array
                    )
                }
                _ => {
                    let callee_val = emit_expr(gen, callee);
                    let args_array = format!("&[{}]", args_strs.join(", "));
                    format!("ni_call(vm, &{}, {})?", callee_val, args_array)
                }
            }
        }

        // Method call
        ExprKind::MethodCall {
            object,
            method,
            args,
            named_args: _,
        } => {
            let obj_val = emit_expr(gen, object);
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            let args_array = format!("&[{}]", args_strs.join(", "));
            format!(
                "ni_method_call(vm, &{}, \"{}\", {})?",
                obj_val, method, args_array
            )
        }

        // Super call -- dispatch to parent class method directly
        ExprKind::SuperCall { method, args } => {
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            let args_array = format!("&[{}]", args_strs.join(", "));
            if let Some(class_name) = &gen.current_class {
                if let Some(superclass) = gen.superclass_map.get(class_name).cloned() {
                    let mangled = name_mangler::mangle_method(&superclass, method);
                    return format!("{}(vm, self_val, {})?", mangled, args_array);
                }
            }
            // Fallback: no superclass info available
            format!(
                "/* super.{}({}) -- no superclass */ NiValue::None",
                method, args_array
            )
        }

        // Collections
        ExprKind::List(items) => {
            let item_strs: Vec<String> = items.iter().map(|i| emit_expr(gen, i)).collect();
            format!(
                "NiValue::List(Rc::new(RefCell::new(vec![{}])))",
                item_strs.join(", ")
            )
        }
        ExprKind::Map(pairs) => {
            let pair_strs: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    let key = emit_expr(gen, k);
                    let val = emit_expr(gen, v);
                    format!("({}, {})", key, val)
                })
                .collect();
            format!(
                "NiValue::Map(Rc::new(RefCell::new(vec![{}])))",
                pair_strs.join(", ")
            )
        }

        // Range
        ExprKind::Range {
            start,
            end,
            inclusive,
        } => {
            let s = emit_expr(gen, start);
            let e = emit_expr(gen, end);
            let tmp_s = gen.fresh_temp();
            let tmp_e = gen.fresh_temp();
            format!(
                "{{ let {} = {}; let {} = {}; if let (NiValue::Int(s), NiValue::Int(e)) = (&{}, &{}) {{ NiValue::Range(NiRange {{ start: *s, end: *e, inclusive: {}, step: 1 }}) }} else {{ return Err(NiRuntimeError::new(\"Range requires Int endpoints\")); }} }}",
                tmp_s, s, tmp_e, e, tmp_s, tmp_e, inclusive
            )
        }

        // Lambda
        ExprKind::Lambda { params, body } => {
            let lambda_name = name_mangler::mangle_lambda(gen.lambda_counter);
            gen.lambda_counter += 1;

            // Collect free variables referenced in the lambda body
            let param_names: std::collections::HashSet<String> =
                params.iter().map(|p| p.name.clone()).collect();
            let free_vars = collect_free_vars_stmts(
                body,
                &param_names,
                &gen.known_functions,
                &gen.known_classes,
                &gen.known_enums,
            );

            // Generate the lambda body
            let saved_output = std::mem::take(&mut gen.output);
            let saved_in_function = gen.in_function;
            let saved_indent = gen.indent;
            gen.in_function = true;
            gen.indent = 1;
            gen.output = String::new();

            for (i, param) in params.iter().enumerate() {
                let pname = name_mangler::mangle_local(&param.name);
                if let Some(default) = &param.default {
                    let default_val = emit_expr(gen, default);
                    gen.writeln_to_current(&format!(
                        "let mut {} = args.get({}).cloned().unwrap_or({});",
                        pname, i, default_val
                    ));
                } else {
                    gen.writeln_to_current(&format!(
                        "let mut {} = args.get({}).cloned().unwrap_or(NiValue::None);",
                        pname, i
                    ));
                }
            }

            // Lambda body: if single expression statement, return its value
            if body.len() == 1 {
                if let StmtKind::Expr(expr) = &body[0].kind {
                    let val = emit_expr(gen, expr);
                    gen.writeln_to_current(&format!("Ok({})", val));
                } else {
                    gen.emit_body(body);
                    gen.writeln_to_current("Ok(NiValue::None)");
                }
            } else {
                gen.emit_body(body);
                gen.writeln_to_current("Ok(NiValue::None)");
            }

            let lambda_body = std::mem::take(&mut gen.output);

            gen.output = saved_output;
            gen.in_function = saved_in_function;
            gen.indent = saved_indent;

            if free_vars.is_empty() {
                // No captures needed -- emit as a standalone function
                gen.declarations.push_str(&format!(
                    "pub fn {}(vm: &mut dyn NiVm, args: &[NiValue]) -> NiResult<NiValue> {{\n",
                    lambda_name
                ));
                gen.declarations.push_str(&lambda_body);
                gen.declarations.push_str("}\n\n");

                format!("NiValue::Function(NiFunctionRef {{ name: \"{}\".into(), func: Rc::new(|vm, args| {}(vm, args)) }})",
                        lambda_name, lambda_name)
            } else {
                // Captures needed -- emit as an inline Rust move closure
                let captured_clones: Vec<String> = free_vars
                    .iter()
                    .map(|v| {
                        format!(
                            "let {v}_capture = {v}.clone();",
                            v = name_mangler::mangle_local(v)
                        )
                    })
                    .collect();
                let capture_rebinds: Vec<String> = free_vars
                    .iter()
                    .map(|v| {
                        let m = name_mangler::mangle_local(v);
                        format!("    let mut {} = {}_capture.clone();\n", m, m)
                    })
                    .collect();
                format!(
                    "{{ {captures} NiValue::Function(NiFunctionRef {{ name: \"{name}\".into(), func: Rc::new(move |vm, args| {{ {rebinds}{body}}} ) }}) }}",
                    captures = captured_clones.join(" "),
                    name = lambda_name,
                    rebinds = capture_rebinds.join(""),
                    body = lambda_body.trim(),
                )
            }
        }

        // Ternary / if expression
        ExprKind::IfExpr {
            value,
            condition,
            else_value,
        } => {
            let cond = emit_expr(gen, condition);
            let then_val = emit_expr(gen, value);
            let else_val = emit_expr(gen, else_value);
            format!(
                "if ni_is_truthy(&{}) {{ {} }} else {{ {} }}",
                cond, then_val, else_val
            )
        }

        // Spawn
        ExprKind::Spawn(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* spawn */ {}", val)
        }

        // Yield
        ExprKind::Yield(val) => {
            if let Some(expr) = val {
                let v = emit_expr(gen, expr);
                format!("/* yield {} */ NiValue::None", v)
            } else {
                "/* yield */ NiValue::None".to_string()
            }
        }

        // Wait
        ExprKind::Wait(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* wait {} */ NiValue::None", val)
        }

        // Await
        ExprKind::Await(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* await */ {}", val)
        }

        // Try expression
        ExprKind::TryExpr(inner) => {
            let val = emit_expr(gen, inner);
            format!(
                "(|| -> NiResult<NiValue> {{ Ok({}) }})().unwrap_or(NiValue::None)",
                val
            )
        }

        // Fail expression
        ExprKind::FailExpr(inner) => {
            let val = emit_expr(gen, inner);
            format!("return Err(NiRuntimeError::from_value({}))", val)
        }

        // F-string
        ExprKind::FStringLiteral(parts) => {
            let mut format_parts = Vec::new();
            for part in parts {
                match part {
                    FStringPart::Literal(s) => {
                        format_parts.push(format!("\"{}\"", escape_string(s)));
                    }
                    FStringPart::Expr(expr) => {
                        let val = emit_expr(gen, expr);
                        format_parts.push(format!("&{}.to_display_string()", val));
                    }
                }
            }
            if format_parts.is_empty() {
                "NiValue::String(Rc::new(String::new()))".to_string()
            } else {
                let concat = format_parts
                    .iter()
                    .map(|p| format!("{}.to_string()", p))
                    .collect::<Vec<_>>()
                    .join(" + &");
                format!("NiValue::String(Rc::new({}))", concat)
            }
        }
    }
}

pub(crate) fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\0', "\\0")
}

/// Collect free variables referenced in a list of statements that are not:
/// - declared as parameters
/// - declared locally (var/const/for binding)
/// - known top-level functions, classes, or enums
/// - built-in names (print, len, etc.)
pub(crate) fn collect_free_vars_stmts(
    stmts: &[Statement],
    bound: &std::collections::HashSet<String>,
    known_fns: &std::collections::HashSet<String>,
    known_classes: &std::collections::HashSet<String>,
    known_enums: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut free = std::collections::HashSet::new();
    let mut local_bound = bound.clone();
    for stmt in stmts {
        collect_free_vars_stmt(
            stmt,
            &mut local_bound,
            known_fns,
            known_classes,
            known_enums,
            &mut free,
        );
    }
    let mut result: Vec<String> = free.into_iter().collect();
    result.sort();
    result
}

fn collect_free_vars_stmt(
    stmt: &Statement,
    bound: &mut std::collections::HashSet<String>,
    known_fns: &std::collections::HashSet<String>,
    known_classes: &std::collections::HashSet<String>,
    known_enums: &std::collections::HashSet<String>,
    free: &mut std::collections::HashSet<String>,
) {
    match &stmt.kind {
        StmtKind::Expr(e) => {
            collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free)
        }
        StmtKind::VarDecl(decl) => {
            collect_free_vars_expr(
                &decl.initializer,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            bound.insert(decl.name.clone());
        }
        StmtKind::ConstDecl(decl) => {
            collect_free_vars_expr(
                &decl.initializer,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            bound.insert(decl.name.clone());
        }
        StmtKind::If(if_stmt) => {
            collect_free_vars_expr(
                &if_stmt.condition,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            for s in &if_stmt.then_body {
                collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
            }
            for (cond, body) in &if_stmt.elif_branches {
                collect_free_vars_expr(cond, bound, known_fns, known_classes, known_enums, free);
                for s in body {
                    collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
                }
            }
            if let Some(eb) = &if_stmt.else_body {
                for s in eb {
                    collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
                }
            }
        }
        StmtKind::While(while_stmt) => {
            collect_free_vars_expr(
                &while_stmt.condition,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            for s in &while_stmt.body {
                collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
            }
        }
        StmtKind::For(for_stmt) => {
            collect_free_vars_expr(
                &for_stmt.iterable,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            bound.insert(for_stmt.variable.clone());
            for s in &for_stmt.body {
                collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
            }
        }
        StmtKind::Return(Some(e)) => {
            collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free)
        }
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue | StmtKind::Pass => {}
        StmtKind::Match(match_stmt) => {
            collect_free_vars_expr(
                &match_stmt.subject,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            for case in &match_stmt.cases {
                for s in &case.body {
                    collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
                }
            }
        }
        StmtKind::Try(try_stmt) => {
            for s in &try_stmt.body {
                collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
            }
            if let Some(param) = &try_stmt.catch_var {
                bound.insert(param.clone());
            }
            match &try_stmt.catch_body {
                CatchBody::Block(stmts) => {
                    for s in stmts {
                        collect_free_vars_stmt(
                            s,
                            bound,
                            known_fns,
                            known_classes,
                            known_enums,
                            free,
                        );
                    }
                }
                CatchBody::Match(cases) => {
                    for case in cases {
                        for s in &case.body {
                            collect_free_vars_stmt(
                                s,
                                bound,
                                known_fns,
                                known_classes,
                                known_enums,
                                free,
                            );
                        }
                    }
                }
            }
        }
        StmtKind::Assert(condition, message) => {
            collect_free_vars_expr(
                condition,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            if let Some(m) = message {
                collect_free_vars_expr(m, bound, known_fns, known_classes, known_enums, free);
            }
        }
        StmtKind::Fail(e) => {
            collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free)
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_free_vars_stmt(s, bound, known_fns, known_classes, known_enums, free);
            }
        }
    }
}

const BUILTIN_NAMES: &[&str] = &[
    "print",
    "len",
    "type_of",
    "to_string",
    "to_int",
    "to_float",
    "range",
    "true",
    "false",
    "none",
    "self",
];

fn collect_free_vars_expr(
    expr: &Expr,
    bound: &std::collections::HashSet<String>,
    known_fns: &std::collections::HashSet<String>,
    known_classes: &std::collections::HashSet<String>,
    known_enums: &std::collections::HashSet<String>,
    free: &mut std::collections::HashSet<String>,
) {
    match &expr.kind {
        ExprKind::Identifier(name) => {
            if !bound.contains(name.as_str())
                && !known_fns.contains(name.as_str())
                && !known_classes.contains(name.as_str())
                && !known_enums.contains(name.as_str())
                && !BUILTIN_NAMES.contains(&name.as_str())
            {
                free.insert(name.clone());
            }
        }
        ExprKind::BinaryOp { left, op: _, right }
        | ExprKind::And(left, right)
        | ExprKind::Or(left, right)
        | ExprKind::NoneCoalesce(left, right) => {
            collect_free_vars_expr(left, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(right, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::Compare { left, op: _, right } => {
            collect_free_vars_expr(left, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(right, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::Negate(e)
        | ExprKind::Not(e)
        | ExprKind::TryExpr(e)
        | ExprKind::FailExpr(e)
        | ExprKind::Spawn(e)
        | ExprKind::Wait(e)
        | ExprKind::Await(e) => {
            collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::Assign { target, value }
        | ExprKind::CompoundAssign {
            target,
            op: _,
            value,
        } => {
            collect_free_vars_expr(target, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(value, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::Call {
            callee,
            args,
            named_args: _,
        } => {
            collect_free_vars_expr(callee, bound, known_fns, known_classes, known_enums, free);
            for a in args {
                collect_free_vars_expr(a, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::MethodCall {
            object,
            method: _,
            args,
            named_args: _,
        } => {
            collect_free_vars_expr(object, bound, known_fns, known_classes, known_enums, free);
            for a in args {
                collect_free_vars_expr(a, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::GetField(obj, _) | ExprKind::SafeNav(obj, _) => {
            collect_free_vars_expr(obj, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::SetField(obj, _, val) => {
            collect_free_vars_expr(obj, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(val, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::GetIndex(obj, idx) => {
            collect_free_vars_expr(obj, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(idx, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::SetIndex(obj, idx, val) => {
            collect_free_vars_expr(obj, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(idx, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(val, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::List(items) => {
            for i in items {
                collect_free_vars_expr(i, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::Map(pairs) => {
            for (k, v) in pairs {
                collect_free_vars_expr(k, bound, known_fns, known_classes, known_enums, free);
                collect_free_vars_expr(v, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::Range { start, end, .. } => {
            collect_free_vars_expr(start, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(end, bound, known_fns, known_classes, known_enums, free);
        }
        ExprKind::IfExpr {
            value,
            condition,
            else_value,
        } => {
            collect_free_vars_expr(value, bound, known_fns, known_classes, known_enums, free);
            collect_free_vars_expr(
                condition,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
            collect_free_vars_expr(
                else_value,
                bound,
                known_fns,
                known_classes,
                known_enums,
                free,
            );
        }
        ExprKind::Lambda { params, body } => {
            let mut inner_bound = bound.clone();
            for p in params {
                inner_bound.insert(p.name.clone());
            }
            for s in body {
                collect_free_vars_stmt(
                    s,
                    &mut inner_bound,
                    known_fns,
                    known_classes,
                    known_enums,
                    free,
                );
            }
        }
        ExprKind::SuperCall { method: _, args } => {
            for a in args {
                collect_free_vars_expr(a, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::Yield(val) => {
            if let Some(e) = val {
                collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free);
            }
        }
        ExprKind::FStringLiteral(parts) => {
            for part in parts {
                if let FStringPart::Expr(e) = part {
                    collect_free_vars_expr(e, bound, known_fns, known_classes, known_enums, free);
                }
            }
        }
        // Literals and self have no free variables
        ExprKind::IntLiteral(_)
        | ExprKind::FloatLiteral(_)
        | ExprKind::StringLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::NoneLiteral
        | ExprKind::SelfExpr => {}
    }
}
