use super::RustCodeGen;
use crate::name_mangler;
use crate::trait_def::NiCodeGen;
use ni_parser::*;

pub(crate) fn emit_class_decl(gen: &mut RustCodeGen, decl: &ClassDecl) {
    let class_name = &decl.name;
    gen.known_classes.insert(class_name.clone());
    if let Some(superclass) = &decl.superclass {
        gen.superclass_map
            .insert(class_name.clone(), superclass.clone());
    }
    let saved_class = gen.current_class.take();
    gen.current_class = Some(class_name.clone());

    // Emit each method as a standalone function
    for method in &decl.methods {
        emit_method_fn(gen, class_name, &method.fun);
    }

    // Emit static methods
    for static_method in &decl.static_methods {
        emit_static_method_fn(gen, class_name, static_method);
    }

    gen.current_class = saved_class;

    // Emit a registration function that builds the NiClassDef
    let register_fn = name_mangler::mangle_class(class_name);
    gen.declarations.push_str(&format!(
        "pub fn {}_register() -> NiClassDef {{\n",
        register_fn
    ));
    gen.declarations.push_str(&format!(
        "    let mut class = NiClassDef::new(\"{}\");\n",
        class_name
    ));

    // Default fields
    for field in &decl.fields {
        if let Some(default) = &field.default {
            // We need to emit field defaults as simple values
            // For now, use None as placeholder for complex expressions
            match &default.kind {
                ExprKind::IntLiteral(n) => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::Int({}));\n",
                        field.name, n
                    ));
                }
                ExprKind::FloatLiteral(f) => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::Float({}));\n",
                        field.name, f
                    ));
                }
                ExprKind::StringLiteral(s) => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::String(Rc::new(\"{}\".to_string())));\n",
                        field.name, super::expr::escape_string(s)
                    ));
                }
                ExprKind::BoolLiteral(b) => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::Bool({}));\n",
                        field.name, b
                    ));
                }
                ExprKind::NoneLiteral => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::None);\n",
                        field.name
                    ));
                }
                _ => {
                    gen.declarations.push_str(&format!(
                        "    class.default_fields.insert(\"{}\".to_string(), NiValue::None);\n",
                        field.name
                    ));
                }
            }
        } else {
            gen.declarations.push_str(&format!(
                "    class.default_fields.insert(\"{}\".to_string(), NiValue::None);\n",
                field.name
            ));
        }
    }

    // Register methods
    for method in &decl.methods {
        let mangled = name_mangler::mangle_method(class_name, &method.fun.name);
        gen.declarations.push_str(&format!(
            "    class.methods.insert(\"{}\".to_string(), Rc::new(|vm, self_val, args| {}(vm, self_val, args)));\n",
            method.fun.name, mangled
        ));
    }

    // Superclass -- link parent class for method chain lookup
    if let Some(superclass) = &decl.superclass {
        let parent_register = name_mangler::mangle_class(superclass);
        gen.declarations.push_str(&format!(
            "    class.superclass = Some(Rc::new({}_register()));\n",
            parent_register
        ));
    }

    gen.declarations.push_str("    class\n");
    gen.declarations.push_str("}\n\n");

    // In main body, register the class
    gen.writeln_to_current(&format!(
        "let mut {} = NiValue::Class(Rc::new({}_register()));",
        name_mangler::mangle_local(class_name),
        register_fn
    ));
}

fn emit_method_fn(gen: &mut RustCodeGen, class_name: &str, fun: &FunDecl) {
    let mangled = name_mangler::mangle_method(class_name, &fun.name);

    let saved_output = std::mem::take(&mut gen.output);
    let saved_in_function = gen.in_function;
    let saved_indent = gen.indent;
    gen.in_function = true;
    gen.indent = 1;
    gen.output = String::new();

    // Extract parameters
    for (i, param) in fun.params.iter().enumerate() {
        let pname = name_mangler::mangle_local(&param.name);
        if let Some(default) = &param.default {
            let default_val = gen.emit_expr(default);
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

    gen.emit_body(&fun.body);
    gen.writeln_to_current("Ok(NiValue::None)");

    let body = std::mem::take(&mut gen.output);

    gen.declarations.push_str(&format!(
        "pub fn {}(vm: &mut dyn NiVm, self_val: &NiValue, args: &[NiValue]) -> NiResult<NiValue> {{\n",
        mangled
    ));
    gen.declarations.push_str(&body);
    gen.declarations.push_str("}\n\n");

    gen.output = saved_output;
    gen.in_function = saved_in_function;
    gen.indent = saved_indent;
}

fn emit_static_method_fn(gen: &mut RustCodeGen, class_name: &str, fun: &FunDecl) {
    let mangled = name_mangler::mangle_static_method(class_name, &fun.name);

    let saved_output = std::mem::take(&mut gen.output);
    let saved_in_function = gen.in_function;
    let saved_indent = gen.indent;
    gen.in_function = true;
    gen.indent = 1;
    gen.output = String::new();

    for (i, param) in fun.params.iter().enumerate() {
        let pname = name_mangler::mangle_local(&param.name);
        if let Some(default) = &param.default {
            let default_val = gen.emit_expr(default);
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

    gen.emit_body(&fun.body);
    gen.writeln_to_current("Ok(NiValue::None)");

    let body = std::mem::take(&mut gen.output);

    gen.declarations.push_str(&format!(
        "pub fn {}(vm: &mut dyn NiVm, args: &[NiValue]) -> NiResult<NiValue> {{\n",
        mangled
    ));
    gen.declarations.push_str(&body);
    gen.declarations.push_str("}\n\n");

    gen.output = saved_output;
    gen.in_function = saved_in_function;
    gen.indent = saved_indent;
}

pub(crate) fn emit_enum_decl(gen: &mut RustCodeGen, decl: &EnumDecl) {
    let enum_name = &decl.name;
    gen.known_enums.insert(enum_name.clone());
    gen.writeln_to_current(&format!(
        "let mut {} = {{",
        name_mangler::mangle_local(enum_name)
    ));
    gen.indent += 1;
    gen.writeln_to_current("let mut variants = std::collections::HashMap::new();");

    for (i, variant) in decl.variants.iter().enumerate() {
        if let Some(value_expr) = &variant.value {
            let val = gen.emit_expr(value_expr);
            gen.writeln_to_current(&format!(
                "variants.insert(\"{}\".to_string(), {});",
                variant.name, val
            ));
        } else {
            gen.writeln_to_current(&format!(
                "variants.insert(\"{}\".to_string(), NiValue::Int({}));",
                variant.name, i
            ));
        }
    }

    gen.writeln_to_current(&format!(
        "NiValue::Enum(Rc::new(NiEnumDef {{ name: \"{}\".to_string(), variants }}))",
        enum_name
    ));
    gen.indent -= 1;
    gen.writeln_to_current("};");
}
