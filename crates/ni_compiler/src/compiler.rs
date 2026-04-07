use std::collections::HashSet;
use std::path::PathBuf;

use ni_error::{NiError, NiResult};
use ni_parser::*;
use ni_vm::chunk::{Chunk, ExceptionEntry, OpCode};
use ni_vm::debug::LocalVarEntry;
use ni_vm::gc::{GcHeap, GcRef};
use ni_vm::intern::InternTable;
use ni_vm::object::{NiClass, NiClosure, NiEnum, NiFunction, NiInstance, NiObject};
use ni_vm::value::Value;

#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: usize,
    is_captured: bool,
    is_mutable: bool,
}

#[derive(Debug, Clone)]
struct UpvalueInfo {
    index: u8,
    is_local: bool,
    name: String,
}

#[derive(Debug, Clone)]
struct LoopContext {
    start: usize,
    break_jumps: Vec<usize>,
    depth: usize,
}

struct FunctionCompiler {
    function_name: String,
    chunk: Chunk,
    locals: Vec<Local>,
    upvalues: Vec<UpvalueInfo>,
    scope_depth: usize,
    loop_stack: Vec<LoopContext>,
    arity: u8,
    default_count: u8,
    exception_table: Vec<ExceptionEntry>,
    local_var_table: Vec<LocalVarEntry>,
    docstring: Option<String>,
}

impl FunctionCompiler {
    fn new(name: &str, arity: u8) -> Self {
        let mut fc = Self {
            function_name: name.to_string(),
            chunk: Chunk::new(),
            locals: Vec::new(),
            upvalues: Vec::new(),
            scope_depth: 0,
            loop_stack: Vec::new(),
            arity,
            default_count: 0,
            exception_table: Vec::new(),
            local_var_table: Vec::new(),
            docstring: None,
        };
        // Reserve slot 0 for the function/self
        fc.locals.push(Local {
            name: String::new(),
            depth: 0,
            is_captured: false,
            is_mutable: false,
        });
        // Slot 0 entry in var table
        fc.local_var_table.push(LocalVarEntry {
            slot: 0,
            name: String::new(),
            start_offset: 0,
            end_offset: usize::MAX,
        });
        fc
    }

    fn emit(&mut self, op: OpCode, line: usize) {
        self.chunk.write_op(op, line);
    }

    fn emit_byte(&mut self, byte: u8, line: usize) {
        self.chunk.write(byte, line);
    }

    fn emit_u16(&mut self, value: u16, line: usize) {
        self.chunk.write_u16(value, line);
    }

    fn emit_constant(&mut self, value: Value, line: usize) {
        let idx = self.chunk.add_constant(value);
        self.emit(OpCode::Constant, line);
        self.emit_u16(idx, line);
    }

    fn emit_jump(&mut self, op: OpCode, line: usize) -> usize {
        self.emit(op, line);
        let offset = self.chunk.current_offset();
        self.emit_u16(0, line); // placeholder
        offset
    }

    fn patch_jump(&mut self, offset: usize) {
        self.chunk.patch_jump(offset);
    }

    fn emit_loop(&mut self, loop_start: usize, line: usize) {
        self.emit(OpCode::Loop, line);
        let offset = self.chunk.current_offset() - loop_start + 2;
        self.emit_u16(offset as u16, line);
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self, line: usize) {
        self.scope_depth -= 1;
        while let Some(local) = self.locals.last() {
            if local.depth <= self.scope_depth {
                break;
            }
            let slot = (self.locals.len() - 1) as u8;
            if local.is_captured {
                self.emit(OpCode::CloseUpvalue, line);
            } else {
                self.emit(OpCode::Pop, line);
            }
            // Record end_offset for this local's var table entry
            let end_offset = self.chunk.current_offset();
            for entry in self.local_var_table.iter_mut().rev() {
                if entry.slot == slot && entry.end_offset == usize::MAX {
                    entry.end_offset = end_offset;
                    break;
                }
            }
            self.locals.pop();
        }
    }

    fn add_local(&mut self, name: &str, is_mutable: bool) -> u8 {
        let slot = self.locals.len() as u8;
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
            is_captured: false,
            is_mutable,
        });
        let start_offset = self.chunk.current_offset();
        self.local_var_table.push(LocalVarEntry {
            slot,
            name: name.to_string(),
            start_offset,
            end_offset: usize::MAX,
        });
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<u8> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u8);
            }
        }
        None
    }
}

pub struct Compiler<'a> {
    heap: &'a mut GcHeap,
    interner: &'a mut InternTable,
    compilers: Vec<FunctionCompiler>,
    class_depth: usize, // track if we're inside a class (for self/super)
    source_root: Option<PathBuf>,
    import_stack: HashSet<String>, // for circular import detection
    spec_mode: bool,
    immutable_globals: HashSet<String>,
    pub allowed_modules: Option<Vec<String>>,
}

impl<'a> Compiler<'a> {
    pub fn new(heap: &'a mut GcHeap, interner: &'a mut InternTable) -> Self {
        let top = FunctionCompiler::new("<script>", 0);
        Self {
            heap,
            interner,
            compilers: vec![top],
            class_depth: 0,
            source_root: None,
            import_stack: HashSet::new(),
            spec_mode: false,
            immutable_globals: HashSet::new(),
            allowed_modules: None,
        }
    }

    pub fn with_allowed_modules(mut self, modules: Option<Vec<String>>) -> Self {
        self.allowed_modules = modules;
        self
    }

    pub fn with_source_root(mut self, root: PathBuf) -> Self {
        self.source_root = Some(root);
        self
    }

    pub fn with_spec_mode(mut self, spec_mode: bool) -> Self {
        self.spec_mode = spec_mode;
        self
    }

    fn current(&mut self) -> &mut FunctionCompiler {
        self.compilers.last_mut().unwrap()
    }

    fn current_ref(&self) -> &FunctionCompiler {
        self.compilers.last().unwrap()
    }

    /// Allocate an interned string constant: interns the string and adds InternedString to current chunk.
    fn string_const(&mut self, s: &str) -> u16 {
        let id = self.interner.intern(s);
        let r = self.heap.alloc(NiObject::InternedString(id));
        self.compilers
            .last_mut()
            .unwrap()
            .chunk
            .add_constant(Value::Object(r))
    }

    pub fn compile(&mut self, program: &Program) -> NiResult<GcRef> {
        // Emit module docstring as __doc__ global
        if let Some(ref ds) = program.docstring {
            let ds_idx = self.string_const(ds);
            self.current().emit(OpCode::Constant, 0);
            self.current().emit_u16(ds_idx, 0);
            let name_idx = self.string_const("__doc__");
            self.current().emit(OpCode::DefineGlobal, 0);
            self.current().emit_u16(name_idx, 0);
        }

        for decl in &program.declarations {
            self.compile_declaration(decl)?;
        }
        self.current().emit(OpCode::None, 0);
        self.current().emit(OpCode::Return, 0);
        self.finish_function()
    }

    fn finish_function(&mut self) -> NiResult<GcRef> {
        let mut fc = self.compilers.pop().unwrap();
        // Finalize any remaining open entries with the final chunk offset
        let final_offset = fc.chunk.current_offset();
        for entry in &mut fc.local_var_table {
            if entry.end_offset == usize::MAX {
                entry.end_offset = final_offset;
            }
        }
        let upvalue_names = fc.upvalues.iter().map(|uv| uv.name.clone()).collect();
        let func = NiFunction {
            name: fc.function_name,
            arity: fc.arity,
            default_count: fc.default_count,
            chunk: fc.chunk,
            upvalue_count: fc.upvalues.len() as u8,
            exception_table: fc.exception_table,
            local_var_table: fc.local_var_table,
            upvalue_names,
            docstring: fc.docstring,
        };
        let fn_ref = self.heap.alloc(NiObject::Function(func));
        let closure = NiObject::Closure(NiClosure {
            function: fn_ref,
            upvalues: Vec::new(),
        });
        let closure_ref = self.heap.alloc(closure);

        if !self.compilers.is_empty() {
            // Emit closure instruction in the enclosing compiler
            let fn_const = self.current().chunk.add_constant(Value::Object(fn_ref));
            let line = 0;
            self.current().emit(OpCode::Closure, line);
            self.current().emit_u16(fn_const, line);

            // Emit upvalue descriptors
            for uv in &fc.upvalues {
                self.current()
                    .emit_byte(if uv.is_local { 1 } else { 0 }, line);
                self.current().emit_byte(uv.index, line);
            }
        }

        Ok(closure_ref)
    }

    fn compile_declaration(&mut self, decl: &Declaration) -> NiResult<()> {
        let line = decl.span.line;
        match &decl.kind {
            DeclKind::Var(var_decl) => self.compile_var_decl(var_decl, line),
            DeclKind::Const(const_decl) => self.compile_const_decl(const_decl, line),
            DeclKind::Fun(fun_decl) => self.compile_fun_decl(fun_decl, line),
            DeclKind::Class(class_decl) => self.compile_class_decl(class_decl, line),
            DeclKind::Enum(enum_decl) => self.compile_enum_decl(enum_decl, line),
            DeclKind::Import(import_decl) => self.compile_import(import_decl, line),
            DeclKind::Spec(spec_decl) => {
                if self.spec_mode {
                    self.compile_spec_decl(spec_decl, line)?;
                }
                Ok(())
            }
            DeclKind::Statement(stmt) => self.compile_statement(stmt),
        }
    }

    fn compile_var_decl(&mut self, var_decl: &VarDecl, line: usize) -> NiResult<()> {
        self.compile_expr(&var_decl.initializer)?;

        if self.current_ref().scope_depth > 0 {
            // Local variable -- mutable
            self.current().add_local(&var_decl.name, true);
        } else {
            // Global variable -- mutable (not tracked in immutable_globals)
            let name_idx = self.string_const(&var_decl.name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(name_idx, line);
        }
        Ok(())
    }

    fn compile_const_decl(&mut self, const_decl: &ConstDecl, line: usize) -> NiResult<()> {
        self.compile_expr(&const_decl.initializer)?;

        if self.current_ref().scope_depth > 0 {
            // Local -- immutable
            self.current().add_local(&const_decl.name, false);
        } else {
            // Global -- immutable
            self.immutable_globals.insert(const_decl.name.clone());
            let name_idx = self.string_const(&const_decl.name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(name_idx, line);
        }
        Ok(())
    }

    fn compile_fun_decl(&mut self, fun_decl: &FunDecl, line: usize) -> NiResult<()> {
        // For top-level: define global, for nested: define local
        if self.current_ref().scope_depth > 0 {
            self.current().add_local(&fun_decl.name, false);
        }

        self.compile_function(fun_decl, line)?;

        if self.current_ref().scope_depth == 0 {
            self.immutable_globals.insert(fun_decl.name.clone());
            let name_idx = self.string_const(&fun_decl.name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(name_idx, line);
        }
        Ok(())
    }

    fn compile_spec_decl(&mut self, spec_decl: &ni_parser::SpecDecl, line: usize) -> NiResult<()> {
        let global_name = format!("spec:{}", spec_decl.name);

        if spec_decl.sections.is_empty() {
            // Flat spec: zero-arity function (backward compatible)
            let fun_decl = FunDecl {
                name: global_name.clone(),
                params: vec![],
                return_type: None,
                body: spec_decl.body.clone(),
                docstring: None,
            };
            self.compile_function(&fun_decl, line)?;
        } else {
            // Structured spec: compile as 1-arity (or 2-arity with each) function
            // taking path_index parameter.
            //
            // First, enumerate all root-to-leaf paths.
            let paths = enumerate_spec_paths(&spec_decl.sections);
            let path_count = paths.len();

            // Store path metadata as a constant list of label trails
            // Format: list of strings like "given:X > when:Y > then:Z"
            let mut path_labels = Vec::new();
            for path in &paths {
                let trail: Vec<String> = path
                    .iter()
                    .map(|s| {
                        let kind_str = match s.kind {
                            SpecSectionKind::Given => "given",
                            SpecSectionKind::When => "when",
                            SpecSectionKind::Then => "then",
                        };
                        format!("{} {}", kind_str, s.label)
                    })
                    .collect();
                path_labels.push(trail.join(" > "));
            }

            let has_each = spec_decl.each.is_some();
            let arity: u8 = if has_each { 2 } else { 1 };

            let mut fc = FunctionCompiler::new(&global_name, arity);
            fc.scope_depth = 1;

            // Add parameter(s) as locals
            fc.add_local("__path_index__", false);
            if has_each {
                fc.add_local("__row_index__", false);
            }

            self.compilers.push(fc);

            // If each clause, compile the row destructuring
            if let Some(each) = &spec_decl.each {
                // Build the list of row maps as a constant
                for item in &each.items {
                    self.compile_expr(item)?;
                }
                self.current().emit(OpCode::BuildList, line);
                self.current().emit_u16(each.items.len() as u16, line);
                let list_slot = self.current().add_local("__each_rows__", false);

                // Get current row: __each_rows__[__row_index__]
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(list_slot, line);
                self.current().emit(OpCode::GetLocal, line);
                let row_idx_slot = self.current().resolve_local("__row_index__").unwrap();
                self.current().emit_byte(row_idx_slot, line);
                self.current().emit(OpCode::GetIndex, line);
                let _row_slot = self.current().add_local("__row__", false);
            }

            // Compile any top-level body statements (setup code before BDD sections)
            for stmt in &spec_decl.body {
                self.compile_statement(stmt)?;
            }

            // Compile path dispatch: if/elif chain on __path_index__
            let path_idx_slot = self.current().resolve_local("__path_index__").unwrap();

            let mut end_jumps = Vec::new();
            for (i, path) in paths.iter().enumerate() {
                // if __path_index__ == i:
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(path_idx_slot, line);
                self.current().emit_constant(Value::Int(i as i64), line);
                self.current().emit(OpCode::Equal, line);
                let skip_jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line); // pop condition

                // Execute path: each section's body in order (root to leaf)
                self.current().begin_scope();
                for section in path {
                    for stmt in &section.body {
                        self.compile_statement(stmt)?;
                    }
                }
                self.current().end_scope(line);

                end_jumps.push(self.current().emit_jump(OpCode::Jump, line));
                self.current().patch_jump(skip_jump);
                self.current().emit(OpCode::Pop, line); // pop condition (false path)
            }

            // Patch all end jumps
            for jump in end_jumps {
                self.current().patch_jump(jump);
            }

            // Implicit return none
            self.current().emit(OpCode::None, 0);
            self.current().emit(OpCode::Return, 0);

            self.finish_function()?;

            // Store metadata: path count and labels as a list constant
            // We'll store it as global "spec_meta:{name}" = [path_count, label1, label2, ...]
            let meta_name = format!("spec_meta:{}", spec_decl.name);
            let mut meta_values = vec![Value::Int(path_count as i64)];
            for label in &path_labels {
                let r = self.heap.alloc(NiObject::String(label.clone()));
                meta_values.push(Value::Object(r));
            }
            let has_each_val = if has_each {
                Value::Bool(true)
            } else {
                Value::Bool(false)
            };
            meta_values.push(has_each_val);
            if let Some(each) = &spec_decl.each {
                meta_values.push(Value::Int(each.items.len() as i64));
            }
            let meta_list = self.heap.alloc(NiObject::List(meta_values));
            self.current().emit_constant(Value::Object(meta_list), line);
            let meta_idx = self.string_const(&meta_name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(meta_idx, line);
        }

        // Define spec global
        let name_idx = self.string_const(&global_name);
        self.current().emit(OpCode::DefineGlobal, line);
        self.current().emit_u16(name_idx, line);
        Ok(())
    }

    fn compile_function(&mut self, fun_decl: &FunDecl, _line: usize) -> NiResult<()> {
        let arity = fun_decl.params.len() as u8;
        let default_count = fun_decl
            .params
            .iter()
            .filter(|p| p.default.is_some())
            .count() as u8;

        let mut fc = FunctionCompiler::new(&fun_decl.name, arity);
        fc.default_count = default_count;
        fc.scope_depth = 1; // function body is already a scope
        fc.docstring = fun_decl.docstring.clone();

        // Add parameters as locals -- params are mutable
        for param in &fun_decl.params {
            fc.add_local(&param.name, true);
        }

        self.compilers.push(fc);

        // Handle default parameters
        for param in &fun_decl.params {
            if let Some(default) = &param.default {
                let slot = self.current().resolve_local(&param.name).unwrap();
                let line = default.span.line;

                // If param is None (wasn't provided), use default
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(slot, line);
                self.current().emit(OpCode::None, line);
                self.current().emit(OpCode::Equal, line);
                let jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line);

                self.compile_expr(default)?;
                self.current().emit(OpCode::SetLocal, line);
                self.current().emit_byte(slot, line);
                self.current().emit(OpCode::Pop, line);

                let end_jump = self.current().emit_jump(OpCode::Jump, line);
                self.current().patch_jump(jump);
                self.current().emit(OpCode::Pop, line);
                self.current().patch_jump(end_jump);
            }
        }

        // Compile function body
        for stmt in &fun_decl.body {
            self.compile_statement(stmt)?;
        }

        // Implicit return none
        self.current().emit(OpCode::None, 0);
        self.current().emit(OpCode::Return, 0);

        self.finish_function()?;
        Ok(())
    }

    fn compile_class_decl(&mut self, class_decl: &ClassDecl, line: usize) -> NiResult<()> {
        let name_idx = self.string_const(&class_decl.name);

        if self.current_ref().scope_depth > 0 {
            self.current().add_local(&class_decl.name, false);
        }

        self.current().emit(OpCode::Class, line);
        self.current().emit_u16(name_idx, line);

        if self.current_ref().scope_depth == 0 {
            self.immutable_globals.insert(class_decl.name.clone());
            let name_idx2 = self.string_const(&class_decl.name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(name_idx2, line);
        }

        // Handle superclass
        if let Some(superclass_name) = &class_decl.superclass {
            // Stack: push subclass, then superclass on top
            // Inherit pops superclass, peeks subclass (subclass stays on stack)
            self.compile_variable_load(&class_decl.name, line)?;
            self.compile_variable_load(superclass_name, line)?;
            self.current().emit(OpCode::Inherit, line);
            // Pop the subclass that Inherit left on stack
            self.current().emit(OpCode::Pop, line);
        }

        self.class_depth += 1;

        // Load class for method definitions
        self.compile_variable_load(&class_decl.name, line)?;

        // Set docstring on class if present
        if let Some(ref ds) = class_decl.docstring {
            let ds_const = self.string_const(ds);
            self.current().emit(OpCode::Constant, line);
            self.current().emit_u16(ds_const, line);
            self.current().emit(OpCode::SetDocstring, line);
        }

        // Compile fields with defaults
        for field in &class_decl.fields {
            if let Some(default) = &field.default {
                // Store as class field default
                let field_name = self.string_const(&field.name);
                self.current().emit(OpCode::Dup, line);
                self.compile_expr(default)?;
                self.current().emit(OpCode::SetProperty, line);
                self.current().emit_u16(field_name, line);
                self.current().emit(OpCode::Pop, line);
            }
        }

        // Compile methods
        for method in &class_decl.methods {
            self.compile_method(&method.fun, line)?;
        }

        // Compile static methods
        for static_method in &class_decl.static_methods {
            self.compile_method(static_method, line)?;
        }

        self.current().emit(OpCode::Pop, line); // pop the class
        self.class_depth -= 1;

        Ok(())
    }

    fn compile_method(&mut self, fun_decl: &FunDecl, line: usize) -> NiResult<()> {
        let arity = fun_decl.params.len() as u8;
        let default_count = fun_decl
            .params
            .iter()
            .filter(|p| p.default.is_some())
            .count() as u8;

        let mut fc = FunctionCompiler::new(&fun_decl.name, arity);
        fc.default_count = default_count;
        fc.scope_depth = 1;
        fc.docstring = fun_decl.docstring.clone();

        // Slot 0 is 'self' (or '' for the function)
        fc.locals[0].name = "self".to_string();

        for param in &fun_decl.params {
            fc.add_local(&param.name, true);
        }

        self.compilers.push(fc);

        // Handle default params
        for param in &fun_decl.params {
            if let Some(default) = &param.default {
                let slot = self.current().resolve_local(&param.name).unwrap();
                let line = default.span.line;
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(slot, line);
                self.current().emit(OpCode::None, line);
                self.current().emit(OpCode::Equal, line);
                let jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line);
                self.compile_expr(default)?;
                self.current().emit(OpCode::SetLocal, line);
                self.current().emit_byte(slot, line);
                self.current().emit(OpCode::Pop, line);
                let end_jump = self.current().emit_jump(OpCode::Jump, line);
                self.current().patch_jump(jump);
                self.current().emit(OpCode::Pop, line);
                self.current().patch_jump(end_jump);
            }
        }

        for stmt in &fun_decl.body {
            self.compile_statement(stmt)?;
        }

        // init methods return self (slot 0), other methods return none
        if fun_decl.name == "init" {
            self.current().emit(OpCode::GetLocal, 0);
            self.current().emit_byte(0, 0);
        } else {
            self.current().emit(OpCode::None, 0);
        }
        self.current().emit(OpCode::Return, 0);

        self.finish_function()?;

        // Add method to class
        let method_name = self.string_const(&fun_decl.name);
        self.current().emit(OpCode::Method, line);
        self.current().emit_u16(method_name, line);

        Ok(())
    }

    fn compile_enum_decl(&mut self, enum_decl: &EnumDecl, line: usize) -> NiResult<()> {
        // Create an enum object and define it as a global
        // For simplicity, compile it as creating a map-like object
        let name_idx = self.string_const(&enum_decl.name);

        // Build enum as a class with variant fields
        self.current().emit(OpCode::Class, line);
        self.current().emit_u16(name_idx, line);

        // Set variant values
        for (i, variant) in enum_decl.variants.iter().enumerate() {
            let variant_name = self.string_const(&variant.name);
            self.current().emit(OpCode::Dup, line);
            if let Some(val_expr) = &variant.value {
                self.compile_expr(val_expr)?;
            } else {
                self.current().emit_constant(Value::Int(i as i64), line);
            }
            self.current().emit(OpCode::SetProperty, line);
            self.current().emit_u16(variant_name, line);
            self.current().emit(OpCode::Pop, line);
        }

        if self.current_ref().scope_depth > 0 {
            self.current().add_local(&enum_decl.name, false);
        } else {
            self.immutable_globals.insert(enum_decl.name.clone());
            let name_idx2 = self.string_const(&enum_decl.name);
            self.current().emit(OpCode::DefineGlobal, line);
            self.current().emit_u16(name_idx2, line);
        }

        Ok(())
    }

    fn compile_statement(&mut self, stmt: &Statement) -> NiResult<()> {
        let line = stmt.span.line;
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                self.compile_expr(expr)?;
                self.current().emit(OpCode::Pop, line);
            }
            StmtKind::VarDecl(var_decl) => {
                self.compile_var_decl(var_decl, line)?;
            }
            StmtKind::ConstDecl(const_decl) => {
                self.compile_const_decl(const_decl, line)?;
            }
            StmtKind::If(if_stmt) => self.compile_if(if_stmt, line)?,
            StmtKind::While(while_stmt) => self.compile_while(while_stmt, line)?,
            StmtKind::For(for_stmt) => self.compile_for(for_stmt, line)?,
            StmtKind::Match(match_stmt) => self.compile_match(match_stmt, line)?,
            StmtKind::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e)?;
                } else {
                    self.current().emit(OpCode::None, line);
                }
                self.current().emit(OpCode::Return, line);
            }
            StmtKind::Break => {
                let loop_ctx = self
                    .current()
                    .loop_stack
                    .last()
                    .ok_or_else(|| NiError::compile("'break' outside of loop", stmt.span))?;
                let loop_depth = loop_ctx.depth;

                // Pop locals in nested scopes, using CloseUpvalue for captured locals
                let locals = &self.current_ref().locals;
                let start = locals
                    .iter()
                    .rposition(|l| l.depth <= loop_depth)
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let captured: Vec<bool> = locals[start..].iter().map(|l| l.is_captured).collect();
                for is_captured in captured.into_iter().rev() {
                    if is_captured {
                        self.current().emit(OpCode::CloseUpvalue, line);
                    } else {
                        self.current().emit(OpCode::Pop, line);
                    }
                }

                let jump = self.current().emit_jump(OpCode::Jump, line);
                self.current()
                    .loop_stack
                    .last_mut()
                    .unwrap()
                    .break_jumps
                    .push(jump);
            }
            StmtKind::Continue => {
                let loop_ctx = self
                    .current()
                    .loop_stack
                    .last()
                    .ok_or_else(|| NiError::compile("'continue' outside of loop", stmt.span))?;
                let loop_start = loop_ctx.start;
                let loop_depth = loop_ctx.depth;

                // Pop locals in nested scopes, using CloseUpvalue for captured locals
                let locals = &self.current_ref().locals;
                let start = locals
                    .iter()
                    .rposition(|l| l.depth <= loop_depth)
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let captured: Vec<bool> = locals[start..].iter().map(|l| l.is_captured).collect();
                for is_captured in captured.into_iter().rev() {
                    if is_captured {
                        self.current().emit(OpCode::CloseUpvalue, line);
                    } else {
                        self.current().emit(OpCode::Pop, line);
                    }
                }

                self.current().emit_loop(loop_start, line);
            }
            StmtKind::Pass => {} // no-op
            StmtKind::Block(stmts) => {
                self.current().begin_scope();
                for s in stmts {
                    self.compile_statement(s)?;
                }
                self.current().end_scope(line);
            }
            StmtKind::Try(try_stmt) => self.compile_try(try_stmt, line)?,
            StmtKind::Fail(expr) => {
                self.compile_expr(expr)?;
                self.current().emit(OpCode::Fail, line);
            }
            StmtKind::Assert(condition, message) => {
                self.compile_assert(condition, message.as_ref(), line)?;
            }
        }
        Ok(())
    }

    fn compile_if(&mut self, if_stmt: &IfStmt, line: usize) -> NiResult<()> {
        self.compile_expr(&if_stmt.condition)?;
        let then_jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
        self.current().emit(OpCode::Pop, line); // pop condition

        self.current().begin_scope();
        for stmt in &if_stmt.then_body {
            self.compile_statement(stmt)?;
        }
        self.current().end_scope(line);

        let mut end_jumps = vec![self.current().emit_jump(OpCode::Jump, line)];

        self.current().patch_jump(then_jump);
        self.current().emit(OpCode::Pop, line); // pop condition

        for (cond, body) in &if_stmt.elif_branches {
            self.compile_expr(cond)?;
            let elif_jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
            self.current().emit(OpCode::Pop, line);

            self.current().begin_scope();
            for stmt in body {
                self.compile_statement(stmt)?;
            }
            self.current().end_scope(line);

            end_jumps.push(self.current().emit_jump(OpCode::Jump, line));

            self.current().patch_jump(elif_jump);
            self.current().emit(OpCode::Pop, line);
        }

        if let Some(else_body) = &if_stmt.else_body {
            self.current().begin_scope();
            for stmt in else_body {
                self.compile_statement(stmt)?;
            }
            self.current().end_scope(line);
        }

        for jump in end_jumps {
            self.current().patch_jump(jump);
        }

        Ok(())
    }

    fn compile_while(&mut self, while_stmt: &WhileStmt, line: usize) -> NiResult<()> {
        let loop_start = self.current().chunk.current_offset();
        let depth = self.current_ref().scope_depth;

        self.current().loop_stack.push(LoopContext {
            start: loop_start,
            break_jumps: Vec::new(),
            depth,
        });

        self.compile_expr(&while_stmt.condition)?;
        let exit_jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
        self.current().emit(OpCode::Pop, line);

        self.current().begin_scope();
        for stmt in &while_stmt.body {
            self.compile_statement(stmt)?;
        }
        self.current().end_scope(line);

        self.current().emit_loop(loop_start, line);

        self.current().patch_jump(exit_jump);
        self.current().emit(OpCode::Pop, line);

        let loop_ctx = self.current().loop_stack.pop().unwrap();
        for jump in loop_ctx.break_jumps {
            self.current().patch_jump(jump);
        }

        Ok(())
    }

    fn compile_for(&mut self, for_stmt: &ForStmt, line: usize) -> NiResult<()> {
        self.current().begin_scope();

        // Evaluate iterable and get iterator
        self.compile_expr(&for_stmt.iterable)?;
        self.current().emit(OpCode::GetIterator, line);
        let iter_slot = self.current().add_local("__iter__", false);

        // Loop variable -- mutable (reassigned each iteration)
        self.current().emit(OpCode::None, line);
        let var_slot = self.current().add_local(&for_stmt.variable, true);

        let loop_start = self.current().chunk.current_offset();
        let depth = self.current_ref().scope_depth;

        self.current().loop_stack.push(LoopContext {
            start: loop_start,
            break_jumps: Vec::new(),
            depth,
        });

        // Get next value from iterator
        self.current().emit(OpCode::GetLocal, line);
        self.current().emit_byte(iter_slot, line);
        let exit_jump = self.current().emit_jump(OpCode::IteratorNext, line);

        // Store in loop variable
        self.current().emit(OpCode::SetLocal, line);
        self.current().emit_byte(var_slot, line);
        self.current().emit(OpCode::Pop, line);

        // Compile body
        self.current().begin_scope();
        for stmt in &for_stmt.body {
            self.compile_statement(stmt)?;
        }
        self.current().end_scope(line);

        self.current().emit_loop(loop_start, line);

        self.current().patch_jump(exit_jump);

        let loop_ctx = self.current().loop_stack.pop().unwrap();
        for jump in loop_ctx.break_jumps {
            self.current().patch_jump(jump);
        }

        self.current().end_scope(line);

        Ok(())
    }

    fn compile_match(&mut self, match_stmt: &MatchStmt, line: usize) -> NiResult<()> {
        self.compile_expr(&match_stmt.subject)?;
        self.compile_match_cases(&match_stmt.cases, line)?;
        Ok(())
    }

    /// Compile match cases. Expects the subject value on top of the stack.
    /// Pops the subject when done.
    fn compile_match_cases(&mut self, cases: &[MatchCase], line: usize) -> NiResult<()> {
        let mut end_jumps = Vec::new();

        for case in cases {
            let mut pattern_jump = None;
            let mut has_binding_scope = false;

            for (i, pattern) in case.patterns.iter().enumerate() {
                match pattern {
                    Pattern::Wildcard => {
                        // Always matches
                    }
                    Pattern::Literal(expr) => {
                        self.current().emit(OpCode::Dup, line);
                        self.compile_expr(expr)?;
                        self.current().emit(OpCode::Equal, line);

                        if i < case.patterns.len() - 1 {
                            // Multiple patterns: OR them
                            let or_jump = self.current().emit_jump(OpCode::JumpIfTrue, line);
                            self.current().emit(OpCode::Pop, line);
                            // Continue to next pattern
                            pattern_jump = Some(or_jump);
                        } else if let Some(pj) = pattern_jump.take() {
                            let no_match = self.current().emit_jump(OpCode::JumpIfFalse, line);
                            self.current().emit(OpCode::Pop, line);
                            self.current().patch_jump(pj);
                            self.current().emit(OpCode::Pop, line);
                            pattern_jump = Some(no_match);
                        } else {
                            pattern_jump =
                                Some(self.current().emit_jump(OpCode::JumpIfFalse, line));
                            self.current().emit(OpCode::Pop, line);
                        }
                    }
                    Pattern::Binding(name) => {
                        self.current().emit(OpCode::Dup, line);
                        self.current().begin_scope();
                        self.current().add_local(name, false);
                        has_binding_scope = true;
                    }
                    Pattern::TypeCheck(_, _) => {
                        // Simplified: just match (full type checking is Phase 2)
                    }
                }
            }

            // Guard
            if let Some(guard) = &case.guard {
                self.compile_expr(guard)?;
                let guard_fail = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line);

                // Execute body
                self.current().begin_scope();
                for stmt in &case.body {
                    self.compile_statement(stmt)?;
                }
                self.current().end_scope(line);

                // Close binding scope (if any) after body
                if has_binding_scope {
                    self.current().end_scope(line);
                }

                end_jumps.push(self.current().emit_jump(OpCode::Jump, line));
                self.current().patch_jump(guard_fail);
                self.current().emit(OpCode::Pop, line);
            } else {
                // Execute body
                self.current().begin_scope();
                for stmt in &case.body {
                    self.compile_statement(stmt)?;
                }
                self.current().end_scope(line);

                // Close binding scope (if any) after body
                if has_binding_scope {
                    self.current().end_scope(line);
                }

                end_jumps.push(self.current().emit_jump(OpCode::Jump, line));
            }

            if let Some(pj) = pattern_jump {
                self.current().patch_jump(pj);
                self.current().emit(OpCode::Pop, line);
            }
        }

        // Patch end_jumps BEFORE the Pop so all paths (match or fallthrough) pop the subject
        for jump in end_jumps {
            self.current().patch_jump(jump);
        }

        // Pop the subject
        self.current().emit(OpCode::Pop, line);

        Ok(())
    }

    fn compile_import(&mut self, import_decl: &ImportDecl, line: usize) -> NiResult<()> {
        // Extract the module path string for native module lookup
        let module_path = match import_decl {
            ImportDecl::Module { path, .. } => path.clone(),
            ImportDecl::From { path, .. } => path.clone(),
            ImportDecl::FromAll { path } => path.clone(),
        };

        // Check for native modules first (works without source_root)
        let native_module_name = if module_path.len() == 1 {
            Some(module_path[0].as_str())
        } else {
            None
        };
        let native_globals = native_module_name.and_then(|name| self.resolve_native_module(name));

        // If not a native module, resolve from the file system
        let module_globals = if let Some(globals) = native_globals {
            globals
        } else {
            let source_root = match &self.source_root {
                Some(root) => root.clone(),
                None => {
                    return Err(NiError::runtime(
                        "Cannot use import without a source file context",
                    ));
                }
            };
            let file_path = self.resolve_import_path(&source_root, &module_path);
            self.compile_module_file(&file_path, line)?
        };

        // For From and Module imports, define ALL module globals so that
        // cross-function references work (imported functions call sibling
        // functions via GetGlobal).
        match import_decl {
            ImportDecl::Module { path, alias } => {
                // Define all module globals for cross-function references
                for (name, value) in &module_globals {
                    self.current().emit_constant(value.clone(), line);
                    let name_idx = self.string_const(name);
                    self.current().emit(OpCode::DefineGlobal, line);
                    self.current().emit_u16(name_idx, line);
                }
                // Then define the module map for user access
                let module_name = alias
                    .clone()
                    .unwrap_or_else(|| path.last().unwrap().clone());
                let map_entries: Vec<(Value, Value)> = module_globals
                    .into_iter()
                    .map(|(k, v)| {
                        let key_ref = self.heap.alloc(NiObject::String(k));
                        (Value::Object(key_ref), v)
                    })
                    .collect();
                let map_ref = self.heap.alloc(NiObject::Map(map_entries));
                self.current().emit_constant(Value::Object(map_ref), line);
                if self.current_ref().scope_depth > 0 {
                    self.current().add_local(&module_name, false);
                } else {
                    self.immutable_globals.insert(module_name.clone());
                    let name_idx = self.string_const(&module_name);
                    self.current().emit(OpCode::DefineGlobal, line);
                    self.current().emit_u16(name_idx, line);
                }
            }
            ImportDecl::From { names, .. } => {
                // Define all module globals for cross-function references
                for (name, value) in &module_globals {
                    self.current().emit_constant(value.clone(), line);
                    let name_idx = self.string_const(name);
                    self.current().emit(OpCode::DefineGlobal, line);
                    self.current().emit_u16(name_idx, line);
                }
                // Then bind the explicitly imported names
                for import_name in names {
                    let value = module_globals
                        .get(&import_name.name)
                        .cloned()
                        .unwrap_or(Value::None);
                    let local_name = import_name
                        .alias
                        .clone()
                        .unwrap_or(import_name.name.clone());
                    self.current().emit_constant(value, line);
                    if self.current_ref().scope_depth > 0 {
                        self.current().add_local(&local_name, false);
                    } else {
                        self.immutable_globals.insert(local_name.clone());
                        let name_idx = self.string_const(&local_name);
                        self.current().emit(OpCode::DefineGlobal, line);
                        self.current().emit_u16(name_idx, line);
                    }
                }
            }
            ImportDecl::FromAll { .. } => {
                for (name, value) in module_globals {
                    self.current().emit_constant(value, line);
                    if self.current_ref().scope_depth > 0 {
                        self.current().add_local(&name, false);
                    } else {
                        self.immutable_globals.insert(name.clone());
                        let name_idx = self.string_const(&name);
                        self.current().emit(OpCode::DefineGlobal, line);
                        self.current().emit_u16(name_idx, line);
                    }
                }
            }
        }
        Ok(())
    }

    fn resolve_native_module(
        &mut self,
        name: &str,
    ) -> Option<std::collections::HashMap<String, Value>> {
        if let Some(ref allowed) = self.allowed_modules {
            if !allowed.iter().any(|m| m == name) {
                return None;
            }
        }
        let entries = match name {
            "math" => ni_vm::native::create_math_module(self.heap),
            "random" => ni_vm::native::create_random_module(self.heap),
            "time" => ni_vm::native::create_time_module(self.heap),
            "json" => ni_vm::native::create_json_module(self.heap),
            "nion" => ni_vm::native::create_nion_module(self.heap),
            _ => return None,
        };
        let mut map = std::collections::HashMap::new();
        for (key, value) in entries {
            if let Value::Object(kr) = &key {
                if let Some(NiObject::String(s)) = self.heap.get(*kr) {
                    map.insert(s.clone(), value);
                }
            }
        }
        Some(map)
    }

    #[allow(clippy::ptr_arg)]
    fn resolve_import_path(&self, source_root: &PathBuf, path: &[String]) -> PathBuf {
        let mut file_path = source_root.clone();
        for component in path {
            file_path = file_path.join(component);
        }
        file_path.set_extension("ni");
        file_path
    }

    fn compile_module_file(
        &mut self,
        file_path: &PathBuf,
        line: usize,
    ) -> NiResult<std::collections::HashMap<String, Value>> {
        let path_str = file_path.to_string_lossy().to_string();

        // Canonicalize the file path to resolve symlinks and normalize the path
        let canonical = file_path
            .canonicalize()
            .map_err(|e| NiError::runtime(format!("Cannot resolve module path '{}': {}", path_str, e)))?;
        let canonical_root = self
            .source_root
            .as_ref()
            .and_then(|r| r.canonicalize().ok())
            .or_else(|| self.source_root.clone())
            .unwrap_or_else(|| canonical.clone());

        // Prevent path traversal: module must be within the project root
        if !canonical.starts_with(&canonical_root) {
            return Err(NiError::compile(
                format!("Import path escapes project root: {}", path_str),
                ni_error::Span::new(0, 0, line, line, 0, 0),
            ));
        }

        // Use the canonical path string for circular import detection to prevent bypass
        let canonical_str = canonical.to_string_lossy().to_string();

        // Circular import detection
        if self.import_stack.contains(&canonical_str) {
            return Err(NiError::compile(
                format!("Circular import detected: {}", path_str),
                ni_error::Span::new(0, 0, line, line, 0, 0),
            ));
        }

        // Enforce module file size limit (10 MB)
        let metadata = std::fs::metadata(&canonical)
            .map_err(|e| NiError::runtime(format!("Cannot read module '{}': {}", path_str, e)))?;
        if metadata.len() > 10 * 1024 * 1024 {
            return Err(NiError::compile(
                format!("Module file too large: {} bytes (max 10MB)", metadata.len()),
                ni_error::Span::new(0, 0, line, line, 0, 0),
            ));
        }

        // Read the module source
        let source = std::fs::read_to_string(&canonical)
            .map_err(|e| NiError::runtime(format!("Cannot read module '{}': {}", path_str, e)))?;

        // Lex and parse
        let tokens = ni_lexer::lex(&source)?;
        let program = ni_parser::parse(tokens)?;

        self.import_stack.insert(canonical_str.clone());

        // Compile and run in a module VM (compile against the module VM's heap)
        let source_root = self.source_root.clone();
        let import_stack = self.import_stack.clone();

        let mut module_vm = ni_vm::Vm::new();
        module_vm.set_instruction_limit(10_000_000); // 10M instructions max for module init
        let closure_ref = {
            let mut module_compiler = Compiler::new(&mut module_vm.heap, &mut module_vm.interner);
            module_compiler.source_root = source_root;
            module_compiler.import_stack = import_stack;
            module_compiler.compile(&program)?
        };

        module_vm.interpret(closure_ref).map_err(|e| {
            NiError::runtime(format!("Error in module '{}': {}", path_str, e.message))
        })?;

        self.import_stack.remove(&canonical_str);

        // Collect exported globals (filter out builtins)
        let builtins: HashSet<&str> = [
            "print",
            "type_of",
            "type",
            "len",
            "clock",
            "to_string",
            "to_int",
            "to_float",
            "to_bool",
            "abs",
            "min",
            "max",
            "clamp",
            "sqrt",
            "floor",
            "ceil",
            "round",
            "sin",
            "cos",
            "enumerate",
            "range",
            "log",
            "log_warning",
            "log_error",
        ]
        .into_iter()
        .collect();

        // Deep-copy exported values from module VM's heap to our heap,
        // remapping InternIds from the module's intern table to the main one.
        let module_globals: std::collections::HashMap<ni_vm::InternId, Value> =
            std::mem::take(&mut module_vm.globals);
        let mut exports = std::collections::HashMap::new();
        for (id, value) in module_globals {
            let name = module_vm.interner.resolve(id).to_string();
            if !builtins.contains(name.as_str()) {
                let copied = deep_copy_value(
                    value,
                    &module_vm.heap,
                    self.heap,
                    &module_vm.interner,
                    self.interner,
                );
                exports.insert(name, copied);
            }
        }

        Ok(exports)
    }

    fn compile_try(&mut self, try_stmt: &TryStmt, line: usize) -> NiResult<()> {
        let try_start = self.current().chunk.current_offset();
        let stack_depth = self.current_ref().locals.len();

        // Compile try body
        self.current().begin_scope();
        for stmt in &try_stmt.body {
            self.compile_statement(stmt)?;
        }
        self.current().end_scope(line);

        let try_end = self.current().chunk.current_offset();

        // Jump over catch block on normal completion
        let skip_catch = self.current().emit_jump(OpCode::Jump, line);

        // Catch handler starts here
        let handler_ip = self.current().chunk.current_offset();

        // Compile catch body based on type
        match &try_stmt.catch_body {
            CatchBody::Block(stmts) => {
                self.current().begin_scope();
                if let Some(catch_var) = &try_stmt.catch_var {
                    self.current().add_local(catch_var, false);
                } else {
                    self.current().emit(OpCode::Pop, line);
                }
                for stmt in stmts {
                    self.compile_statement(stmt)?;
                }
                self.current().end_scope(line);
            }
            CatchBody::Match(cases) => {
                // Error value is on stack as match subject
                if let Some(catch_var) = &try_stmt.catch_var {
                    // Add error as local, then push it again as match subject
                    self.current().begin_scope();
                    self.current().add_local(catch_var, false);
                    let slot = self.current().resolve_local(catch_var).unwrap();
                    self.current().emit(OpCode::GetLocal, line);
                    self.current().emit_byte(slot, line);
                    self.compile_match_cases(cases, line)?;
                    self.current().end_scope(line);
                } else {
                    // Error stays on stack, used directly as match subject
                    self.compile_match_cases(cases, line)?;
                }
            }
        }

        // Patch the skip-catch jump
        self.current().patch_jump(skip_catch);

        // Record exception entry
        self.current().exception_table.push(ExceptionEntry {
            try_start,
            try_end,
            handler_ip,
            stack_depth,
        });

        Ok(())
    }

    fn compile_assert(
        &mut self,
        condition: &Expr,
        message: Option<&Expr>,
        line: usize,
    ) -> NiResult<()> {
        // Rich assert: when no custom message and condition is a comparison,
        // emit AssertCmp that reports expected vs actual values.
        if message.is_none() {
            if let ExprKind::Compare { left, op, right } = &condition.kind {
                if *op != CmpOp::In {
                    // Stack: [left_val, right_val]
                    // AssertCmp does the comparison in the VM -- no double evaluation.
                    self.compile_expr(left)?;
                    self.compile_expr(right)?;
                    let cmp_tag = match op {
                        CmpOp::Eq => 0u8,
                        CmpOp::NotEq => 1,
                        CmpOp::Lt => 2,
                        CmpOp::Gt => 3,
                        CmpOp::LtEq => 4,
                        CmpOp::GtEq => 5,
                        _ => 0,
                    };
                    self.current().emit(OpCode::AssertCmp, line);
                    self.current().emit_byte(cmp_tag, line);
                    return Ok(());
                }
            }
        }

        // Standard assert
        self.compile_expr(condition)?;
        if let Some(msg) = message {
            self.compile_expr(msg)?;
            self.current().emit(OpCode::AssertOp, line);
            self.current().emit_byte(1, line);
        } else {
            self.current().emit(OpCode::AssertOp, line);
            self.current().emit_byte(0, line);
        }
        Ok(())
    }

    fn compile_try_expr(&mut self, inner: &Expr, line: usize) -> NiResult<()> {
        let catch_point_offset = {
            self.current().emit(OpCode::SetCatchPoint, line);
            let offset = self.current().chunk.current_offset();
            self.current().emit_u16(0, line); // placeholder
            offset
        };

        self.compile_expr(inner)?;

        self.current().emit(OpCode::ClearCatchPoint, line);
        let skip_handler = self.current().emit_jump(OpCode::Jump, line);

        // handler: VM pushes None and jumps here
        self.current().patch_jump(catch_point_offset);

        // after_handler: result or None is on stack
        self.current().patch_jump(skip_handler);

        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> NiResult<()> {
        let line = expr.span.line;
        match &expr.kind {
            ExprKind::IntLiteral(n) => {
                self.current().emit_constant(Value::Int(*n), line);
            }
            ExprKind::FloatLiteral(f) => {
                self.current().emit_constant(Value::Float(*f), line);
            }
            ExprKind::StringLiteral(s) => {
                let id = self.interner.intern(s);
                let r = self.heap.alloc(NiObject::InternedString(id));
                self.current().emit_constant(Value::Object(r), line);
            }
            ExprKind::FStringLiteral(parts) => {
                let mut count = 0u16;
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => {
                            let id = self.interner.intern(s);
                            let r = self.heap.alloc(NiObject::InternedString(id));
                            self.current().emit_constant(Value::Object(r), line);
                            count += 1;
                        }
                        FStringPart::Expr(e) => {
                            self.compile_expr(e)?;
                            count += 1;
                        }
                    }
                }
                self.current().emit(OpCode::StringConcat, line);
                self.current().emit_u16(count, line);
            }
            ExprKind::BoolLiteral(b) => {
                self.current()
                    .emit(if *b { OpCode::True } else { OpCode::False }, line);
            }
            ExprKind::NoneLiteral => {
                self.current().emit(OpCode::None, line);
            }
            ExprKind::Identifier(name) => {
                self.compile_variable_load(name, line)?;
            }
            ExprKind::SelfExpr => {
                // self is always local slot 0 in methods
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(0, line);
            }
            ExprKind::Negate(operand) => {
                self.compile_expr(operand)?;
                self.current().emit(OpCode::Negate, line);
            }
            ExprKind::Not(operand) => {
                self.compile_expr(operand)?;
                self.current().emit(OpCode::Not, line);
            }
            ExprKind::BinaryOp { left, op, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op {
                    BinOp::Add => self.current().emit(OpCode::Add, line),
                    BinOp::Sub => self.current().emit(OpCode::Subtract, line),
                    BinOp::Mul => self.current().emit(OpCode::Multiply, line),
                    BinOp::Div => self.current().emit(OpCode::Divide, line),
                    BinOp::Mod => self.current().emit(OpCode::Modulo, line),
                }
            }
            ExprKind::And(left, right) => {
                self.compile_expr(left)?;
                let jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line);
                self.compile_expr(right)?;
                self.current().patch_jump(jump);
            }
            ExprKind::Or(left, right) => {
                self.compile_expr(left)?;
                let jump = self.current().emit_jump(OpCode::JumpIfTrue, line);
                self.current().emit(OpCode::Pop, line);
                self.compile_expr(right)?;
                self.current().patch_jump(jump);
            }
            ExprKind::Compare { left, op, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op {
                    CmpOp::Eq => self.current().emit(OpCode::Equal, line),
                    CmpOp::NotEq => self.current().emit(OpCode::NotEqual, line),
                    CmpOp::Lt => self.current().emit(OpCode::Less, line),
                    CmpOp::Gt => self.current().emit(OpCode::Greater, line),
                    CmpOp::LtEq => self.current().emit(OpCode::LessEqual, line),
                    CmpOp::GtEq => self.current().emit(OpCode::GreaterEqual, line),
                    CmpOp::Is => self.current().emit(OpCode::Equal, line), // simplified
                    CmpOp::In => {
                        return Err(NiError::compile(
                            "'in' operator is not yet implemented in compiled code",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::Assign { target, value } => match &target.kind {
                ExprKind::Identifier(_) => {
                    self.compile_expr(value)?;
                    self.compile_assign_target(target, line)?;
                }
                ExprKind::GetField(obj, name) => {
                    self.compile_expr(obj)?;
                    self.compile_expr(value)?;
                    let name_idx = self.string_const(name);
                    self.current().emit(OpCode::SetProperty, line);
                    self.current().emit_u16(name_idx, line);
                }
                ExprKind::GetIndex(obj, index) => {
                    self.compile_expr(obj)?;
                    self.compile_expr(index)?;
                    self.compile_expr(value)?;
                    self.current().emit(OpCode::SetIndex, line);
                }
                _ => {
                    self.compile_expr(value)?;
                    self.compile_assign_target(target, line)?;
                }
            },
            ExprKind::CompoundAssign { target, op, value } => {
                match &target.kind {
                    ExprKind::Identifier(_) => {
                        self.compile_expr(target)?;
                        self.compile_expr(value)?;
                        match op {
                            BinOp::Add => self.current().emit(OpCode::Add, line),
                            BinOp::Sub => self.current().emit(OpCode::Subtract, line),
                            BinOp::Mul => self.current().emit(OpCode::Multiply, line),
                            BinOp::Div => self.current().emit(OpCode::Divide, line),
                            BinOp::Mod => self.current().emit(OpCode::Modulo, line),
                        }
                        self.compile_assign_target(target, line)?;
                    }
                    ExprKind::GetField(obj, name) => {
                        self.compile_expr(obj)?;
                        self.current().emit(OpCode::Dup, line); // duplicate receiver
                        let name_idx = self.string_const(name);
                        self.current().emit(OpCode::GetProperty, line);
                        self.current().emit_u16(name_idx, line);
                        self.compile_expr(value)?;
                        match op {
                            BinOp::Add => self.current().emit(OpCode::Add, line),
                            BinOp::Sub => self.current().emit(OpCode::Subtract, line),
                            BinOp::Mul => self.current().emit(OpCode::Multiply, line),
                            BinOp::Div => self.current().emit(OpCode::Divide, line),
                            BinOp::Mod => self.current().emit(OpCode::Modulo, line),
                        }
                        let name_idx2 = self.string_const(name);
                        self.current().emit(OpCode::SetProperty, line);
                        self.current().emit_u16(name_idx2, line);
                    }
                    ExprKind::GetIndex(obj, index) => {
                        // Stack for SetIndex: [obj, index, new_value]
                        // Push obj and index for SetIndex, then recompile obj+index for
                        // GetIndex to read the current value, then compute the new value.
                        self.compile_expr(obj)?;   // [obj]
                        self.compile_expr(index)?; // [obj, index]
                        // Recompile obj and index so GetIndex can consume them
                        self.compile_expr(obj)?;   // [obj, index, obj]
                        self.compile_expr(index)?; // [obj, index, obj, index]
                        self.current().emit(OpCode::GetIndex, line); // [obj, index, current_val]
                        self.compile_expr(value)?; // [obj, index, current_val, value]
                        match op {
                            BinOp::Add => self.current().emit(OpCode::Add, line),
                            BinOp::Sub => self.current().emit(OpCode::Subtract, line),
                            BinOp::Mul => self.current().emit(OpCode::Multiply, line),
                            BinOp::Div => self.current().emit(OpCode::Divide, line),
                            BinOp::Mod => self.current().emit(OpCode::Modulo, line),
                        }
                        // Stack: [obj, index, new_value]
                        self.current().emit(OpCode::SetIndex, line);
                    }
                    _ => {
                        self.compile_expr(target)?;
                        self.compile_expr(value)?;
                        match op {
                            BinOp::Add => self.current().emit(OpCode::Add, line),
                            BinOp::Sub => self.current().emit(OpCode::Subtract, line),
                            BinOp::Mul => self.current().emit(OpCode::Multiply, line),
                            BinOp::Div => self.current().emit(OpCode::Divide, line),
                            BinOp::Mod => self.current().emit(OpCode::Modulo, line),
                        }
                        self.compile_assign_target(target, line)?;
                    }
                }
            }
            ExprKind::GetField(obj, name) => {
                self.compile_expr(obj)?;
                let name_idx = self.string_const(name);
                self.current().emit(OpCode::GetProperty, line);
                self.current().emit_u16(name_idx, line);
            }
            ExprKind::SetField(obj, name, value) => {
                self.compile_expr(obj)?;
                self.compile_expr(value)?;
                let name_idx = self.string_const(name);
                self.current().emit(OpCode::SetProperty, line);
                self.current().emit_u16(name_idx, line);
            }
            ExprKind::GetIndex(obj, index) => {
                self.compile_expr(obj)?;
                self.compile_expr(index)?;
                self.current().emit(OpCode::GetIndex, line);
            }
            ExprKind::SetIndex(obj, index, value) => {
                self.compile_expr(obj)?;
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.current().emit(OpCode::SetIndex, line);
            }
            ExprKind::SafeNav(obj, name) => {
                self.compile_expr(obj)?;
                let name_idx = self.string_const(name);
                self.current().emit(OpCode::SafeNav, line);
                self.current().emit_u16(name_idx, line);
            }
            ExprKind::NoneCoalesce(left, right) => {
                self.compile_expr(left)?;
                let jump = self.current().emit_jump(OpCode::NoneCoalesce, line);
                self.compile_expr(right)?;
                self.current().patch_jump(jump);
            }
            ExprKind::Call {
                callee,
                args,
                named_args: _,
            } => {
                self.compile_expr(callee)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.current().emit(OpCode::Call, line);
                self.current().emit_byte(args.len() as u8, line);
            }
            ExprKind::MethodCall {
                object,
                method,
                args,
                named_args: _,
            } => {
                self.compile_expr(object)?;
                for arg in args {
                    self.compile_expr(arg)?;
                }
                let name_idx = self.string_const(method);
                self.current().emit(OpCode::Invoke, line);
                self.current().emit_u16(name_idx, line);
                self.current().emit_byte(args.len() as u8, line);
            }
            ExprKind::SuperCall { method, args } => {
                // Load self
                self.current().emit(OpCode::GetLocal, line);
                self.current().emit_byte(0, line);

                for arg in args {
                    self.compile_expr(arg)?;
                }

                // Get superclass from the class
                let name_idx = self.string_const(method);
                self.current().emit(OpCode::SuperInvoke, line);
                self.current().emit_u16(name_idx, line);
                self.current().emit_byte(args.len() as u8, line);
            }
            ExprKind::List(elements) => {
                for elem in elements {
                    self.compile_expr(elem)?;
                }
                self.current().emit(OpCode::BuildList, line);
                self.current().emit_u16(elements.len() as u16, line);
            }
            ExprKind::Map(entries) => {
                for (key, val) in entries {
                    self.compile_expr(key)?;
                    self.compile_expr(val)?;
                }
                self.current().emit(OpCode::BuildMap, line);
                self.current().emit_u16(entries.len() as u16, line);
            }
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                self.compile_expr(start)?;
                self.compile_expr(end)?;
                self.current().emit(OpCode::BuildRange, line);
                self.current()
                    .emit_byte(if *inclusive { 1 } else { 0 }, line);
            }
            ExprKind::Lambda { params, body } => {
                let fun_decl = FunDecl {
                    name: "<lambda>".to_string(),
                    params: params.clone(),
                    return_type: None,
                    body: body.clone(),
                    docstring: None,
                };
                self.compile_function(&fun_decl, line)?;
            }
            ExprKind::IfExpr {
                value,
                condition,
                else_value,
            } => {
                // value if condition else else_value
                self.compile_expr(condition)?;
                let else_jump = self.current().emit_jump(OpCode::JumpIfFalse, line);
                self.current().emit(OpCode::Pop, line);
                self.compile_expr(value)?;
                let end_jump = self.current().emit_jump(OpCode::Jump, line);
                self.current().patch_jump(else_jump);
                self.current().emit(OpCode::Pop, line);
                self.compile_expr(else_value)?;
                self.current().patch_jump(end_jump);
            }
            ExprKind::Spawn(call) => {
                self.compile_expr(call)?;
                self.current().emit(OpCode::SpawnFiber, line);
            }
            ExprKind::Yield(value) => {
                if let Some(val) = value {
                    self.compile_expr(val)?;
                } else {
                    self.current().emit(OpCode::None, line);
                }
                self.current().emit(OpCode::Yield, line);
            }
            ExprKind::Wait(duration) => {
                self.compile_expr(duration)?;
                self.current().emit(OpCode::Wait, line);
            }
            ExprKind::Await(inner) => {
                self.compile_expr(inner)?;
                self.current().emit(OpCode::Await, line);
            }
            ExprKind::TryExpr(inner) => {
                self.compile_try_expr(inner, line)?;
            }
            ExprKind::FailExpr(value) => {
                self.compile_expr(value)?;
                self.current().emit(OpCode::Fail, line);
            }
        }
        Ok(())
    }

    fn compile_variable_load(&mut self, name: &str, line: usize) -> NiResult<()> {
        // Try local first
        if let Some(slot) = self.current().resolve_local(name) {
            self.current().emit(OpCode::GetLocal, line);
            self.current().emit_byte(slot, line);
            return Ok(());
        }

        // Try upvalue
        if let Some(slot) = self.resolve_upvalue(name) {
            self.current().emit(OpCode::GetUpvalue, line);
            self.current().emit_byte(slot, line);
            return Ok(());
        }

        // Global
        let name_idx = self.string_const(name);
        self.current().emit(OpCode::GetGlobal, line);
        self.current().emit_u16(name_idx, line);
        Ok(())
    }

    fn check_assignment_allowed(&self, name: &str, span: ni_error::Span) -> NiResult<()> {
        // Check locals in current function
        let compiler = self.current_ref();
        for local in compiler.locals.iter().rev() {
            if local.name == name {
                if !local.is_mutable {
                    return Err(NiError::compile(
                        format!("Cannot assign to immutable variable '{}'", name),
                        span,
                    ));
                }
                return Ok(());
            }
        }

        // Check upvalues: walk enclosing compilers to find the original local
        if self.compilers.len() >= 2 {
            for depth in (0..self.compilers.len() - 1).rev() {
                if let Some(slot) = self.compilers[depth].resolve_local(name) {
                    if !self.compilers[depth].locals[slot as usize].is_mutable {
                        return Err(NiError::compile(
                            format!("Cannot assign to immutable variable '{}'", name),
                            span,
                        ));
                    }
                    return Ok(());
                }
            }
        }

        // Check immutable globals
        if self.immutable_globals.contains(name) {
            return Err(NiError::compile(
                format!("Cannot assign to immutable binding '{}'", name),
                span,
            ));
        }

        Ok(())
    }

    fn compile_assign_target(&mut self, target: &Expr, line: usize) -> NiResult<()> {
        match &target.kind {
            ExprKind::Identifier(name) => {
                self.check_assignment_allowed(name, target.span)?;
                if let Some(slot) = self.current().resolve_local(name) {
                    self.current().emit(OpCode::SetLocal, line);
                    self.current().emit_byte(slot, line);
                } else if let Some(slot) = self.resolve_upvalue(name) {
                    self.current().emit(OpCode::SetUpvalue, line);
                    self.current().emit_byte(slot, line);
                } else {
                    let name_idx = self.string_const(name);
                    self.current().emit(OpCode::SetGlobal, line);
                    self.current().emit_u16(name_idx, line);
                }
            }
            ExprKind::GetField(obj, name) => {
                self.compile_expr(obj)?;
                let name_idx = self.string_const(name);
                self.current().emit(OpCode::SetProperty, line);
                self.current().emit_u16(name_idx, line);
            }
            ExprKind::GetIndex(obj, index) => {
                self.compile_expr(obj)?;
                self.compile_expr(index)?;
                self.current().emit(OpCode::SetIndex, line);
            }
            _ => {
                return Err(NiError::compile("Invalid assignment target", target.span));
            }
        }
        Ok(())
    }

    fn resolve_upvalue(&mut self, name: &str) -> Option<u8> {
        if self.compilers.len() < 2 {
            return None;
        }

        let len = self.compilers.len();
        let current = len - 1;

        // Search enclosing compilers from innermost to outermost
        for depth in (0..current).rev() {
            // Check for a local variable in this compiler
            if let Some(slot) = self.compilers[depth].resolve_local(name) {
                self.compilers[depth].locals[slot as usize].is_captured = true;
                // Chain upvalues: depth+1 gets is_local=true, each further level is_local=false
                let mut upvalue_idx = self.add_upvalue(depth + 1, slot, true, name);
                for d in (depth + 2)..=current {
                    upvalue_idx = self.add_upvalue(d, upvalue_idx, false, name);
                }
                return Some(upvalue_idx);
            }

            // Check for an existing upvalue in this compiler (already captured from outer)
            for i in 0..self.compilers[depth].upvalues.len() {
                if self.compilers[depth].upvalues[i].name == name {
                    // Chain from depth+1 up to current
                    let mut upvalue_idx = self.add_upvalue(depth + 1, i as u8, false, name);
                    for d in (depth + 2)..=current {
                        upvalue_idx = self.add_upvalue(d, upvalue_idx, false, name);
                    }
                    return Some(upvalue_idx);
                }
            }
        }

        None
    }

    fn add_upvalue(&mut self, compiler_idx: usize, index: u8, is_local: bool, name: &str) -> u8 {
        let compiler = &mut self.compilers[compiler_idx];
        // Check if already exists
        for (i, uv) in compiler.upvalues.iter().enumerate() {
            if uv.index == index && uv.is_local == is_local {
                return i as u8;
            }
        }

        let slot = compiler.upvalues.len() as u8;
        compiler.upvalues.push(UpvalueInfo {
            index,
            is_local,
            name: name.to_string(),
        });
        slot
    }
}

/// Enumerate all root-to-leaf paths through a spec section tree.
/// Each path is a Vec of SpecSections from root (given) to leaf (then).
fn enumerate_spec_paths(sections: &[SpecSection]) -> Vec<Vec<&SpecSection>> {
    let mut paths = Vec::new();
    for section in sections {
        let mut current_path = vec![section];
        collect_paths(section, &mut current_path, &mut paths);
    }
    paths
}

fn collect_paths<'a>(
    section: &'a SpecSection,
    current_path: &mut Vec<&'a SpecSection>,
    paths: &mut Vec<Vec<&'a SpecSection>>,
) {
    if section.children.is_empty() {
        // Leaf node -- record this path
        paths.push(current_path.clone());
    } else {
        for child in &section.children {
            current_path.push(child);
            collect_paths(child, current_path, paths);
            current_path.pop();
        }
    }
}

/// Deep-copy a Value from one GcHeap to another.
fn deep_copy_value(
    value: Value,
    source: &GcHeap,
    target: &mut GcHeap,
    src_interner: &InternTable,
    dst_interner: &mut InternTable,
) -> Value {
    match value {
        Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::None => value,
        Value::Object(r) => {
            if let Some(obj) = source.get(r) {
                let new_obj = deep_copy_object(obj, source, target, src_interner, dst_interner);
                let new_ref = target.alloc(new_obj);
                Value::Object(new_ref)
            } else {
                Value::None
            }
        }
    }
}

fn deep_copy_object(
    obj: &NiObject,
    source: &GcHeap,
    target: &mut GcHeap,
    src_interner: &InternTable,
    dst_interner: &mut InternTable,
) -> NiObject {
    match obj {
        NiObject::String(s) => NiObject::String(s.clone()),
        NiObject::InternedString(id) => {
            // Remap: resolve in source interner, intern in destination
            let s = src_interner.resolve(*id);
            let new_id = dst_interner.intern(s);
            NiObject::InternedString(new_id)
        }
        NiObject::Function(f) => {
            let mut new_chunk = f.chunk.clone();
            for c in &mut new_chunk.constants {
                *c = deep_copy_value(c.clone(), source, target, src_interner, dst_interner);
            }
            NiObject::Function(NiFunction {
                name: f.name.clone(),
                arity: f.arity,
                default_count: f.default_count,
                chunk: new_chunk,
                upvalue_count: f.upvalue_count,
                exception_table: f.exception_table.clone(),
                local_var_table: f.local_var_table.clone(),
                upvalue_names: f.upvalue_names.clone(),
                docstring: f.docstring.clone(),
            })
        }
        NiObject::Closure(c) => {
            let func_val = deep_copy_value(
                Value::Object(c.function),
                source,
                target,
                src_interner,
                dst_interner,
            );
            let func_ref = match func_val {
                Value::Object(r) => r,
                _ => unreachable!(),
            };
            NiObject::Closure(NiClosure {
                function: func_ref,
                upvalues: vec![], // module-level closures have no upvalues
            })
        }
        NiObject::List(items) => {
            let new_items: Vec<Value> = items
                .iter()
                .map(|v| deep_copy_value(v.clone(), source, target, src_interner, dst_interner))
                .collect();
            NiObject::List(new_items)
        }
        NiObject::Map(entries) => {
            let new_entries: Vec<(Value, Value)> = entries
                .iter()
                .map(|(k, v)| {
                    (
                        deep_copy_value(k.clone(), source, target, src_interner, dst_interner),
                        deep_copy_value(v.clone(), source, target, src_interner, dst_interner),
                    )
                })
                .collect();
            NiObject::Map(new_entries)
        }
        NiObject::Class(c) => {
            // Remap method InternIds and deep-copy method closures
            let mut new_methods = std::collections::HashMap::new();
            for (id, method_ref) in &c.methods {
                let new_id = {
                    let s = src_interner.resolve(*id);
                    dst_interner.intern(s)
                };
                let new_val = deep_copy_value(
                    Value::Object(*method_ref),
                    source,
                    target,
                    src_interner,
                    dst_interner,
                );
                let new_ref = match new_val {
                    Value::Object(r) => r,
                    _ => unreachable!(),
                };
                new_methods.insert(new_id, new_ref);
            }
            // Remap field default InternIds and deep-copy values
            let mut new_fields = std::collections::HashMap::new();
            for (id, val) in &c.fields {
                let new_id = {
                    let s = src_interner.resolve(*id);
                    dst_interner.intern(s)
                };
                let new_val =
                    deep_copy_value(val.clone(), source, target, src_interner, dst_interner);
                new_fields.insert(new_id, new_val);
            }
            // Deep-copy superclass reference if present
            let new_superclass = c.superclass.map(|sc_ref| {
                let new_val = deep_copy_value(
                    Value::Object(sc_ref),
                    source,
                    target,
                    src_interner,
                    dst_interner,
                );
                match new_val {
                    Value::Object(r) => r,
                    _ => unreachable!(),
                }
            });
            NiObject::Class(NiClass {
                name: c.name.clone(),
                methods: new_methods,
                superclass: new_superclass,
                fields: new_fields,
                docstring: c.docstring.clone(),
            })
        }
        NiObject::Enum(e) => {
            // Remap variant InternIds and deep-copy values
            let mut new_variants = std::collections::HashMap::new();
            for (id, val) in &e.variants {
                let new_id = {
                    let s = src_interner.resolve(*id);
                    dst_interner.intern(s)
                };
                let new_val =
                    deep_copy_value(val.clone(), source, target, src_interner, dst_interner);
                new_variants.insert(new_id, new_val);
            }
            NiObject::Enum(NiEnum {
                name: e.name.clone(),
                variants: new_variants,
            })
        }
        NiObject::Instance(inst) => {
            // Deep-copy the class reference and remap field InternIds
            let new_class_val = deep_copy_value(
                Value::Object(inst.class),
                source,
                target,
                src_interner,
                dst_interner,
            );
            let new_class_ref = match new_class_val {
                Value::Object(r) => r,
                _ => unreachable!(),
            };
            let mut new_fields = std::collections::HashMap::new();
            for (id, val) in &inst.fields {
                let new_id = {
                    let s = src_interner.resolve(*id);
                    dst_interner.intern(s)
                };
                let new_val =
                    deep_copy_value(val.clone(), source, target, src_interner, dst_interner);
                new_fields.insert(new_id, new_val);
            }
            NiObject::Instance(NiInstance {
                class: new_class_ref,
                fields: new_fields,
            })
        }
        // For other types, clone as-is
        other => other.clone(),
    }
}
