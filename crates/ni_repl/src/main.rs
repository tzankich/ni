use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use ni_error::NiError;
use ni_vm::{GcRef, Value, Vm};

#[cfg(test)]
mod tests;

fn run_source(vm: &mut Vm, source: &str) -> Result<(), NiError> {
    let tokens = ni_lexer::lex(source).inspect_err(|e| {
        eprintln!("{}", e.format_with_source(source));
    })?;

    let program = ni_parser::parse(tokens).inspect_err(|e| {
        eprintln!("{}", e.format_with_source(source));
    })?;

    let closure =
        ni_compiler::compile(&program, &mut vm.heap, &mut vm.interner).inspect_err(|e| {
            eprintln!("{}", e.format_with_source(source));
        })?;

    vm.interpret(closure).map_err(|e| {
        eprintln!("{}", e);
        e
    })?;

    Ok(())
}

fn run_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let mut vm = Vm::new();

    let source_root = Path::new(path).parent().map(|p| p.to_path_buf());

    let tokens = ni_lexer::lex(&source).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let program = ni_parser::parse(tokens).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let closure = if let Some(root) = source_root {
        ni_compiler::compile_with_source_root(&program, &mut vm.heap, &mut vm.interner, root)
    } else {
        ni_compiler::compile(&program, &mut vm.heap, &mut vm.interner)
    }
    .map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    vm.interpret(closure).map_err(|e| {
        eprintln!("{}", e);
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    Ok(())
}

fn run_repl() {
    println!("Ni Language v0.1.0");
    println!("Type expressions or statements. Use Ctrl+D to exit.");
    println!();

    let stdin = io::stdin();
    let mut vm = Vm::new();

    loop {
        print!("ni> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        let bytes_read = stdin.lock().read_line(&mut line).unwrap();
        if bytes_read == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if we need continuation (line ends with :)
        if trimmed.ends_with(':') {
            let mut full_source = line.clone();
            loop {
                print!("... ");
                io::stdout().flush().unwrap();
                let mut cont_line = String::new();
                let bytes = stdin.lock().read_line(&mut cont_line).unwrap();
                if bytes == 0 {
                    break;
                }
                if cont_line.trim().is_empty() {
                    break;
                }
                full_source.push_str(&cont_line);
            }
            match run_source(&mut vm, &full_source) {
                Ok(()) => {}
                Err(_) => {} // error already printed
            }
        } else {
            // Try as expression first (auto-print result)
            let expr_source = format!("print({})", trimmed);
            if run_source(&mut vm, &expr_source).is_err() {
                // Try as statement
                match run_source(&mut vm, &line) {
                    Ok(()) => {}
                    Err(_) => {} // error already printed
                }
            }
        }
    }
}

fn run_codegen(target: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;

    let tokens = ni_lexer::lex(&source).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let program = ni_parser::parse(tokens).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let output = match target {
        "rust" => ni_codegen::codegen_rust(&program),
        "c" => ni_codegen::codegen_c(&program),
        _ => {
            eprintln!("Unknown target '{}'. Use 'rust' or 'c'.", target);
            std::process::exit(1);
        }
    };

    print!("{}", output);
    Ok(())
}

fn run_lint(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;

    let tokens = ni_lexer::lex(&source).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let program = ni_parser::parse(tokens).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let diagnostics = ni_lint::lint(&program);

    if diagnostics.is_empty() {
        println!("No issues found.");
    } else {
        for diag in &diagnostics {
            eprintln!("{}", diag.format_with_source(&source));
            eprintln!();
        }
        eprintln!("{} warning(s) found.", diagnostics.len());
        std::process::exit(1);
    }

    Ok(())
}

fn run_fmt(path: &str, write: bool) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;

    let formatted = ni_fmt::format(&source).map_err(|e| {
        eprintln!("{}", e.format_with_source(&source));
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    if write {
        std::fs::write(path, &formatted)?;
        println!("Formatted {path}");
    } else {
        print!("{formatted}");
    }

    Ok(())
}

/// Discover spec files recursively from a directory.
fn discover_spec_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    discover_spec_files_recursive(dir, &mut files);
    files.sort();
    files
}

fn discover_spec_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            discover_spec_files_recursive(&path, files);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".spec.ni") {
                files.push(path);
            } else if name.ends_with(".ni") {
                // Quick text scan: check if any line starts with `spec "`
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.lines().any(|line| line.starts_with("spec \"")) {
                        files.push(path);
                    }
                }
            }
        }
    }
}

struct SpecResult {
    name: String,
    passed: bool,
    error: Option<String>,
    line: Option<usize>,
}

fn get_closure_arity(vm: &Vm, closure_ref: GcRef) -> u8 {
    if let Some(obj) = vm.heap.get(closure_ref) {
        if let Some(closure) = obj.as_closure() {
            if let Some(func) = vm.heap.get(closure.function) {
                if let Some(f) = func.as_function() {
                    return f.arity;
                }
            }
        }
    }
    0
}

/// Run specs in a single file. Returns (file_results, pass_count, fail_count).
fn run_spec_file(path: &Path) -> Result<Vec<SpecResult>, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;

    let source_root = path.parent().map(|p| p.to_path_buf());

    let tokens = ni_lexer::lex(&source).map_err(|e| e.format_with_source(&source).to_string())?;

    let program =
        ni_parser::parse(tokens).map_err(|e| e.format_with_source(&source).to_string())?;

    let mut vm = Vm::new();

    let closure = if let Some(root) = source_root {
        ni_compiler::compile_spec_mode_with_source_root(
            &program,
            &mut vm.heap,
            &mut vm.interner,
            root,
        )
    } else {
        ni_compiler::compile_spec_mode(&program, &mut vm.heap, &mut vm.interner)
    }
    .map_err(|e| e.format_with_source(&source).to_string())?;

    // Run top-level code (defines functions, classes, and registers spec closures as globals)
    vm.interpret(closure).map_err(|e| format!("{}", e))?;

    // Collect spec globals: keys starting with "spec:"
    let mut spec_entries: Vec<(String, ni_vm::GcRef)> = Vec::new();
    for (&id, value) in &vm.globals {
        let name = vm.interner.resolve(id).to_string();
        if let Some(spec_name) = name.strip_prefix("spec:") {
            if let Value::Object(r) = value {
                spec_entries.push((spec_name.to_string(), *r));
            }
        }
    }
    spec_entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Collect spec metadata for structured BDD specs
    let mut spec_meta: std::collections::HashMap<String, Vec<Value>> =
        std::collections::HashMap::new();
    for (&id, value) in &vm.globals {
        let name = vm.interner.resolve(id).to_string();
        if let Some(meta_name) = name.strip_prefix("spec_meta:") {
            if let Value::Object(r) = value {
                if let Some(list) = vm.heap.get(*r).and_then(|o| o.as_list()) {
                    spec_meta.insert(meta_name.to_string(), list.clone());
                }
            }
        }
    }

    // Run each spec
    let mut results = Vec::new();
    for (spec_name, closure_ref) in spec_entries {
        let arity = get_closure_arity(&vm, closure_ref);

        if arity == 0 {
            // Flat spec: execute once
            match vm.call(Value::Object(closure_ref), &[]) {
                Ok(_) => results.push(SpecResult {
                    name: spec_name,
                    passed: true,
                    error: None,
                    line: None,
                }),
                Err(e) => results.push(SpecResult {
                    name: spec_name,
                    passed: false,
                    error: Some(e.message.clone()),
                    line: e.span.map(|s| s.line),
                }),
            }
        } else {
            // Structured BDD spec: read metadata for path count and labels
            let meta = spec_meta.get(&spec_name);
            let path_count = meta
                .and_then(|m| m.first())
                .and_then(|v| v.as_int())
                .unwrap_or(0) as usize;

            let path_labels: Vec<String> = meta
                .map(|m| {
                    m[1..=path_count]
                        .iter()
                        .map(|v| {
                            if let Value::Object(r) = v {
                                vm.heap
                                    .get(*r)
                                    .and_then(|o| o.as_string())
                                    .unwrap_or("?")
                                    .to_string()
                            } else {
                                "?".to_string()
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Check for `each` clause
            let has_each = arity >= 2;
            let row_count = if has_each {
                meta.and_then(|m| m.last().and_then(|v| v.as_int()))
                    .unwrap_or(0) as usize
            } else {
                1
            };

            for path_idx in 0..path_count {
                let label = path_labels
                    .get(path_idx)
                    .cloned()
                    .unwrap_or_else(|| format!("path {}", path_idx));

                if has_each {
                    for row_idx in 0..row_count {
                        let run_name = format!("{} [{}] (row {})", spec_name, label, row_idx);
                        match vm.call(
                            Value::Object(closure_ref),
                            &[Value::Int(path_idx as i64), Value::Int(row_idx as i64)],
                        ) {
                            Ok(_) => results.push(SpecResult {
                                name: run_name,
                                passed: true,
                                error: None,
                                line: None,
                            }),
                            Err(e) => results.push(SpecResult {
                                name: run_name,
                                passed: false,
                                error: Some(e.message.clone()),
                                line: e.span.map(|s| s.line),
                            }),
                        }
                    }
                } else {
                    let run_name = format!("{} [{}]", spec_name, label);
                    match vm.call(Value::Object(closure_ref), &[Value::Int(path_idx as i64)]) {
                        Ok(_) => results.push(SpecResult {
                            name: run_name,
                            passed: true,
                            error: None,
                            line: None,
                        }),
                        Err(e) => results.push(SpecResult {
                            name: run_name,
                            passed: false,
                            error: Some(e.message.clone()),
                            line: e.span.map(|s| s.line),
                        }),
                    }
                }
            }
        }
    }

    Ok(results)
}

fn run_specs(target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let target_path = match target {
        Some(t) => PathBuf::from(t),
        None => std::env::current_dir()?,
    };

    let files = if target_path.is_file() {
        vec![target_path]
    } else if target_path.is_dir() {
        discover_spec_files(&target_path)
    } else {
        eprintln!("Not a file or directory: {}", target_path.display());
        std::process::exit(1);
    };

    if files.is_empty() {
        println!("No spec files found.");
        return Ok(());
    }

    let mut total_pass = 0usize;
    let mut total_fail = 0usize;

    for file in &files {
        // Display file name
        let display_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        println!("{}", display_name);

        match run_spec_file(file) {
            Ok(results) => {
                for result in &results {
                    if result.passed {
                        println!("  PASS  {}", result.name);
                        total_pass += 1;
                    } else {
                        println!("  FAIL  {}", result.name);
                        if let Some(err) = &result.error {
                            println!("        {}", err);
                        }
                        if let Some(line) = result.line {
                            println!("        at {}:{}", display_name, line);
                        }
                        total_fail += 1;
                    }
                }
            }
            Err(e) => {
                println!("  ERROR {}", e);
                total_fail += 1;
            }
        }
        println!();
    }

    let total = total_pass + total_fail;
    println!(
        "Results: {} passed, {} failed, {} total",
        total_pass, total_fail, total
    );

    if total_fail > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.len() {
        1 => run_repl(),
        2 if args[1] == "run" => {
            eprintln!("Usage: ni run <file.ni>");
            std::process::exit(1);
        }
        2 if args[1] == "lint" => {
            eprintln!("Usage: ni lint <file.ni>");
            std::process::exit(1);
        }
        2 if args[1] == "test" => {
            // ni test -- discover and run all tests from current directory
            if let Err(e) = run_specs(None) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        2 if args[1] == "fmt" => {
            eprintln!("Usage: ni fmt <file.ni> [--write]");
            std::process::exit(1);
        }
        3 if args[1] == "test" => {
            // ni test <file|dir>
            if let Err(e) = run_specs(Some(&args[2])) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        3 if args[1] == "run" => {
            if let Err(e) = run_file(&args[2]) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        3 if args[1] == "lint" => {
            if let Err(e) = run_lint(&args[2]) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        3 if args[1] == "fmt" => {
            if let Err(e) = run_fmt(&args[2], false) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        4 if args[1] == "fmt" && args[3] == "--write" => {
            if let Err(e) = run_fmt(&args[2], true) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        // ni codegen --target rust|c <file.ni>
        5 if args[1] == "codegen" && args[2] == "--target" => {
            if let Err(e) = run_codegen(&args[3], &args[4]) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        2 => {
            // ni <file.ni>
            if let Err(e) = run_file(&args[1]) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage: ni [run] <file.ni>");
            eprintln!("       ni test [file.ni | dir]");
            eprintln!("       ni lint <file.ni>");
            eprintln!("       ni fmt <file.ni> [--write]");
            eprintln!("       ni codegen --target rust|c <file.ni>");
            std::process::exit(1);
        }
    }
}
