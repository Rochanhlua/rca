use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use rca::compiler::semantic::Checker;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let commands = ["lex", "parse", "check"];
    if args.len() != 3 || !commands.contains(&args[1].as_str()) {
        println!("RCA Programming Language - Built for the Intelligence Era");
        println!("Usage:");
        println!("  rca lex <file.rca>");
        println!("  rca parse <file.rca>");
        println!("  rca check <file.rca>");
        return;
    }

    let source = match fs::read_to_string(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", args[2], e);
            process::exit(1);
        }
    };

    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(d) => {
            eprintln!("error: {}", d);
            process::exit(1);
        }
    };

    if args[1] == "lex" {
        for t in tokens {
            println!("{}:{}  {:?}", t.span.line, t.span.column, t.kind);
        }
        return;
    }

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(d) => {
            eprintln!("error: {}", d);
            process::exit(1);
        }
    };

    if args[1] == "parse" {
        println!("{:#?}", program);
        return;
    }

    match Checker::new().check_program(&program) {
        Ok(()) => println!("OK: no errors found"),
        Err(errors) => {
            for e in &errors {
                eprintln!("error: {}", e);
            }
            eprintln!("{} error(s) found", errors.len());
            process::exit(1);
        }
    }
}
