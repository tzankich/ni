use super::CCodeGen;
use crate::name_mangler;
use ni_parser::*;

pub(crate) fn emit_expr(gen: &mut CCodeGen, expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::IntLiteral(n) => format!("ni_int({})", n),
        ExprKind::FloatLiteral(f) => {
            let s = format!("{}", f);
            if s.contains('.') || s.contains('e') || s.contains('E') {
                format!("ni_float({})", s)
            } else {
                format!("ni_float({}.0)", s)
            }
        }
        ExprKind::StringLiteral(s) => {
            let escaped = escape_string_c(s);
            format!("ni_string(\"{}\")", escaped)
        }
        ExprKind::BoolLiteral(b) => {
            if *b {
                "ni_bool(1)".to_string()
            } else {
                "ni_bool(0)".to_string()
            }
        }
        ExprKind::NoneLiteral => "NI_NONE".to_string(),

        ExprKind::Identifier(name) => {
            if name == "self" && gen.current_class.is_some() {
                return "self_val".to_string();
            }
            // Known functions: wrap as a function value
            if gen.known_functions.contains(name.as_str()) {
                let mangled = name_mangler::mangle_fun(name);
                return format!("ni_make_function(\"{}\", {})", name, mangled);
            }
            // Known classes: wrap as a constructor reference
            if gen.known_classes.contains(name.as_str()) {
                let mangled = name_mangler::mangle_class(name);
                return format!("{}_new_value()", mangled);
            }
            name_mangler::mangle_local(name)
        }

        ExprKind::SelfExpr => "self_val".to_string(),

        ExprKind::Negate(inner) => {
            let val = emit_expr(gen, inner);
            format!("ni_negate({})", val)
        }
        ExprKind::Not(inner) => {
            let val = emit_expr(gen, inner);
            format!("ni_not({})", val)
        }

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
            format!("{}({}, {})", func, l, r)
        }

        ExprKind::And(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            // Use comma operator in a block expression (GCC extension) or ternary
            format!(
                "({{ NiValue {} = {}; ni_is_truthy({}) ? {} : {}; }})",
                tmp, l, tmp, r, tmp
            )
        }
        ExprKind::Or(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            format!(
                "({{ NiValue {} = {}; ni_is_truthy({}) ? {} : {}; }})",
                tmp, l, tmp, tmp, r
            )
        }

        ExprKind::Compare { left, op, right } => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            match op {
                CmpOp::Eq => format!("ni_eq({}, {})", l, r),
                CmpOp::NotEq => format!("ni_neq({}, {})", l, r),
                CmpOp::Lt => format!("ni_less_than({}, {})", l, r),
                CmpOp::Gt => format!("ni_greater_than({}, {})", l, r),
                CmpOp::LtEq => format!("ni_less_eq({}, {})", l, r),
                CmpOp::GtEq => format!("ni_greater_eq({}, {})", l, r),
                CmpOp::Is => {
                    if let ExprKind::Identifier(type_name) = &right.kind {
                        format!("ni_is_type({}, \"{}\")", l, type_name)
                    } else {
                        format!("ni_eq({}, {})", l, r)
                    }
                }
                CmpOp::In => format!("ni_in({}, {})", l, r),
            }
        }

        ExprKind::Assign { target, value } => {
            let val = emit_expr(gen, value);
            match &target.kind {
                ExprKind::Identifier(name) => {
                    let mangled = name_mangler::mangle_local(name);
                    format!("({} = {})", mangled, val)
                }
                ExprKind::GetField(obj, field) => {
                    let obj_val = emit_expr(gen, obj);
                    format!("ni_set_prop({}, \"{}\", {})", obj_val, field, val)
                }
                ExprKind::GetIndex(obj, idx) => {
                    let obj_val = emit_expr(gen, obj);
                    let idx_val = emit_expr(gen, idx);
                    format!("ni_set_index({}, {}, {})", obj_val, idx_val, val)
                }
                _ => format!("/* unsupported assign */ {}", val),
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
                    format!("({} = {}({}, {}))", mangled, func, mangled, val)
                }
                _ => "/* unsupported compound assign */ NI_NONE".to_string(),
            }
        }

        ExprKind::GetField(obj, field) => {
            let obj_val = emit_expr(gen, obj);
            format!("ni_get_prop({}, \"{}\")", obj_val, field)
        }
        ExprKind::SetField(obj, field, value) => {
            let obj_val = emit_expr(gen, obj);
            let val = emit_expr(gen, value);
            format!("ni_set_prop({}, \"{}\", {})", obj_val, field, val)
        }

        ExprKind::GetIndex(obj, index) => {
            let obj_val = emit_expr(gen, obj);
            let idx_val = emit_expr(gen, index);
            format!("ni_get_index({}, {})", obj_val, idx_val)
        }
        ExprKind::SetIndex(obj, index, value) => {
            let obj_val = emit_expr(gen, obj);
            let idx_val = emit_expr(gen, index);
            let val = emit_expr(gen, value);
            format!("ni_set_index({}, {}, {})", obj_val, idx_val, val)
        }

        ExprKind::SafeNav(obj, field) => {
            let obj_val = emit_expr(gen, obj);
            let tmp = gen.fresh_temp();
            format!(
                "({{ NiValue {} = {}; ni_is_none({}) ? NI_NONE : ni_get_prop({}, \"{}\"); }})",
                tmp, obj_val, tmp, tmp, field
            )
        }

        ExprKind::NoneCoalesce(left, right) => {
            let l = emit_expr(gen, left);
            let r = emit_expr(gen, right);
            let tmp = gen.fresh_temp();
            format!(
                "({{ NiValue {} = {}; ni_is_none({}) ? {} : {}; }})",
                tmp, l, tmp, r, tmp
            )
        }

        ExprKind::Call {
            callee,
            args,
            named_args: _,
        } => {
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            let argc = args_strs.len();

            match &callee.kind {
                ExprKind::Identifier(name) if name == "print" => {
                    if args_strs.is_empty() {
                        "ni_print_newline(vm)".to_string()
                    } else {
                        let prints: Vec<String> = args_strs
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                if i > 0 {
                                    format!("ni_print_space(vm); ni_print(vm, {})", a)
                                } else {
                                    format!("ni_print(vm, {})", a)
                                }
                            })
                            .collect();
                        format!("({{ {}; ni_print_newline(vm); NI_NONE; }})", prints.join("; "))
                    }
                }
                ExprKind::Identifier(name) if name == "len" => {
                    if args_strs.is_empty() {
                        "ni_int(0)".to_string()
                    } else {
                        format!("ni_len({})", args_strs[0])
                    }
                }
                ExprKind::Identifier(name) if name == "type_of" => {
                    if args_strs.is_empty() {
                        "ni_string(\"none\")".to_string()
                    } else {
                        format!("ni_type_of({})", args_strs[0])
                    }
                }
                ExprKind::Identifier(name) if name == "to_string" => {
                    if args_strs.is_empty() {
                        "ni_string(\"none\")".to_string()
                    } else {
                        format!("ni_to_string({})", args_strs[0])
                    }
                }
                ExprKind::Identifier(name) if name == "to_int" => {
                    if args_strs.is_empty() {
                        "ni_int(0)".to_string()
                    } else {
                        format!("ni_to_int({})", args_strs[0])
                    }
                }
                ExprKind::Identifier(name) if name == "to_float" => {
                    if args_strs.is_empty() {
                        "ni_float(0.0)".to_string()
                    } else {
                        format!("ni_to_float({})", args_strs[0])
                    }
                }
                ExprKind::Identifier(name) if name == "range" => match args_strs.len() {
                    1 => format!("ni_range(ni_int(0), {})", args_strs[0]),
                    2 => format!("ni_range({}, {})", args_strs[0], args_strs[1]),
                    _ => "NI_NONE".to_string(),
                },
                // Known user-defined function: call directly
                ExprKind::Identifier(name) if gen.known_functions.contains(name.as_str()) => {
                    let mangled = name_mangler::mangle_fun(name);
                    if argc == 0 {
                        format!("{}(vm, NULL, 0)", mangled)
                    } else {
                        let args_array = format!("(NiValue[]){{ {} }}", args_strs.join(", "));
                        format!("{}(vm, {}, {})", mangled, args_array, argc)
                    }
                }
                // Known user-defined class: construct via class new function
                ExprKind::Identifier(name) if gen.known_classes.contains(name.as_str()) => {
                    let mangled = name_mangler::mangle_class(name);
                    if argc == 0 {
                        format!("{}_new(vm, NULL, 0)", mangled)
                    } else {
                        let args_array = format!("(NiValue[]){{ {} }}", args_strs.join(", "));
                        format!("{}_new(vm, {}, {})", mangled, args_array, argc)
                    }
                }
                _ => {
                    let callee_val = emit_expr(gen, callee);
                    if argc == 0 {
                        format!("ni_call(vm, {}, NULL, 0)", callee_val)
                    } else {
                        let args_array = format!("(NiValue[]){{ {} }}", args_strs.join(", "));
                        format!("ni_call(vm, {}, {}, {})", callee_val, args_array, argc)
                    }
                }
            }
        }

        ExprKind::MethodCall {
            object,
            method,
            args,
            named_args: _,
        } => {
            let obj_val = emit_expr(gen, object);
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            let argc = args_strs.len();
            if argc == 0 {
                format!("ni_method_call(vm, {}, \"{}\", NULL, 0)", obj_val, method)
            } else {
                let args_array = format!("(NiValue[]){{ {} }}", args_strs.join(", "));
                format!(
                    "ni_method_call(vm, {}, \"{}\", {}, {})",
                    obj_val, method, args_array, argc
                )
            }
        }

        ExprKind::SuperCall { method, args } => {
            let args_strs: Vec<String> = args.iter().map(|a| emit_expr(gen, a)).collect();
            if let Some(class_name) = &gen.current_class {
                if let Some(superclass) = gen.superclass_map.get(class_name).cloned() {
                    let mangled = name_mangler::mangle_method(&superclass, method);
                    let argc = args_strs.len();
                    if argc == 0 {
                        return format!("{}(vm, self_val, NULL, 0)", mangled);
                    } else {
                        let args_array = format!("(NiValue[]){{ {} }}", args_strs.join(", "));
                        return format!("{}(vm, self_val, {}, {})", mangled, args_array, argc);
                    }
                }
            }
            // Fallback: no superclass info available
            format!(
                "/* super.{}({}) -- no superclass */ NI_NONE",
                method,
                args_strs.join(", ")
            )
        }

        ExprKind::List(items) => {
            let item_strs: Vec<String> = items.iter().map(|i| emit_expr(gen, i)).collect();
            let count = item_strs.len();
            if count == 0 {
                "ni_list(NULL, 0)".to_string()
            } else {
                format!(
                    "ni_list((NiValue[]){{ {} }}, {})",
                    item_strs.join(", "),
                    count
                )
            }
        }

        ExprKind::Map(pairs) => {
            let count = pairs.len();
            if count == 0 {
                "ni_map(NULL, NULL, 0)".to_string()
            } else {
                let keys: Vec<String> = pairs.iter().map(|(k, _)| emit_expr(gen, k)).collect();
                let vals: Vec<String> = pairs.iter().map(|(_, v)| emit_expr(gen, v)).collect();
                format!(
                    "ni_map((NiValue[]){{ {} }}, (NiValue[]){{ {} }}, {})",
                    keys.join(", "),
                    vals.join(", "),
                    count
                )
            }
        }

        ExprKind::Range {
            start,
            end,
            inclusive,
        } => {
            let s = emit_expr(gen, start);
            let e = emit_expr(gen, end);
            format!(
                "ni_make_range({}, {}, {})",
                s,
                e,
                if *inclusive { 1 } else { 0 }
            )
        }

        ExprKind::Lambda { params, body } => {
            let lambda_name = name_mangler::mangle_lambda(gen.lambda_counter);
            gen.lambda_counter += 1;

            // Forward declare
            gen.forward_decls.push_str(&format!(
                "NiValue {}(NiVm* vm, NiValue* args, int argc);\n",
                lambda_name
            ));

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
                        "NiValue {} = (argc > {}) ? args[{}] : {};",
                        pname, i, i, default_val
                    ));
                } else {
                    gen.writeln_to_current(&format!(
                        "NiValue {} = (argc > {}) ? args[{}] : NI_NONE;",
                        pname, i, i
                    ));
                }
            }

            if body.len() == 1 {
                if let StmtKind::Expr(expr) = &body[0].kind {
                    let val = emit_expr(gen, expr);
                    gen.writeln_to_current(&format!("return {};", val));
                } else {
                    gen.emit_body(body);
                    gen.writeln_to_current("return NI_NONE;");
                }
            } else {
                gen.emit_body(body);
                gen.writeln_to_current("return NI_NONE;");
            }

            let lambda_body = std::mem::take(&mut gen.output);

            gen.definitions.push_str(&format!(
                "NiValue {}(NiVm* vm, NiValue* args, int argc) {{\n",
                lambda_name
            ));
            gen.definitions.push_str(&lambda_body);
            gen.definitions.push_str("}\n\n");

            gen.output = saved_output;
            gen.in_function = saved_in_function;
            gen.indent = saved_indent;

            format!("ni_make_function(\"{}\", {})", lambda_name, lambda_name)
        }

        ExprKind::IfExpr {
            value,
            condition,
            else_value,
        } => {
            let cond = emit_expr(gen, condition);
            let then_val = emit_expr(gen, value);
            let else_val = emit_expr(gen, else_value);
            format!("(ni_is_truthy({}) ? {} : {})", cond, then_val, else_val)
        }

        ExprKind::Spawn(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* spawn */ {}", val)
        }

        ExprKind::Yield(val) => {
            if let Some(expr) = val {
                let v = emit_expr(gen, expr);
                format!("/* yield {} */ NI_NONE", v)
            } else {
                "/* yield */ NI_NONE".to_string()
            }
        }

        ExprKind::Wait(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* wait {} */ NI_NONE", val)
        }

        ExprKind::Await(expr) => {
            let val = emit_expr(gen, expr);
            format!("/* await */ {}", val)
        }

        ExprKind::TryExpr(inner) => {
            let val = emit_expr(gen, inner);
            // In C, we'd need setjmp/longjmp; simplified version:
            format!("ni_try_expr({})", val)
        }

        ExprKind::FailExpr(inner) => {
            let val = emit_expr(gen, inner);
            format!("ni_fail({})", val)
        }

        ExprKind::FStringLiteral(parts) => {
            if parts.is_empty() {
                return "ni_string(\"\")".to_string();
            }
            let part_strs: Vec<String> = parts
                .iter()
                .map(|part| match part {
                    FStringPart::Literal(s) => format!("ni_string(\"{}\")", escape_string_c(s)),
                    FStringPart::Expr(expr) => {
                        let val = emit_expr(gen, expr);
                        format!("ni_to_string({})", val)
                    }
                })
                .collect();
            // Chain concatenation
            let mut result = part_strs[0].clone();
            for part in &part_strs[1..] {
                result = format!("ni_str_concat({}, {})", result, part);
            }
            result
        }
    }
}

pub(crate) fn escape_string_c(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\0', "\\0")
}
