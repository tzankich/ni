mod class;
pub mod coroutine;
mod expr;
mod stmt;

use crate::name_mangler;
use crate::trait_def::NiCodeGen;
use ni_parser::*;
use std::collections::{HashMap, HashSet};

pub struct RustCodeGen {
    output: String,
    indent: usize,
    /// Top-level statements collected for ni_main
    main_body: String,
    /// Functions/classes emitted before main
    declarations: String,
    /// Counter for generating unique temp variable names
    temp_counter: usize,
    /// Counter for lambda names
    lambda_counter: usize,
    /// Current class context (for self references)
    current_class: Option<String>,
    /// Whether we're inside a function/method body (vs top-level)
    in_function: bool,
    /// Names of user-defined functions (for direct call resolution)
    known_functions: HashSet<String>,
    /// Names of user-defined classes (for constructor resolution)
    known_classes: HashSet<String>,
    /// Names of user-defined enums
    known_enums: HashSet<String>,
    /// Maps child class name → parent class name (for super call resolution)
    pub(crate) superclass_map: HashMap<String, String>,
}

impl Default for RustCodeGen {
    fn default() -> Self {
        Self::new()
    }
}

impl RustCodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            main_body: String::new(),
            declarations: String::new(),
            temp_counter: 0,
            lambda_counter: 0,
            current_class: None,
            in_function: false,
            known_functions: HashSet::new(),
            known_classes: HashSet::new(),
            known_enums: HashSet::new(),
            superclass_map: HashMap::new(),
        }
    }

    pub fn finish(self) -> String {
        let mut result = String::new();
        result.push_str("#![allow(unused_variables, unused_mut, unused_imports, unreachable_code, unused_parens, non_snake_case, unused_labels)]\n");
        result.push_str("use ni_runtime::prelude::*;\n");
        result.push_str("use std::rc::Rc;\n");
        result.push_str("use std::cell::RefCell;\n\n");

        result.push_str(&self.declarations);

        // Emit ni_main
        result.push_str("pub fn ni_main(vm: &mut dyn NiVm) -> NiResult<NiValue> {\n");
        result.push_str(&self.main_body);
        result.push_str("    Ok(NiValue::None)\n");
        result.push_str("}\n");

        result
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn fresh_temp(&mut self) -> String {
        self.temp_counter += 1;
        format!("_tmp_{}", self.temp_counter)
    }

    #[allow(dead_code)]
    fn write_to_current(&mut self, s: &str) {
        if self.in_function {
            self.output.push_str(s);
        } else {
            self.main_body.push_str(s);
        }
    }

    fn writeln_to_current(&mut self, s: &str) {
        let indent = self.indent_str();
        if self.in_function {
            self.output.push_str(&indent);
            self.output.push_str(s);
            self.output.push('\n');
        } else {
            self.main_body.push_str(&indent);
            self.main_body.push_str(s);
            self.main_body.push('\n');
        }
    }

    fn emit_body(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.emit_statement(stmt);
        }
    }
}

impl NiCodeGen for RustCodeGen {
    fn begin_program(&mut self) {
        self.indent = 1; // Start indented inside ni_main
    }

    fn end_program(&mut self) {
        self.indent = 0;
    }

    fn emit_var_decl(&mut self, decl: &VarDecl) {
        if self.in_function {
            let val = self.emit_expr(&decl.initializer);
            let name = name_mangler::mangle_local(&decl.name);
            self.writeln_to_current(&format!("let mut {} = {};", name, val));
        } else {
            let val = self.emit_expr(&decl.initializer);
            let name = name_mangler::mangle_local(&decl.name);
            self.writeln_to_current(&format!("let mut {} = {};", name, val));
        }
    }

    fn emit_const_decl(&mut self, decl: &ConstDecl) {
        let val = self.emit_expr(&decl.initializer);
        let name = name_mangler::mangle_local(&decl.name);
        self.writeln_to_current(&format!("let {} = {};", name, val));
    }

    fn emit_fun_decl(&mut self, decl: &FunDecl) {
        let is_nested = self.in_function;

        // Register this function name so calls can be resolved directly
        self.known_functions.insert(decl.name.clone());

        // Check for captured variables if this is a nested function
        let param_names: HashSet<String> = decl.params.iter().map(|p| p.name.clone()).collect();
        let free_vars = if is_nested {
            expr::collect_free_vars_stmts(
                &decl.body,
                &param_names,
                &self.known_functions,
                &self.known_classes,
                &self.known_enums,
            )
        } else {
            vec![]
        };

        let saved_output = std::mem::take(&mut self.output);
        let saved_in_function = self.in_function;
        let saved_indent = self.indent;
        self.in_function = true;
        self.indent = 1;

        let mangled_name = name_mangler::mangle_fun(&decl.name);

        // Build parameter extraction
        self.output = String::new();
        for (i, param) in decl.params.iter().enumerate() {
            let pname = name_mangler::mangle_local(&param.name);
            if let Some(default) = &param.default {
                let default_val = self.emit_expr(default);
                self.writeln_to_current(&format!(
                    "let mut {} = args.get({}).cloned().unwrap_or({});",
                    pname, i, default_val
                ));
            } else {
                self.writeln_to_current(&format!(
                    "let mut {} = args.get({}).cloned().unwrap_or(NiValue::None);",
                    pname, i
                ));
            }
        }

        // Emit body
        self.emit_body(&decl.body);
        self.writeln_to_current("Ok(NiValue::None)");

        let body = std::mem::take(&mut self.output);

        self.output = saved_output;
        self.in_function = saved_in_function;
        self.indent = saved_indent;

        if is_nested && !free_vars.is_empty() {
            // Nested function with captures -- emit as a local Rust move closure
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
            self.writeln_to_current(&format!(
                "{captures} let mut {local} = NiValue::Function(NiFunctionRef {{ name: \"{name}\".into(), func: Rc::new(move |vm, args| {{ {rebinds}{body}}} ) }});",
                captures = captured_clones.join(" "),
                local = name_mangler::mangle_local(&decl.name),
                name = decl.name,
                rebinds = capture_rebinds.join(""),
                body = body.trim(),
            ));
        } else {
            // Top-level or no captures: emit as standalone function
            self.declarations.push_str(&format!(
                "pub fn {}(vm: &mut dyn NiVm, args: &[NiValue]) -> NiResult<NiValue> {{\n",
                mangled_name
            ));
            self.declarations.push_str(&body);
            self.declarations.push_str("}\n\n");

            if is_nested {
                // Nested but no captures: create a local binding referencing the standalone fn
                self.writeln_to_current(&format!(
                    "let mut {} = NiValue::Function(NiFunctionRef {{ name: \"{}\".into(), func: Rc::new(|vm, args| {}(vm, args)) }});",
                    name_mangler::mangle_local(&decl.name), decl.name, mangled_name
                ));
            }
        }
    }

    fn emit_class_decl(&mut self, decl: &ClassDecl) {
        class::emit_class_decl(self, decl);
    }

    fn emit_enum_decl(&mut self, decl: &EnumDecl) {
        class::emit_enum_decl(self, decl);
    }

    fn emit_import(&mut self, _decl: &ImportDecl) {
        self.writeln_to_current("// import (handled at compile time)");
    }

    fn emit_statement(&mut self, stmt: &Statement) {
        stmt::emit_statement(self, stmt);
    }

    fn emit_expr(&mut self, expr: &Expr) -> String {
        expr::emit_expr(self, expr)
    }
}
