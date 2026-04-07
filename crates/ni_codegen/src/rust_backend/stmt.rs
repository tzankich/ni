use super::RustCodeGen;
use crate::name_mangler;
use crate::trait_def::NiCodeGen;
use ni_parser::*;

pub(crate) fn emit_statement(gen: &mut RustCodeGen, stmt: &Statement) {
    match &stmt.kind {
        StmtKind::Expr(expr) => {
            let val = gen.emit_expr(expr);
            // Discard the value with a let _ binding (avoids unused warnings)
            gen.writeln_to_current(&format!("let _ = {};", val));
        }

        StmtKind::VarDecl(decl) => {
            gen.emit_var_decl(decl);
        }

        StmtKind::ConstDecl(decl) => {
            gen.emit_const_decl(decl);
        }

        StmtKind::If(if_stmt) => {
            emit_if(gen, if_stmt);
        }

        StmtKind::While(while_stmt) => {
            gen.writeln_to_current("loop {");
            gen.indent += 1;
            // Evaluate condition each iteration
            let cond = gen.emit_expr(&while_stmt.condition);
            gen.writeln_to_current(&format!("if !ni_is_truthy(&{}) {{ break; }}", cond));
            gen.emit_body(&while_stmt.body);
            gen.indent -= 1;
            gen.writeln_to_current("}");
        }

        StmtKind::For(for_stmt) => {
            emit_for(gen, for_stmt);
        }

        StmtKind::Match(match_stmt) => {
            emit_match(gen, match_stmt);
        }

        StmtKind::Return(expr) => {
            if let Some(e) = expr {
                let val = gen.emit_expr(e);
                gen.writeln_to_current(&format!("return Ok({});", val));
            } else {
                gen.writeln_to_current("return Ok(NiValue::None);");
            }
        }

        StmtKind::Break => {
            gen.writeln_to_current("break;");
        }

        StmtKind::Continue => {
            gen.writeln_to_current("continue;");
        }

        StmtKind::Pass => {
            // no-op
        }

        StmtKind::Block(stmts) => {
            gen.writeln_to_current("{");
            gen.indent += 1;
            gen.emit_body(stmts);
            gen.indent -= 1;
            gen.writeln_to_current("}");
        }

        StmtKind::Try(try_stmt) => {
            emit_try(gen, try_stmt);
        }

        StmtKind::Fail(expr) => {
            let val = gen.emit_expr(expr);
            gen.writeln_to_current(&format!("return Err(NiRuntimeError::from_value({}));", val));
        }

        StmtKind::Assert(expr, msg) => {
            let val = gen.emit_expr(expr);
            if let Some(m) = msg {
                let msg_val = gen.emit_expr(m);
                gen.writeln_to_current(&format!(
                    "if !ni_is_truthy(&{}) {{ return Err(NiRuntimeError::new(format!(\"Assertion failed: {{}}\", {}.to_display_string()))); }}",
                    val, msg_val
                ));
            } else {
                gen.writeln_to_current(&format!(
                    "if !ni_is_truthy(&{}) {{ return Err(NiRuntimeError::new(\"Assertion failed\")); }}",
                    val
                ));
            }
        }
    }
}

fn emit_if(gen: &mut RustCodeGen, if_stmt: &IfStmt) {
    let cond = gen.emit_expr(&if_stmt.condition);
    gen.writeln_to_current(&format!("if ni_is_truthy(&{}) {{", cond));
    gen.indent += 1;
    gen.emit_body(&if_stmt.then_body);
    gen.indent -= 1;

    for (elif_cond, elif_body) in &if_stmt.elif_branches {
        let cond = gen.emit_expr(elif_cond);
        gen.writeln_to_current(&format!("}} else if ni_is_truthy(&{}) {{", cond));
        gen.indent += 1;
        gen.emit_body(elif_body);
        gen.indent -= 1;
    }

    if let Some(else_body) = &if_stmt.else_body {
        gen.writeln_to_current("} else {");
        gen.indent += 1;
        gen.emit_body(else_body);
        gen.indent -= 1;
    }

    gen.writeln_to_current("}");
}

fn emit_for(gen: &mut RustCodeGen, for_stmt: &ForStmt) {
    let iter_val = gen.emit_expr(&for_stmt.iterable);
    let iter_tmp = gen.fresh_temp();
    gen.writeln_to_current(&format!(
        "let mut {} = ni_get_iterator(&{})?;",
        iter_tmp, iter_val
    ));

    if let Some(second_var) = &for_stmt.second_var {
        // for key, value in collection
        let key_name = name_mangler::mangle_local(&for_stmt.variable);
        let val_name = name_mangler::mangle_local(second_var);
        gen.writeln_to_current("loop {");
        gen.indent += 1;
        gen.writeln_to_current(&format!(
            "match ni_iterator_next_pair(&mut {})? {{",
            iter_tmp
        ));
        gen.indent += 1;
        gen.writeln_to_current(&format!("Some((mut {}, mut {})) => {{", key_name, val_name));
        gen.indent += 1;
        gen.emit_body(&for_stmt.body);
        gen.indent -= 1;
        gen.writeln_to_current("}");
        gen.writeln_to_current("None => break,");
        gen.indent -= 1;
        gen.writeln_to_current("}");
        gen.indent -= 1;
        gen.writeln_to_current("}");
    } else {
        // for x in collection
        let var_name = name_mangler::mangle_local(&for_stmt.variable);
        gen.writeln_to_current("loop {");
        gen.indent += 1;
        gen.writeln_to_current(&format!("match ni_iterator_next(&mut {})? {{", iter_tmp));
        gen.indent += 1;
        gen.writeln_to_current(&format!("Some(mut {}) => {{", var_name));
        gen.indent += 1;
        gen.emit_body(&for_stmt.body);
        gen.indent -= 1;
        gen.writeln_to_current("}");
        gen.writeln_to_current("None => break,");
        gen.indent -= 1;
        gen.writeln_to_current("}");
        gen.indent -= 1;
        gen.writeln_to_current("}");
    }
}

fn emit_match(gen: &mut RustCodeGen, match_stmt: &MatchStmt) {
    let subject = gen.emit_expr(&match_stmt.subject);
    let subject_tmp = gen.fresh_temp();
    gen.writeln_to_current(&format!("let {} = {};", subject_tmp, subject));
    gen.writeln_to_current("'match_block: {");
    gen.indent += 1;

    for (i, case) in match_stmt.cases.iter().enumerate() {
        let conditions: Vec<String> = case
            .patterns
            .iter()
            .map(|p| {
                match p {
                    Pattern::Literal(expr) => {
                        let val = gen.emit_expr(expr);
                        format!("ni_is_truthy(&ni_eq(&{}, &{}))", subject_tmp, val)
                    }
                    Pattern::Wildcard => "true".to_string(),
                    Pattern::Binding(name) => {
                        // Binding always matches, but captures value
                        format!(
                            "{{ let mut {} = {}.clone(); true }}",
                            name_mangler::mangle_local(name),
                            subject_tmp
                        )
                    }
                    Pattern::TypeCheck(binding, type_name) => {
                        format!(
                            "{{ let mut {} = {}.clone(); ni_is_truthy(&ni_is(&{}, \"{}\")) }}",
                            name_mangler::mangle_local(binding),
                            subject_tmp,
                            subject_tmp,
                            type_name
                        )
                    }
                }
            })
            .collect();

        let combined_cond = if conditions.len() == 1 {
            conditions[0].clone()
        } else {
            conditions.join(" || ")
        };

        let keyword = if i == 0 { "if" } else { "} else if" };

        if let Some(guard) = &case.guard {
            let guard_val = gen.emit_expr(guard);
            gen.writeln_to_current(&format!(
                "{} {} && ni_is_truthy(&{}) {{",
                keyword, combined_cond, guard_val
            ));
        } else {
            gen.writeln_to_current(&format!("{} {} {{", keyword, combined_cond));
        }
        gen.indent += 1;

        // For binding patterns, need to redeclare the variable
        for pattern in &case.patterns {
            if let Pattern::Binding(name) = pattern {
                let mangled = name_mangler::mangle_local(name);
                gen.writeln_to_current(&format!("let mut {} = {}.clone();", mangled, subject_tmp));
            } else if let Pattern::TypeCheck(binding, _) = pattern {
                let mangled = name_mangler::mangle_local(binding);
                gen.writeln_to_current(&format!("let mut {} = {}.clone();", mangled, subject_tmp));
            }
        }

        gen.emit_body(&case.body);
        gen.indent -= 1;
    }

    if !match_stmt.cases.is_empty() {
        gen.writeln_to_current("}");
    }

    gen.indent -= 1;
    gen.writeln_to_current("}");
}

fn emit_try(gen: &mut RustCodeGen, try_stmt: &TryStmt) {
    gen.writeln_to_current("match (|| -> NiResult<NiValue> {");
    gen.indent += 1;
    gen.emit_body(&try_stmt.body);
    gen.writeln_to_current("Ok(NiValue::None)");
    gen.indent -= 1;
    gen.writeln_to_current("})() {");
    gen.indent += 1;
    gen.writeln_to_current("Ok(_) => {}");

    match &try_stmt.catch_body {
        CatchBody::Block(stmts) => {
            if let Some(var_name) = &try_stmt.catch_var {
                let mangled = name_mangler::mangle_local(var_name);
                gen.writeln_to_current("Err(_err) => {");
                gen.indent += 1;
                gen.writeln_to_current(&format!(
                    "let mut {} = _err.value.unwrap_or(NiValue::String(Rc::new(_err.message)));",
                    mangled
                ));
                gen.emit_body(stmts);
                gen.indent -= 1;
                gen.writeln_to_current("}");
            } else {
                gen.writeln_to_current("Err(_) => {");
                gen.indent += 1;
                gen.emit_body(stmts);
                gen.indent -= 1;
                gen.writeln_to_current("}");
            }
        }
        CatchBody::Match(cases) => {
            if let Some(var_name) = &try_stmt.catch_var {
                let mangled = name_mangler::mangle_local(var_name);
                gen.writeln_to_current("Err(_err) => {");
                gen.indent += 1;
                gen.writeln_to_current(&format!(
                    "let mut {} = _err.value.unwrap_or(NiValue::String(Rc::new(_err.message)));",
                    mangled
                ));
                // Emit match on the caught value
                let fake_match = MatchStmt {
                    subject: Expr {
                        kind: ExprKind::Identifier(var_name.clone()),
                        span: ni_error::Span::default(),
                    },
                    cases: cases.clone(),
                };
                emit_match(gen, &fake_match);
                gen.indent -= 1;
                gen.writeln_to_current("}");
            } else {
                gen.writeln_to_current("Err(_) => {}");
            }
        }
    }

    gen.indent -= 1;
    gen.writeln_to_current("}");
}
