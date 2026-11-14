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
use std::process::{self, Command};
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

    if command == "compile" || command == "build" {
        match rca::compiler::codegen::compile(&ir) {
            Ok(obj_bytes) => {
                let out_path = format!("{}.o", path);
                if let Err(e) = fs::write(&out_path, obj_bytes) {
                    eprintln!("failed to write output file: {}", e);
                    return 1;
                }
                if command == "build" {
                    let exe_path = path.trim_end_matches(".rca");
                    let mut cmd = Command::new("clang");
                    cmd.arg(&out_path).arg("-o").arg(exe_path);
                    #[cfg(not(windows))]
                    cmd.arg("-lm");
                    let status = cmd.status();
                    match status {
                        Ok(s) if s.success() => {
                            println!("Successfully built executable: {}", exe_path)
                        }
                        _ => {
                            eprintln!("failed to link with clang");
                            return 1;
                        }
                    }
                } else {
                    println!("Successfully compiled to {}", out_path);
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
    if args.len() >= 2 {
        match args[1].as_str() {
            "--version" | "-V" | "version" => {
                println!("rcac {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "--help" | "-h" | "help" => {
                print_help();
                process::exit(0);
            }
            _ => {}
        }
    }
    if args.len() >= 2 && args[1] == "pkg" {
        let code = run_pkg(&args[2..]);
        process::exit(code);
    }
    let commands = [
        "lex", "parse", "check", "ir", "ir-opt", "run", "run-tree", "compile", "build",
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
        println!("  rca compile <file.rca>   (generate native object file .o)");
        println!("  rca build <file.rca>     (compile and link to an executable)");
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

    if command == "compile" || command == "build" {
        match rca::compiler::codegen::compile(&ir) {
            Ok(obj_bytes) => {
                let out_path = format!("{}.o", path);
                if let Err(e) = fs::write(&out_path, obj_bytes) {
                    eprintln!("failed to write output file: {}", e);
                    return 1;
                }
                if command == "build" {
                    let exe_path = path.trim_end_matches(".rca");
                    let mut cmd = Command::new("clang");
                    cmd.arg(&out_path).arg("-o").arg(exe_path);
                    #[cfg(not(windows))]
                    cmd.arg("-lm");
                    let status = cmd.status();
                    match status {
                        Ok(s) if s.success() => {
                            println!("Successfully built executable: {}", exe_path)
                        }
                        _ => {
                            eprintln!("failed to link with clang");
                            return 1;
                        }
                    }
                } else {
                    println!("Successfully compiled to {}", out_path);
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

/// Package manager entry point. Called as `rcac pkg <action> [args...]`.
fn run_pkg(args: &[String]) -> i32 {
    let action = match args.first() {
        Some(a) => a.as_str(),
        None => {
            eprintln!("usage: rcac pkg <init|add|install>");
            return 1;
        }
    };
    match action {
        "init" => pkg_init(),
        "add" => pkg_add(&args[1..]),
        "install" => pkg_install(),
        _ => {
            eprintln!("unknown pkg action: {}", action);
            eprintln!("usage: rcac pkg <init|add|install>");
            1
        }
    }
}

fn pkg_init() -> i32 {
    let manifest = std::path::Path::new("rca.json");
    if manifest.exists() {
        eprintln!("rca.json already exists in this directory");
        return 1;
    }
    let name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "rca-project".to_string());
    let json = format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"dependencies\": {{}}\n}}\n",
        name
    );
    match std::fs::write(manifest, json) {
        Ok(()) => {
            println!("created rca.json (name: {})", name);
            0
        }
        Err(e) => {
            eprintln!("cannot write rca.json: {}", e);
            1
        }
    }
}

fn pkg_add(args: &[String]) -> i32 {
    if args.len() != 2 {
        eprintln!("usage: rcac pkg add <name> <source>");
        return 1;
    }
    let dep_name = &args[0];
    let dep_src = &args[1];
    let manifest_path = std::path::Path::new("rca.json");
    if !manifest_path.exists() {
        eprintln!("no rca.json in this directory (run `rcac pkg init` first)");
        return 1;
    }
    let content = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot read rca.json: {}", e);
            return 1;
        }
    };
    let value = match rca::runtime::json::parse(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid rca.json: {}", e);
            return 1;
        }
    };
    let mut map = match value {
        rca::runtime::values::Value::Map(m) => m,
        _ => {
            eprintln!("rca.json must be a JSON object");
            return 1;
        }
    };
    let dep_key = rca::runtime::values::Value::Str("dependencies".to_string());
    let mut found = false;
    for (k, v) in map.iter_mut() {
        if k == &dep_key {
            if let rca::runtime::values::Value::Map(deps) = v {
                let name_v = rca::runtime::values::Value::Str(dep_name.clone());
                let src_v = rca::runtime::values::Value::Str(dep_src.clone());
                let mut updated = false;
                for (dk, dv) in deps.iter_mut() {
                    if dk == &name_v {
                        *dv = src_v.clone();
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    deps.push((name_v, src_v));
                }
                found = true;
            }
            break;
        }
    }
    if !found {
        eprintln!("rca.json has no \"dependencies\" object");
        return 1;
    }
    let new_value = rca::runtime::values::Value::Map(map);
    let new_json = match rca::runtime::json::stringify(&new_value) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot stringify rca.json: {}", e);
            return 1;
        }
    };
    match std::fs::write(manifest_path, format!("{}\n", new_json)) {
        Ok(()) => {
            println!("added dependency: {} -> {}", dep_name, dep_src);
            0
        }
        Err(e) => {
            eprintln!("cannot write rca.json: {}", e);
            1
        }
    }
}

fn pkg_home() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".rca").join("packages")
}

fn pkg_install() -> i32 {
    let manifest_path = std::path::Path::new("rca.json");
    if !manifest_path.exists() {
        eprintln!("no rca.json in this directory (run `rcac pkg init` first)");
        return 1;
    }
    let content = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot read rca.json: {}", e);
            return 1;
        }
    };
    let value = match rca::runtime::json::parse(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid rca.json: {}", e);
            return 1;
        }
    };
    let map = match value {
        rca::runtime::values::Value::Map(m) => m,
        _ => {
            eprintln!("rca.json must be a JSON object");
            return 1;
        }
    };
    let dep_key = rca::runtime::values::Value::Str("dependencies".to_string());
    let mut deps: Vec<(String, String)> = Vec::new();
    for (k, v) in map.iter() {
        if k == &dep_key
            && let rca::runtime::values::Value::Map(d) = v
        {
            for (dk, dv) in d.iter() {
                if let (rca::runtime::values::Value::Str(n), rca::runtime::values::Value::Str(s)) =
                    (dk, dv)
                {
                    deps.push((n.clone(), s.clone()));
                }
            }
        }
    }
    if deps.is_empty() {
        println!("no dependencies to install");
        return 0;
    }
    let home = pkg_home();
    if let Err(e) = std::fs::create_dir_all(&home) {
        eprintln!("cannot create {}: {}", home.display(), e);
        return 1;
    }
    let mut ok_count = 0;
    let mut fail_count = 0;
    for (name, source) in &deps {
        match fetch_dependency(name, source) {
            Ok(body) => {
                let dest_dir = home.join(name);
                if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                    eprintln!("  {} : cannot create {}: {}", name, dest_dir.display(), e);
                    fail_count += 1;
                    continue;
                }
                let dest = dest_dir.join("main.rca");
                match std::fs::write(&dest, &body) {
                    Ok(()) => {
                        println!("  {} : installed -> {}", name, dest.display());
                        ok_count += 1;
                    }
                    Err(e) => {
                        eprintln!("  {} : cannot write {}: {}", name, dest.display(), e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("  {} : {}", name, e);
                fail_count += 1;
            }
        }
    }
    println!(
        "installed {} / {} ({} failed)",
        ok_count,
        deps.len(),
        fail_count
    );
    if fail_count == 0 { 0 } else { 1 }
}

fn fetch_dependency(name: &str, source: &str) -> Result<String, String> {
    if source.starts_with("github:") {
        return Err(format!(
            "github: sources not yet supported ({}); use http:// or a local path for now",
            source
        ));
    }
    if source.starts_with("https://") {
        return Err("https:// not yet supported (no TLS); use http:// or a local path".to_string());
    }
    if source.starts_with("http://") {
        return rca::runtime::http::http_get(source).map_err(|e| format!("fetch failed: {}", e));
    }
    // local path
    std::fs::read_to_string(source)
        .map_err(|e| format!("cannot read {}: {}", source, e))
        .map_err(|e| format!("{} : {}", name, e))
}

fn print_help() {
    println!(
        "RCA Programming Language {} - Built for the Intelligence Era",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("Usage:");
    println!("  rcac <command> <file.rca> [args...]");
    println!("  rcac pkg <action> [args...]");
    println!();
    println!("Commands:");
    println!("  lex        show tokens");
    println!("  parse      show the syntax tree");
    println!("  check      type check only");
    println!("  ir         show bytecode");
    println!("  ir-opt     show optimized bytecode");
    println!("  run        run on the bytecode VM");
    println!("  run-tree   run on the tree-walking interpreter");
    println!("  compile    generate a native object file (<file>.rca.o)");
    println!("  build      compile and link a native executable (needs clang)");
    println!("  pkg        package manager: init | add | install");
    println!();
    println!("Meta:");
    println!("  --version  print version and exit");
    println!("  --help     print this help and exit");
    println!();
    println!("Docs: docs/cli.md, docs/SYNTAX.md, docs/packages.md");
}
