use rca::compiler::lexer::Lexer;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 || args[1] != "lex" {
        println!("RCA Programming Language - Built for the Intelligence Era");
        println!("Usage: rca lex <file.rca>");
        return;
    }
    let source = match fs::read_to_string(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", args[2], e);
            std::process::exit(1);
        }
    };
    match Lexer::new(&source).tokenize() {
        Ok(tokens) => {
            for t in tokens {
                println!("{}:{}  {:?}", t.span.line, t.span.column, t.kind);
            }
        }
        Err(d) => {
            eprintln!("error: {}", d);
            std::process::exit(1);
        }
    }
}
