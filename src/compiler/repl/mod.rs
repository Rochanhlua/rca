//! Minimal REPL (dev tool #4).
//!
//! Behavior:
//! - Reads lines from stdin until EOF or a `:quit` line.
//! - Each non-empty line is parsed as a full program and executed on a
//!   fresh VM. State is **not** preserved between lines (a future version
//!   will keep a shared scope and a persistent VM).
//! - `print(...)` is the way to see output; there is no auto-print.
//! - Errors are printed and the loop continues.
//!
//! Limitations (documented on purpose):
//! - Multi-line constructs (blocks, functions) are not supported yet.
//!   A single line must be a complete program.
//! - No auto-print of expression values.
//! - No history, no completion, no syntax highlighting.

use crate::compiler::ir::lower;
use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use crate::compiler::semantic::Checker;
use crate::runtime::vm::Vm;
use std::io::{self, Write};

pub fn run_repl() -> i32 {
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("rca> ");
        if io::stdout().flush().is_err() {
            eprintln!("cannot flush stdout");
            return 1;
        }
        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                return 0;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {}", e);
                return 1;
            }
        }
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.trim().is_empty() {
            continue;
        }
        if trimmed == ":quit" || trimmed == ":q" {
            return 0;
        }
        if trimmed == ":help" {
            println!("commands:");
            println!("  :quit / :q    exit");
            println!("  :help         this message");
            println!("each other line is a full rca program; use print(...)");
            continue;
        }
        match run_one(trimmed) {
            Ok(()) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}

fn run_one(source: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize().map_err(|d| d.to_string())?;
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|d| d.to_string())?;
    if let Err(errs) = Checker::new().check_program(&program) {
        let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        return Err(msgs.join("\n"));
    }
    let ir = lower::compile(&program).map_err(|d| d.to_string())?;
    let mut vm = Vm::new(&ir);
    vm.run().map_err(|d| d.to_string())
}
