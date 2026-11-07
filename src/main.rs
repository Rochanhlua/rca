use rca::compiler::ir::lower;
use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;
use rca::runtime::execution::Interpreter;
use rca::runtime::vm::Vm;
use std::env;
use std::fs;
use std::process;
use std::thread;

fn run_command(command: &str, source: &str) -> i32 {
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

    let ir = match lower::compile(&program) {
        Ok(ir) => ir,
        Err(d) => {
            eprintln!("compile error: {}", d);
            return 1;
        }
    };
    if command == "ir" {
        println!("{}", ir);
        return 0;
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
    let commands = ["lex", "parse", "check", "ir", "run", "run-tree"];
    if args.len() != 3 || !commands.contains(&args[1].as_str()) {
        println!("RCA Programming Language - Built for the Intelligence Era");
        println!("Usage:");
        println!("  rca lex <file.rca>");
        println!("  rca parse <file.rca>");
        println!("  rca check <file.rca>");
        println!("  rca ir <file.rca>        (show bytecode)");
        println!("  rca run <file.rca>       (run on the VM)");
        println!("  rca run-tree <file.rca>  (run on the tree-walking interpreter)");
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
    let handle = thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(move || run_command(&command, &source))
        .expect("failed to start interpreter thread");
    let code = handle.join().unwrap_or(1);
    process::exit(code);
}
