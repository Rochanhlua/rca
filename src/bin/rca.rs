use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: rca <file.rca>");
        exit(1);
    }
    let rcac = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("rcac")))
        .unwrap_or_else(|| "rcac".into());
    let status = Command::new(&rcac)
        .args(if matches!(args[1].as_str(), "--help" | "-h" | "help" | "--version" | "-V" | "version") { Vec::<String>::new() } else { vec!["run".to_string()] })
        .args(&args[1..])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("rca: failed to launch rcac: {e}");
            exit(1);
        });
    exit(status.code().unwrap_or(1));
}
