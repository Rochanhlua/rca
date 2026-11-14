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

    if command == "fmt" {
        let fmt_tokens = match Lexer::new(source).tokenize() {
            Ok(t) => t,
            Err(d) => {
                eprintln!("error: {}", d);
                return 1;
            }
        };
        return match rca::compiler::fmt::format_tokens(&fmt_tokens) {
            Ok(s) => {
                print!("{}", s);
                0
            }
            Err((msg, span)) => {
                eprintln!("{}:{}: {}", span.line, span.column, msg);
                1
            }
        };
    }

    if command == "doc" {
        print!("{}", rca::compiler::doc::generate(source, &program));
        return 0;
    }
    if command == "lint" {
        let warnings = rca::compiler::lint::lint_program(&program);
        for w in &warnings {
            eprintln!("{}", w);
        }
        eprintln!("{} warning(s)", warnings.len());
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
    if args.len() >= 2 && args[1] == "bench" {
        let code = rcac_bench(&args[2..]);
        process::exit(code);
    }
    if args.len() >= 3 && args[1] == "watch" {
        let code = rcac_watch(&args[2]);
        process::exit(code);
    }
    if args.len() >= 3 && args[1] == "profile" {
        let code = rcac_profile(&args[2]);
        process::exit(code);
    }
    if args.len() >= 3 && args[1] == "coverage" {
        let code = rcac_coverage(&args[2]);
        process::exit(code);
    }
    if args.len() >= 2 && args[1] == "completion" {
        let code = rcac_completion(&args[2..]);
        process::exit(code);
    }
    if args.len() >= 3 && args[1] == "test" {
        let code = rcac_test(&args[2]);
        process::exit(code);
    }
    if args.len() == 2 && args[1] == "repl" {
        let code = rca::compiler::repl::run_repl();
        process::exit(code);
    }
    if args.len() == 2 && args[1] == "lsp" {
        let code = rca::compiler::lsp::run_lsp();
        process::exit(code);
    }
    let commands = [
        "lex",
        "parse",
        "check",
        "ir",
        "ir-opt",
        "run",
        "run-tree",
        "compile",
        "build",
        "lint",
        "test",
        "fmt",
        "doc",
        "bench",
        "watch",
        "completion",
        "profile",
        "coverage",
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
            "github: sources not yet supported ({}); use http://, https://, or a local path for now",
            source
        ));
    }
    if source.starts_with("http://") || source.starts_with("https://") {
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
    println!("  lint       report warnings (unused vars, unreachable code)");
    println!("  test       run all .rca files in tests/rca/ (or a given dir)");
    println!("  fmt        print canonical formatting of a source file");
    println!("  repl       interactive read-eval-print loop (Ctrl+D or :quit)");
    println!("  lsp        language server (JSON-RPC over stdio for editors)");
    println!("  doc        print Markdown API documentation to stdout");
    println!("  bench      run a file N times and print min/avg/max (default N=5)");
    println!("  watch      re-run a file whenever it changes (Ctrl+C to stop)");
    println!("  completion print shell completion (bash|zsh|fish)");
    println!("  profile    run a file with instrumentation and print a report");
    println!("  coverage   run a file and report which lines were executed");
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

fn rcac_test(dir_or_file: &str) -> i32 {
    use std::path::Path;
    // Treat the argument as a directory unless it ends in .rca
    let test_dir = if dir_or_file.ends_with(".rca") {
        Path::new(dir_or_file)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    } else {
        Path::new(dir_or_file).to_path_buf()
    };
    let entries = match std::fs::read_dir(&test_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "cannot read {}: {} (create tests/rca/ or pass a directory)",
                test_dir.display(),
                e
            );
            return 1;
        }
    };
    let mut tests: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "rca").unwrap_or(false))
        .collect();
    tests.sort();
    if tests.is_empty() {
        println!("no .rca test files in {}", test_dir.display());
        return 0;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot locate rcac binary: {}", e);
            return 1;
        }
    };
    let mut pass = 0usize;
    let mut fail = 0usize;
    for t in &tests {
        let out = match std::process::Command::new(&exe).arg("run").arg(t).output() {
            Ok(o) => o,
            Err(e) => {
                println!("ERROR {}: {}", t.display(), e);
                fail += 1;
                continue;
            }
        };
        if out.status.success() {
            println!("PASS  {}", t.display());
            pass += 1;
        } else {
            println!("FAIL  {}", t.display());
            let err = String::from_utf8_lossy(&out.stderr);
            for line in err.lines() {
                eprintln!("      {}", line);
            }
            fail += 1;
        }
    }
    println!();
    println!("{} passed, {} failed ({} total)", pass, fail, tests.len());
    if fail == 0 { 0 } else { 1 }
}

fn rcac_bench(args: &[String]) -> i32 {
    // Usage: rcac bench <file.rca> [iterations]
    if args.is_empty() {
        eprintln!("usage: rcac bench <file.rca> [iterations]");
        return 1;
    }
    let path = &args[0];
    let iters: u32 = if args.len() >= 2 {
        match args[1].parse() {
            Ok(n) if n >= 1 => n,
            _ => {
                eprintln!("iterations must be a positive integer");
                return 1;
            }
        }
    } else {
        5
    };
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot locate rcac binary: {}", e);
            return 1;
        }
    };
    // Warmup (compile + run once, not counted).
    match std::process::Command::new(&exe)
        .arg("run")
        .arg(path)
        .output()
    {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            eprintln!("warmup failed (exit {:?})", o.status.code());
            let stderr = String::from_utf8_lossy(&o.stderr);
            for line in stderr.lines() {
                eprintln!("  {}", line);
            }
            return 1;
        }
        Err(e) => {
            eprintln!("warmup failed: {}", e);
            return 1;
        }
    }
    let mut times_ms: Vec<f64> = Vec::with_capacity(iters as usize);
    for _ in 0..iters {
        let start = std::time::Instant::now();
        let out = match std::process::Command::new(&exe)
            .arg("run")
            .arg(path)
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("run failed: {}", e);
                return 1;
            }
        };
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if !out.status.success() {
            eprintln!("run failed (exit {:?})", out.status.code());
            let stderr = String::from_utf8_lossy(&out.stderr);
            for line in stderr.lines() {
                eprintln!("  {}", line);
            }
            return 1;
        }
        times_ms.push(elapsed);
    }
    let n = times_ms.len();
    if n == 0 {
        eprintln!("no iterations ran");
        return 1;
    }
    let sum: f64 = times_ms.iter().sum();
    let avg = sum / n as f64;
    let min = times_ms.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = times_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    println!("bench: {}", path);
    println!("  iterations: {}", n);
    println!("  min: {:.2} ms", min);
    println!("  avg: {:.2} ms", avg);
    println!("  max: {:.2} ms", max);
    println!(
        "  all: {}",
        times_ms
            .iter()
            .map(|t| format!("{:.2}", t))
            .collect::<Vec<_>>()
            .join(" ms, ")
            + " ms"
    );
    0
}

fn rcac_watch(path: &str) -> i32 {
    use std::time::{Duration, SystemTime};
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cannot locate rcac binary: {}", e);
            return 1;
        }
    };
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cannot stat {}: {}", path, e);
            return 1;
        }
    };
    let mut last_mtime: SystemTime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    println!("watching {} (Ctrl+C to stop)", path);
    // Initial run
    run_watched(&exe, path);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let mtime = match std::fs::metadata(path)
            .and_then(|m| m.modified().or(Ok(SystemTime::UNIX_EPOCH)))
        {
            Ok(t) => t,
            Err(_) => {
                // File disappeared; wait for it to come back.
                continue;
            }
        };
        if mtime != last_mtime {
            last_mtime = mtime;
            println!("\n--- change detected, re-running ---");
            run_watched(&exe, path);
        }
    }
}

fn run_watched(exe: &std::path::Path, path: &str) {
    let out = match std::process::Command::new(exe)
        .arg("run")
        .arg(path)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("run failed: {}", e);
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    print!("{}", stdout);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }
    if !out.status.success() {
        eprintln!("[exit {:?}]", out.status.code());
    }
}

const BASH_COMPLETION: &str = include_str!("../assets/completion/bash.sh");
const ZSH_COMPLETION: &str = include_str!("../assets/completion/zsh.sh");
const FISH_COMPLETION: &str = include_str!("../assets/completion/fish.sh");

fn rcac_completion(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: rcac completion <bash|zsh|fish>");
        return 1;
    }
    match args[0].as_str() {
        "bash" => {
            print!("{}", BASH_COMPLETION);
            0
        }
        "zsh" => {
            print!("{}", ZSH_COMPLETION);
            0
        }
        "fish" => {
            print!("{}", FISH_COMPLETION);
            0
        }
        other => {
            eprintln!("unsupported shell '{}' (try bash|zsh|fish)", other);
            1
        }
    }
}

fn rcac_profile(path: &str) -> i32 {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", path, e);
            return 1;
        }
    };
    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    if program.statements.iter().any(|s| {
        matches!(
            s.kind,
            StmtKind::Import { .. } | StmtKind::FromImport { .. }
        )
    }) {
        eprintln!("error: rcac profile does not support imports yet");
        return 1;
    }
    if let Err(errors) = Checker::new().check_program(&program) {
        for e in &errors {
            eprintln!("error: {}", e);
        }
        return 1;
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
    optimize(&mut ir);

    let mut vm = Vm::with_stats(&ir);
    if let Err(d) = vm.run() {
        eprintln!("runtime error: {}", d);
        return 1;
    }
    let stats = match vm.stats() {
        Some(s) => s,
        None => {
            eprintln!("profile: stats were not collected");
            return 1;
        }
    };

    println!("profile: {}", path);
    println!("  instructions: {}", stats.instruction_count);

    let mut funcs: Vec<(&String, &u64)> = stats.function_time_ns.iter().collect();
    funcs.sort_by(|a, b| b.1.cmp(a.1));
    println!();
    println!("  functions by time:");
    if funcs.is_empty() {
        println!("    (no function calls recorded)");
    } else {
        for (name, ns) in funcs.iter().take(10) {
            let calls = stats.function_calls.get(*name).copied().unwrap_or(0);
            println!(
                "    {:30} {:>10.3} ms  ({} calls)",
                name,
                **ns as f64 / 1_000_000.0,
                calls
            );
        }
    }

    let mut lines: Vec<(&(usize, usize), &u64)> = stats.line_hits.iter().collect();
    lines.sort_by(|a, b| b.1.cmp(a.1));
    println!();
    println!("  hottest lines:");
    for ((ln, col), hits) in lines.iter().take(10) {
        println!("    {}:{}  {} hits", ln, col, hits);
    }
    0
}

fn rcac_coverage(path: &str) -> i32 {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", path, e);
            return 1;
        }
    };
    let total_lines = source.lines().count();
    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(d) => {
            eprintln!("error: {}", d);
            return 1;
        }
    };
    if program.statements.iter().any(|s| {
        matches!(
            s.kind,
            StmtKind::Import { .. } | StmtKind::FromImport { .. }
        )
    }) {
        eprintln!("error: rcac coverage does not support imports yet");
        return 1;
    }
    if let Err(errors) = Checker::new().check_program(&program) {
        for e in &errors {
            eprintln!("error: {}", e);
        }
        return 1;
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
    optimize(&mut ir);

    let mut vm = Vm::with_stats(&ir);
    if let Err(d) = vm.run() {
        eprintln!("runtime error: {}", d);
        return 1;
    }
    let stats = match vm.stats() {
        Some(s) => s,
        None => {
            eprintln!("coverage: stats were not collected");
            return 1;
        }
    };

    // Build a set of line numbers that had at least one executed instruction.
    let mut hit_lines: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for (ln, _col) in stats.line_hits.keys() {
        hit_lines.insert(*ln);
    }

    // Classify every non-empty source line as covered or missed.
    let source_lines: Vec<&str> = source.lines().collect();
    let mut covered = 0usize;
    let mut missed = 0usize;
    let mut missed_list: Vec<usize> = Vec::new();
    for (idx, line) in source_lines.iter().enumerate() {
        let ln = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if hit_lines.contains(&ln) {
            covered += 1;
        } else {
            missed += 1;
            missed_list.push(ln);
        }
    }
    let total_code = covered + missed;
    let pct = if total_code == 0 {
        100.0
    } else {
        (covered as f64 / total_code as f64) * 100.0
    };

    println!("coverage: {}", path);
    println!("  source lines: {}", total_lines);
    println!("  code lines:   {}", total_code);
    println!("  covered:      {}", covered);
    println!("  missed:       {}", missed);
    println!("  coverage:     {:.1}%", pct);
    if missed > 0 && missed <= 40 {
        println!();
        println!("  missed lines:");
        for ln in &missed_list {
            if let Some(text) = source_lines.get(ln - 1) {
                println!("    {}: {}", ln, text);
            } else {
                println!("    {}", ln);
            }
        }
    } else if missed > 40 {
        println!();
        println!("  missed lines: (too many to list; {} total)", missed);
    }
    0
}
