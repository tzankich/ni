mod class;
pub mod coroutine;
mod expr;
mod stmt;

use crate::name_mangler;
use crate::trait_def::NiCodeGen;
use ni_parser::*;
use std::collections::{HashMap, HashSet};

pub struct CCodeGen {
    /// Forward declarations
    forward_decls: String,
    /// Function/class definitions
    definitions: String,
    /// Main body code
    main_body: String,
    indent: usize,
    temp_counter: usize,
    lambda_counter: usize,
    current_class: Option<String>,
    in_function: bool,
    /// Current output target (used during function emission)
    output: String,
    /// Names of user-defined functions (for direct call resolution)
    pub(crate) known_functions: HashSet<String>,
    /// Names of user-defined classes (for constructor resolution)
    pub(crate) known_classes: HashSet<String>,
    /// Maps child class name → parent class name (for super call resolution)
    pub(crate) superclass_map: HashMap<String, String>,
}

impl Default for CCodeGen {
    fn default() -> Self {
        Self::new()
    }
}

impl CCodeGen {
    pub fn new() -> Self {
        Self {
            forward_decls: String::new(),
            definitions: String::new(),
            main_body: String::new(),
            indent: 0,
            temp_counter: 0,
            lambda_counter: 0,
            current_class: None,
            in_function: false,
            output: String::new(),
            known_functions: HashSet::new(),
            known_classes: HashSet::new(),
            superclass_map: HashMap::new(),
        }
    }

    pub fn finish(self) -> String {
        let mut result = String::new();
        result.push_str("#include \"ni_runtime.h\"\n");
        result.push_str("#include <stdio.h>\n");
        result.push_str("#include <stdlib.h>\n");
        result.push_str("#include <string.h>\n\n");

        // Forward declarations
        if !self.forward_decls.is_empty() {
            result.push_str("/* Forward declarations */\n");
            result.push_str(&self.forward_decls);
            result.push('\n');
        }

        // Definitions
        result.push_str(&self.definitions);

        // Main function
        result.push_str("NiValue ni_main(NiVm* vm) {\n");
        result.push_str(&self.main_body);
        result.push_str("    return NI_NONE;\n");
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

impl NiCodeGen for CCodeGen {
    fn begin_program(&mut self) {
        self.indent = 1;
    }

    fn end_program(&mut self) {
        self.indent = 0;
    }

    fn emit_var_decl(&mut self, decl: &VarDecl) {
        let val = self.emit_expr(&decl.initializer);
        let name = name_mangler::mangle_local(&decl.name);
        self.writeln_to_current(&format!("NiValue {} = {};", name, val));
    }

    fn emit_const_decl(&mut self, decl: &ConstDecl) {
        let val = self.emit_expr(&decl.initializer);
        let name = name_mangler::mangle_local(&decl.name);
        self.writeln_to_current(&format!("const NiValue {} = {};", name, val));
    }

    fn emit_fun_decl(&mut self, decl: &FunDecl) {
        self.known_functions.insert(decl.name.clone());
        let mangled_name = name_mangler::mangle_fun(&decl.name);

        // Forward declaration
        self.forward_decls.push_str(&format!(
            "NiValue {}(NiVm* vm, NiValue* args, int argc);\n",
            mangled_name
        ));

        let saved_output = std::mem::take(&mut self.output);
        let saved_in_function = self.in_function;
        let saved_indent = self.indent;
        self.in_function = true;
        self.indent = 1;
        self.output = String::new();

        // Parameter extraction
        for (i, param) in decl.params.iter().enumerate() {
            let pname = name_mangler::mangle_local(&param.name);
            if let Some(default) = &param.default {
                let default_val = self.emit_expr(default);
                self.writeln_to_current(&format!(
                    "NiValue {} = (argc > {}) ? args[{}] : {};",
                    pname, i, i, default_val
                ));
            } else {
                self.writeln_to_current(&format!(
                    "NiValue {} = (argc > {}) ? args[{}] : NI_NONE;",
                    pname, i, i
                ));
            }
        }

        self.emit_body(&decl.body);
        self.writeln_to_current("return NI_NONE;");

        let body = std::mem::take(&mut self.output);

        self.definitions.push_str(&format!(
            "NiValue {}(NiVm* vm, NiValue* args, int argc) {{\n",
            mangled_name
        ));
        self.definitions.push_str(&body);
        self.definitions.push_str("}\n\n");

        self.output = saved_output;
        self.in_function = saved_in_function;
        self.indent = saved_indent;

        // No runtime registration needed in C - functions are directly callable
    }

    fn emit_class_decl(&mut self, decl: &ClassDecl) {
        class::emit_class_decl(self, decl);
    }

    fn emit_enum_decl(&mut self, decl: &EnumDecl) {
        class::emit_enum_decl(self, decl);
    }

    fn emit_import(&mut self, _decl: &ImportDecl) {
        self.writeln_to_current("/* import (handled at compile time) */");
    }

    fn emit_statement(&mut self, stmt: &Statement) {
        stmt::emit_statement(self, stmt);
    }

    fn emit_expr(&mut self, expr: &Expr) -> String {
        expr::emit_expr(self, expr)
    }
}
