use rca::compiler::ast::StmtKind;
use rca::compiler::ir::lower;
use rca::compiler::ir::validate::validate;
use rca::compiler::lexer::Lexer;
use rca::compiler::modules::load_program;
use rca::compiler::optimizer::{instruction_count, optimize};
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;
use rca::runtime::execution::Interpreter;
use rca::runtime::vm::Vm;
use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::thread;

fn run_command(command: &str, path: &str, source: &str) -> i32 {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    if command == "lex" {
        for t in tokens {
            println!("{}:{}  {:?}", t.span.line, t.span.column, t.kind);
        }
        return 0;
    }

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    if command == "parse" {
        println!("{:#?}", program);
        return 0;
    }

    if program.statements.iter().any(|s| {
        matches!(
            s.kind,
            StmtKind::Import { .. } | StmtKind::FromImport { .. }
        )
    }) {
        return run_with_imports(command, path, source);
    }

    if let Err(errors) = Checker::new().check_program(&program) {
        for e in &errors {
            eprintln!("error: {}", e);
        }
        eprintln!("{} error(s) found", errors.len());
        return 1;
    }
    if command == "check" {
        println!("OK: no errors found");
        return 0;
    }

    if command == "run-tree" {
        let mut interpreter = Interpreter::new();
        return match interpreter.run(&program) {
            Ok(()) => 0,
            Err(d) => {
                eprintln!("runtime error: {}", d);
                1
            }
        };
    }

    let mut ir = match lower::compile(&program) {
        Ok(ir) => ir,
        Err(d) => {
            eprintln!("compile error: {}", d);
            return 1;
        }
    };
    if let Err(e) = validate(&ir) {
        eprintln!("internal compiler error: {}", e);
        return 1;
    }
    if command == "ir" {
        println!("{}", ir);
        return 0;
    }

    let before = instruction_count(&ir);
    optimize(&mut ir);
    if let Err(e) = validate(&ir) {
        eprintln!("internal compiler error (after optimization): {}", e);
        return 1;
    }
    if command == "ir-opt" {
        println!(
            "; optimized: {} -> {} instructions\n",
            before,
            instruction_count(&ir)
        );
        println!("{}", ir);
        return 0;
    }

    if command == "compile" {
        match rca::compiler::codegen::compile(&ir) {
            Ok(obj_bytes) => {
                let out_path = format!("{}.o", path);
                match fs::write(&out_path, obj_bytes) {
                    Ok(_) => println!("Successfully compiled to {}", out_path),
                    Err(e) => {
                        eprintln!("failed to write output file: {}", e);
                        return 1;
                    }
                }
                return 0;
            }
            Err(e) => {
                eprintln!("codegen error: {}", e);
                return 1;
            }
        }
    }

    let mut vm = Vm::new(&ir);
    match vm.run() {
        Ok(()) => 0,
        Err(d) => {
            eprintln!("runtime error: {}", d);
            1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let commands = [
        "lex", "parse", "check", "ir", "ir-opt", "run", "run-tree", "compile",
    ];
    if args.len() != 3 || !commands.contains(&args[1].as_str()) {
        println!("RCA Programming Language - Built for the Intelligence Era");
        println!("Usage:");
        println!("  rca lex <file.rca>");
        println!("  rca parse <file.rca>");
        println!("  rca check <file.rca>");
        println!("  rca ir <file.rca>        (show bytecode)");
        println!("  rca ir-opt <file.rca>    (show optimized bytecode)");
        println!("  rca run <file.rca>       (run on the VM)");
        println!("  rca run-tree <file.rca>  (run on the tree-walking interpreter)");
        println!("  rca compile <file.rca>   (generate native object file)");
        return;
    }

    let source = match fs::read_to_string(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", args[2], e);
            process::exit(1);
        }
    };

    let command = args[1].clone();
    let path = args[2].clone();
    let handle = thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(move || run_command(&command, &path, &source))
        .expect("failed to start interpreter thread");
    let code = handle.join().unwrap_or(1);
    process::exit(code);
}

fn run_with_imports(command: &str, path: &str, source: &str) -> i32 {
    let modules = match load_program(Path::new(path), source) {
        Ok(m) => m,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    if let Err(errors) = Checker::check_modules(&modules) {
        for (name, e) in &errors {
            eprintln!("error in module '{}': {}", name, e);
        }
        eprintln!("{} error(s) found", errors.len());
        return 1;
    }
    if command == "check" {
        println!("OK: no errors found ({} modules)", modules.len());
        return 0;
    }
    if command == "run-tree" {
        eprintln!("error: run-tree does not support import yet; use 'rca run'");
        return 1;
    }
    let mut ir = match lower::compile_modules(&modules) {
        Ok(ir) => ir,
        Err(d) => {
            eprintln!("compile error: {}", d);
            return 1;
        }
    };
    if let Err(e) = validate(&ir) {
        eprintln!("internal compiler error: {}", e);
        return 1;
    }
    if command == "ir" {
        println!("{}", ir);
        return 0;
    }
    let before = instruction_count(&ir);
    optimize(&mut ir);
    if let Err(e) = validate(&ir) {
        eprintln!("internal compiler error (after optimization): {}", e);
        return 1;
    }
    if command == "ir-opt" {
        println!(
            "; optimized: {} -> {} instructions\n",
            before,
            instruction_count(&ir)
        );
        println!("{}", ir);
        return 0;
    }

    if command == "compile" {
        match rca::compiler::codegen::compile(&ir) {
            Ok(obj_bytes) => {
                let out_path = format!("{}.o", path);
                match fs::write(&out_path, obj_bytes) {
                    Ok(_) => println!("Successfully compiled to {}", out_path),
                    Err(e) => {
                        eprintln!("failed to write output file: {}", e);
                        return 1;
                    }
                }
                return 0;
            }
            Err(e) => {
                eprintln!("codegen error: {}", e);
                return 1;
            }
        }
    }

    let mut vm = Vm::new(&ir);
    match vm.run() {
        Ok(()) => 0,
        Err(d) => {
            eprintln!("runtime error: {}", d);
            1
        }
    }
}
