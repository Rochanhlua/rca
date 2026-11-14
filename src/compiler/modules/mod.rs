//! Module loading: finds, reads and parses imported .rca files.
//! Modules are returned in dependency order (imported first, entry file last).

use crate::compiler::ast::{Program, StmtKind};
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub program: Program,
}

struct Loader {
    modules: Vec<Module>,
    loading: Vec<PathBuf>,
    done: HashSet<PathBuf>,
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string()
}

fn parse_source(source: &str) -> Result<Program, Diagnostic> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::new(tokens).parse_program()
}

pub fn load_program(entry: &Path, entry_source: &str) -> Result<Vec<Module>, Diagnostic> {
    let program = parse_source(entry_source)?;
    let path = fs::canonicalize(entry).unwrap_or_else(|_| entry.to_path_buf());
    let name = stem(&path);
    let mut loader = Loader {
        modules: Vec::new(),
        loading: Vec::new(),
        done: HashSet::new(),
    };
    loader.visit(path, name, program)?;
    Ok(loader.modules)
}

impl Loader {
    fn visit(&mut self, path: PathBuf, name: String, program: Program) -> Result<(), Diagnostic> {
        self.loading.push(path.clone());
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();

        for stmt in &program.statements {
            let module = match &stmt.kind {
                StmtKind::Import { module, .. } | StmtKind::FromImport { module, .. } => module,
                _ => continue,
            };
            if crate::compiler::stdlib::is_stdlib_module(module) {
                continue;
            }

            let target = dir.join(format!("{}.rca", module));
            let canonical = match fs::canonicalize(&target) {
                Ok(p) => p,
                Err(_) => {
                    if let Some(pkg_path) = package_path(module) {
                        match fs::canonicalize(&pkg_path) {
                            Ok(p) => p,
                            Err(_) => {
                                return Err(Diagnostic::new(
                                    format!(
                                        "cannot find module '{}' (looked for {} and {})",
                                        module,
                                        target.display(),
                                        pkg_path.display()
                                    ),
                                    stmt.span.clone(),
                                ));
                            }
                        }
                    } else {
                        return Err(Diagnostic::new(
                            format!(
                                "cannot find module '{}' (looked for {})",
                                module,
                                target.display()
                            ),
                            stmt.span.clone(),
                        ));
                    }
                }
            };

            if let Some(pos) = self.loading.iter().position(|p| p == &canonical) {
                let mut chain: Vec<String> =
                    self.loading.iter().skip(pos).map(|p| stem(p)).collect();
                chain.push(module.clone());
                return Err(Diagnostic::new(
                    format!("circular import: {}", chain.join(" -> ")),
                    stmt.span.clone(),
                ));
            }

            if self.done.contains(&canonical) {
                continue;
            }

            let source = match fs::read_to_string(&canonical) {
                Ok(s) => s,
                Err(e) => {
                    return Err(Diagnostic::new(
                        format!("cannot read module '{}': {}", module, e),
                        stmt.span.clone(),
                    ));
                }
            };
            let child = match parse_source(&source) {
                Ok(p) => p,
                Err(d) => {
                    return Err(Diagnostic::new(
                        format!("in module '{}': {}", module, d),
                        stmt.span.clone(),
                    ));
                }
            };
            self.visit(canonical, module.clone(), child)?;
        }

        self.loading.pop();
        self.done.insert(path.clone());
        self.modules.push(Module {
            name,
            path,
            program,
        });
        Ok(())
    }
}

/// If `$HOME/.rca/packages/<module>/main.rca` exists, return its path.
fn package_path(module: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home)
        .join(".rca")
        .join("packages")
        .join(module)
        .join("main.rca");
    if p.exists() { Some(p) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("module-tests")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, file: &str, src: &str) {
        fs::write(dir.join(file), src).unwrap();
    }

    fn load(dir: &Path, entry: &str) -> Result<Vec<Module>, Diagnostic> {
        let path = dir.join(entry);
        let src = fs::read_to_string(&path).unwrap();
        load_program(&path, &src)
    }

    const ADD: &str = "pub fn add(a: Int, b: Int) -> Int:\n    return a + b\n";

    #[test]
    fn dependency_is_loaded_before_entry() {
        let dir = make_dir("single");
        write(&dir, "main.rca", "import utils\n");
        write(&dir, "utils.rca", ADD);
        let mods = load(&dir, "main.rca").unwrap();
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].name, "utils");
        assert_eq!(mods[1].name, "main");
    }

    #[test]
    fn missing_module_is_error() {
        let dir = make_dir("missing");
        write(&dir, "main.rca", "import nothing\n");
        let err = load(&dir, "main.rca").unwrap_err().to_string();
        assert!(err.contains("cannot find module 'nothing'"), "got: {}", err);
    }

    #[test]
    fn circular_import_is_error() {
        let dir = make_dir("cycle");
        write(&dir, "a.rca", "import b\n");
        write(&dir, "b.rca", "import a\n");
        let err = load(&dir, "a.rca").unwrap_err().to_string();
        assert!(err.contains("circular import"), "got: {}", err);
    }

    #[test]
    fn shared_module_is_loaded_once() {
        let dir = make_dir("shared");
        write(&dir, "main.rca", "import a\nimport b\n");
        write(&dir, "a.rca", "import common\n");
        write(&dir, "b.rca", "import common\n");
        write(&dir, "common.rca", ADD);
        let mods = load(&dir, "main.rca").unwrap();
        assert_eq!(mods.len(), 4);
        assert_eq!(mods.iter().filter(|m| m.name == "common").count(), 1);
    }

    #[test]
    fn syntax_error_in_module_is_reported() {
        let dir = make_dir("syntax");
        write(&dir, "main.rca", "import bad\n");
        write(&dir, "bad.rca", "let = 5\n");
        let err = load(&dir, "main.rca").unwrap_err().to_string();
        assert!(err.contains("in module 'bad'"), "got: {}", err);
    }
}
