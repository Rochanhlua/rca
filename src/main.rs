use rca::compiler::lexer::Lexer;
use rca::compiler::parser::Parser;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 || (args[1] != "lex" && args[1] != "parse") {
        println!("RCA Programming Language - Built for the Intelligence Era");
        println!("Usage:");
        println!("  rca lex <file.rca>");
        println!("  rca parse <file.rca>");
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

    match Parser::new(tokens).parse_program() {
        Ok(program) => println!("{:#?}", program),
        Err(d) => {
            eprintln!("error: {}", d);
            process::exit(1);
        }
    }
}
