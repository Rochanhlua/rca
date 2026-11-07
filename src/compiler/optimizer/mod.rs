use crate::compiler::ast::BinaryOp;
use crate::compiler::ir::{Chunk, Instruction, IrProgram};
use crate::compiler::lexer::token::Span;
use crate::runtime::execution::Interpreter;
use crate::runtime::values::Value;

/// Total number of instructions in the program.
pub fn instruction_count(program: &IrProgram) -> usize {
    program.main.chunk.code.len() + program.functions.iter().map(|f| f.chunk.code.len()).sum::<usize>()
}

/// Optimizes every function of the program. Program behavior never changes.
pub fn optimize(program: &mut IrProgram) {
    optimize_chunk(&mut program.main.chunk);
    for f in program.functions.iter_mut() {
        optimize_chunk(&mut f.chunk);
    }
}

fn optimize_chunk(chunk: &mut Chunk) {
    loop {
        let folded = fold(chunk);
        let removed = remove_unreachable(chunk);
        if !folded && !removed {
            break;
        }
    }
}

fn jump_targets(code: &[Instruction]) -> Vec<bool> {
    let mut targets = vec![false; code.len() + 1];
    for ins in code {
        match ins {
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) => targets[*t] = true,
            Instruction::ForNext { exit, .. } => targets[*exit] = true,
            _ => {}
        }
    }
    targets
}

/// Rewrites jump targets after instructions were removed.
fn remap(code: &mut [Instruction], map: &[usize]) {
    for ins in code.iter_mut() {
        match ins {
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) => *t = map[*t],
            Instruction::ForNext { exit, .. } => *exit = map[*exit],
            _ => {}
        }
    }
}

fn constant(ins: &Instruction) -> Option<Value> {
    match ins {
        Instruction::PushInt(n) => Some(Value::Int(*n)),
        Instruction::PushFloat(x) => Some(Value::Float(*x)),
        Instruction::PushBool(b) => Some(Value::Bool(*b)),
        Instruction::PushStr(s) => Some(Value::Str(s.clone())),
        _ => None,
    }
}

fn to_instruction(v: Value) -> Option<Instruction> {
    match v {
        Value::Int(n) => Some(Instruction::PushInt(n)),
        Value::Float(x) => Some(Instruction::PushFloat(x)),
        Value::Bool(b) => Some(Instruction::PushBool(b)),
        Value::Str(s) => Some(Instruction::PushStr(s)),
        _ => None,
    }
}

/// `Push a; Push b; <op>` becomes a single push, when the operation succeeds.
fn fold_binary(a: &Instruction, b: &Instruction, c: &Instruction, span: &Span) -> Option<Instruction> {
    let op = match c {
        Instruction::Add => BinaryOp::Add,
        Instruction::Sub => BinaryOp::Sub,
        Instruction::Mul => BinaryOp::Mul,
        Instruction::Div => BinaryOp::Div,
        Instruction::Mod => BinaryOp::Mod,
        Instruction::Eq => BinaryOp::Eq,
        Instruction::NotEq => BinaryOp::NotEq,
        Instruction::Lt => BinaryOp::Lt,
        Instruction::LtEq => BinaryOp::LtEq,
        Instruction::Gt => BinaryOp::Gt,
        Instruction::GtEq => BinaryOp::GtEq,
        _ => return None,
    };
    let l = constant(a)?;
    let r = constant(b)?;
    match Interpreter::apply_binary(&op, l, r, span) {
        Ok(v) => to_instruction(v),
        Err(_) => None, // keep it, so the runtime error still happens
    }
}

fn fold_pair(a: &Instruction, b: &Instruction) -> Option<Vec<Instruction>> {
    match (a, b) {
        (Instruction::PushInt(n), Instruction::Neg) => n.checked_neg().map(|v| vec![Instruction::PushInt(v)]),
        (Instruction::PushFloat(x), Instruction::Neg) => Some(vec![Instruction::PushFloat(-x)]),
        (Instruction::PushBool(v), Instruction::Not) => Some(vec![Instruction::PushBool(!v)]),
        (Instruction::PushBool(true), Instruction::JumpIfFalse(_)) => Some(Vec::new()),
        (Instruction::PushBool(false), Instruction::JumpIfFalse(t)) => Some(vec![Instruction::Jump(*t)]),
        _ => None,
    }
}

/// One folding pass. Returns true if anything changed.
fn fold(chunk: &mut Chunk) -> bool {
    let n = chunk.code.len();
    let targets = jump_targets(&chunk.code);
    let mut new_code: Vec<Instruction> = Vec::with_capacity(n);
    let mut new_spans: Vec<Span> = Vec::with_capacity(n);
    let mut map = vec![0usize; n + 1];
    let mut changed = false;

    let mut i = 0;
    while i < n {
        map[i] = new_code.len();

        // a jump to the very next instruction does nothing
        if let Instruction::Jump(t) = &chunk.code[i] {
            if *t == i + 1 {
                changed = true;
                i += 1;
                continue;
            }
        }

        // never fold across an instruction that something else jumps to
        if i + 2 < n && !targets[i + 1] && !targets[i + 2] {
            if let Some(ins) = fold_binary(
                &chunk.code[i],
                &chunk.code[i + 1],
                &chunk.code[i + 2],
                &chunk.spans[i + 2],
            ) {
                map[i + 1] = new_code.len();
                map[i + 2] = new_code.len();
                new_code.push(ins);
                new_spans.push(chunk.spans[i + 2].clone());
                i += 3;
                changed = true;
                continue;
            }
        }

        if i + 1 < n && !targets[i + 1] {
            if let Some(replacement) = fold_pair(&chunk.code[i], &chunk.code[i + 1]) {
                map[i + 1] = new_code.len();
                for r in replacement {
                    new_code.push(r);
                    new_spans.push(chunk.spans[i + 1].clone());
                }
                i += 2;
                changed = true;
                continue;
            }
        }

        new_code.push(chunk.code[i].clone());
        new_spans.push(chunk.spans[i].clone());
        i += 1;
    }
    map[n] = new_code.len();

    if !changed {
        return false;
    }
    remap(&mut new_code, &map);
    chunk.code = new_code;
    chunk.spans = new_spans;
    true
}

/// Removes instructions that can never run.
fn remove_unreachable(chunk: &mut Chunk) -> bool {
    let n = chunk.code.len();
    let mut reachable = vec![false; n];
    let mut work = vec![0usize];
    while let Some(i) = work.pop() {
        if i >= n || reachable[i] {
            continue;
        }
        reachable[i] = true;
        for s in chunk.code[i].successors(i) {
            work.push(s);
        }
    }
    if reachable.iter().all(|&r| r) {
        return false;
    }

    let mut map = vec![0usize; n + 1];
    let mut kept = 0;
    for i in 0..n {
        map[i] = kept;
        if reachable[i] {
            kept += 1;
        }
    }
    map[n] = kept;

    let mut new_code = Vec::with_capacity(kept);
    let mut new_spans = Vec::with_capacity(kept);
    for i in 0..n {
        if reachable[i] {
            new_code.push(chunk.code[i].clone());
            new_spans.push(chunk.spans[i].clone());
        }
    }
    remap(&mut new_code, &map);
    chunk.code = new_code;
    chunk.spans = new_spans;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::ir::lower;
    use crate::compiler::ir::validate::validate;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::parser::Parser;
    use crate::compiler::semantic::Checker;
    use crate::runtime::vm::Vm;
    use Instruction as I;

    fn lowered(src: &str) -> IrProgram {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        Checker::new().check_program(&program).unwrap();
        lower::compile(&program).unwrap()
    }

    fn optimized(src: &str) -> IrProgram {
        let mut ir = lowered(src);
        optimize(&mut ir);
        assert_eq!(validate(&ir), Ok(()));
        ir
    }

    fn run_ir(ir: &IrProgram) -> String {
        let mut vm = Vm::capturing(ir);
        vm.run().unwrap();
        vm.output().to_string()
    }

    #[test]
    fn folds_arithmetic() {
        assert_eq!(
            optimized("let x = 1 + 2 * 3\n").main.chunk.code,
            vec![I::PushInt(7), I::StoreGlobal(0), I::PushNone, I::Return]
        );
    }

    #[test]
    fn folds_float_and_string() {
        assert_eq!(
            optimized("let a = 1.5 + 2.5\nlet s = \"a\" + \"b\"\n").main.chunk.code,
            vec![
                I::PushFloat(4.0),
                I::StoreGlobal(0),
                I::PushStr("ab".to_string()),
                I::StoreGlobal(1),
                I::PushNone,
                I::Return,
            ]
        );
    }

    #[test]
    fn folds_not_and_comparison() {
        assert_eq!(
            optimized("let b = not (1 < 2)\n").main.chunk.code,
            vec![I::PushBool(false), I::StoreGlobal(0), I::PushNone, I::Return]
        );
    }

    #[test]
    fn keeps_division_by_zero() {
        assert_eq!(
            optimized("let a = 1 / 0\n").main.chunk.code,
            vec![I::PushInt(1), I::PushInt(0), I::Div, I::StoreGlobal(0), I::PushNone, I::Return]
        );
    }

    #[test]
    fn simplifies_constant_and() {
        assert_eq!(
            optimized("let a = true and false\n").main.chunk.code,
            vec![I::PushBool(false), I::StoreGlobal(0), I::PushNone, I::Return]
        );
    }

    #[test]
    fn removes_dead_branch() {
        assert_eq!(
            optimized("if false:\n    print(1)\nprint(2)\n").main.chunk.code,
            vec![I::PushInt(2), I::Print(1), I::Pop, I::PushNone, I::Return]
        );
    }

    #[test]
    fn removes_code_after_return() {
        let ir = optimized("fn f() -> Int:\n    return 1\nprint(f())\n");
        assert_eq!(ir.functions[0].chunk.code, vec![I::PushInt(1), I::Return]);
    }

    #[test]
    fn while_true_with_break() {
        let src = "mut i = 0\nwhile true:\n    i = i + 1\n    if i == 3:\n        break\nprint(i)\n";
        assert_eq!(run_ir(&optimized(src)), "3\n");
    }

    #[test]
    fn arithmetic_matches_unoptimized() {
        let src = "print(1 + 2 * 3 - 4 / 2)\nprint(10 % 3)\nprint(-5 + 2)\nprint(2.5 * 2.0)\nprint(\"x\" + \"y\")\nprint(1 < 2 and 3 > 2)\nprint(not true or false)\n";
        let expected = "5\n1\n-3\n5.0\nxy\ntrue\nfalse\n";
        assert_eq!(run_ir(&lowered(src)), expected);
        assert_eq!(run_ir(&optimized(src)), expected);
    }

    #[test]
    fn examples_stay_valid_and_equal() {
        let sources = [
            include_str!("../../../examples/run_demo.rca"),
            include_str!("../../../examples/full.rca"),
            include_str!("../../../examples/ir_demo.rca"),
        ];
        for src in sources {
            let plain = lowered(src);
            let fast = optimized(src);
            assert!(instruction_count(&fast) <= instruction_count(&plain));
            assert_eq!(run_ir(&plain), run_ir(&fast));
        }
    }
}
