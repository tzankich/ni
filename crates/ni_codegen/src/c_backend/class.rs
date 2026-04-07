use super::CCodeGen;
use crate::name_mangler;
use crate::trait_def::NiCodeGen;
use ni_parser::*;

pub(crate) fn emit_class_decl(gen: &mut CCodeGen, decl: &ClassDecl) {
    let class_name = &decl.name;
    gen.known_classes.insert(class_name.clone());
    if let Some(superclass) = &decl.superclass {
        gen.superclass_map
            .insert(class_name.clone(), superclass.clone());
    }
    let saved_class = gen.current_class.take();
    gen.current_class = Some(class_name.clone());

    // Emit each method as a standalone C function
    for method in &decl.methods {
        emit_method_fn(gen, class_name, &method.fun);
    }

    // Emit static methods
    for static_method in &decl.static_methods {
        emit_static_method_fn(gen, class_name, static_method);
    }

    gen.current_class = saved_class;

    // Emit vtable struct and registration
    let register_fn = name_mangler::mangle_class(class_name);

    // Vtable struct
    gen.definitions.push_str("typedef struct {\n");
    gen.definitions.push_str("    const char* name;\n");
    for method in &decl.methods {
        gen.definitions.push_str(&format!(
            "    NiValue (*{})( NiVm*, NiValue, NiValue*, int);\n",
            method.fun.name
        ));
    }
    gen.definitions
        .push_str(&format!("}} {}_VTable;\n\n", register_fn));

    // Vtable instance
    gen.definitions.push_str(&format!(
        "static {}_VTable {}_vtable = {{\n",
        register_fn, register_fn
    ));
    gen.definitions
        .push_str(&format!("    .name = \"{}\",\n", class_name));
    for method in &decl.methods {
        let mangled = name_mangler::mangle_method(class_name, &method.fun.name);
        gen.definitions
            .push_str(&format!("    .{} = {},\n", method.fun.name, mangled));
    }
    gen.definitions.push_str("};\n\n");

    // Registration function
    gen.definitions.push_str(&format!(
        "NiValue {}_new(NiVm* vm, NiValue* args, int argc) {{\n",
        register_fn
    ));
    gen.definitions.push_str(&format!(
        "    NiValue instance = ni_new_instance(\"{}\", &{}_vtable);\n",
        class_name, register_fn
    ));
    // Set default fields
    for field in &decl.fields {
        if let Some(default) = &field.default {
            let val = match &default.kind {
                ExprKind::IntLiteral(n) => format!("ni_int({})", n),
                ExprKind::FloatLiteral(f) => format!("ni_float({})", f),
                ExprKind::StringLiteral(s) => format!("ni_string(\"{}\")", super::expr::escape_string_c(s)),
                ExprKind::BoolLiteral(b) => {
                    if *b {
                        "ni_bool(1)".into()
                    } else {
                        "ni_bool(0)".into()
                    }
                }
                ExprKind::NoneLiteral => "NI_NONE".into(),
                _ => "NI_NONE".into(),
            };
            gen.definitions.push_str(&format!(
                "    ni_set_prop(instance, \"{}\", {});\n",
                field.name, val
            ));
        } else {
            gen.definitions.push_str(&format!(
                "    ni_set_prop(instance, \"{}\", NI_NONE);\n",
                field.name
            ));
        }
    }

    // Call init if it exists
    let has_init = decl.methods.iter().any(|m| m.fun.name == "init");
    if has_init {
        let init_mangled = name_mangler::mangle_method(class_name, "init");
        gen.definitions.push_str(&format!(
            "    {}(vm, instance, args, argc);\n",
            init_mangled
        ));
    }

    gen.definitions.push_str("    return instance;\n");
    gen.definitions.push_str("}\n\n");

    // Forward declaration for constructor
    gen.forward_decls.push_str(&format!(
        "NiValue {}_new(NiVm* vm, NiValue* args, int argc);\n",
        register_fn
    ));
}

fn emit_method_fn(gen: &mut CCodeGen, class_name: &str, fun: &FunDecl) {
    let mangled = name_mangler::mangle_method(class_name, &fun.name);

    // Forward declaration
    gen.forward_decls.push_str(&format!(
        "NiValue {}(NiVm* vm, NiValue self_val, NiValue* args, int argc);\n",
        mangled
    ));

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

    gen.emit_body(&fun.body);
    gen.writeln_to_current("return NI_NONE;");

    let body = std::mem::take(&mut gen.output);

    gen.definitions.push_str(&format!(
        "NiValue {}(NiVm* vm, NiValue self_val, NiValue* args, int argc) {{\n",
        mangled
    ));
    gen.definitions.push_str(&body);
    gen.definitions.push_str("}\n\n");

    gen.output = saved_output;
    gen.in_function = saved_in_function;
    gen.indent = saved_indent;
}

fn emit_static_method_fn(gen: &mut CCodeGen, class_name: &str, fun: &FunDecl) {
    let mangled = name_mangler::mangle_static_method(class_name, &fun.name);

    gen.forward_decls.push_str(&format!(
        "NiValue {}(NiVm* vm, NiValue* args, int argc);\n",
        mangled
    ));

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

    gen.emit_body(&fun.body);
    gen.writeln_to_current("return NI_NONE;");

    let body = std::mem::take(&mut gen.output);

    gen.definitions.push_str(&format!(
        "NiValue {}(NiVm* vm, NiValue* args, int argc) {{\n",
        mangled
    ));
    gen.definitions.push_str(&body);
    gen.definitions.push_str("}\n\n");

    gen.output = saved_output;
    gen.in_function = saved_in_function;
    gen.indent = saved_indent;
}

pub(crate) fn emit_enum_decl(gen: &mut CCodeGen, decl: &EnumDecl) {
    let enum_name = &decl.name;
    // Emit as a set of constant definitions
    gen.writeln_to_current(&format!("/* enum {} */", enum_name));
    for (i, variant) in decl.variants.iter().enumerate() {
        let mangled_name = format!("{}_{}", name_mangler::mangle_enum(enum_name), variant.name);
        if let Some(value_expr) = &variant.value {
            let val = gen.emit_expr(value_expr);
            gen.writeln_to_current(&format!("NiValue {} = {};", mangled_name, val));
        } else {
            gen.writeln_to_current(&format!("NiValue {} = ni_int({});", mangled_name, i));
        }
    }
}
