use super::{Chunk, Instruction as I, IrProgram};

fn bad_at(name: &str, index: usize, message: impl Into<String>) -> String {
    format!(
        "invalid IR in '{}' at instruction {}: {}",
        name,
        index,
        message.into()
    )
}

/// Checks that compiled IR is well-formed. A failure means a compiler bug.
pub fn validate(program: &IrProgram) -> Result<(), String> {
    validate_chunk(
        "<main>",
        &program.main.chunk,
        program.main.num_locals,
        program,
    )?;
    for f in &program.functions {
        validate_chunk(&f.name, &f.chunk, f.num_locals, program)?;
    }
    Ok(())
}

/// (values popped, values pushed) by an instruction.
fn stack_effect(ins: &I, program: &IrProgram) -> Result<(usize, usize), String> {
    Ok(match ins {
        I::PushInt(_)
        | I::PushFloat(_)
        | I::PushBool(_)
        | I::PushStr(_)
        | I::PushEnum(_, _)
        | I::PushNone
        | I::LoadLocal(_)
        | I::LoadGlobal(_) => (0, 1),
        I::StoreLocal(_) | I::StoreGlobal(_) | I::Pop | I::JumpIfFalse(_) => (1, 0),
        I::Add
        | I::Sub
        | I::Mul
        | I::Div
        | I::Mod
        | I::Eq
        | I::NotEq
        | I::Lt
        | I::LtEq
        | I::Gt
        | I::GtEq
        | I::MakeRange
        | I::Index
        | I::SetField(_)
        | I::ListPush => (2, 1),
        I::ReadFile | I::FileExists => (1, 1),
        I::TimeNow => (0, 1),
        I::TimeSleep => (1, 1),
        I::StrSplit => (2, 1),
        I::StrReplace => (3, 1),
        I::MapNew => (0, 1),
        I::MapSet => (3, 1),
        I::MapGet | I::MapHas => (2, 1),
        I::JsonParse => (1, 1),
        I::JsonStringify => (1, 1),
        I::TcpConnect => (2, 1),
        I::TcpSend => (2, 1),
        I::TcpRecv => (2, 1),
        I::TcpClose => (1, 1),
        I::HttpGet => (1, 1),
        I::Sha256 => (1, 1),
        I::MapLen => (1, 1),
        I::StrTrim => (1, 1),
        I::MathAbs | I::MathFloor | I::MathCeil | I::MathRound => (1, 1),
        I::MathPow => (2, 1),
        I::WriteFile => (2, 1),
        I::Neg
        | I::Not
        | I::GetField(_)
        | I::Len
        | I::Convert(_)
        | I::MakeTensor
        | I::FillTensor(_)
        | I::TensorShape => (1, 1),
        I::MatMul | I::Grad => (2, 1),
        I::TensorReshape => (2, 1),
        I::TensorSum
        | I::TensorMean
        | I::TensorMax
        | I::TensorMin
        | I::TensorTranspose
        | I::TensorMap(_)
        | I::Track => (1, 1),
        I::Jump(_) | I::ForNext { .. } | I::PushHandler(_) | I::PopHandler => (0, 0),
        I::Return | I::Throw => (1, 0),
        I::StoreIndex => (3, 1),
        I::MakeList(n) => (*n, 1),
        I::MakeTuple(n) => (*n, 1),
        I::Print(n) => (*n, 1),
        I::MakeStruct(id) => match program.structs.get(*id) {
            Some(s) => (s.fields.len(), 1),
            None => return Err(format!("unknown struct id {}", id)),
        },
        I::Call(id) => match program.functions.get(*id) {
            Some(f) => (f.arity, 1),
            None => return Err(format!("unknown function id {}", id)),
        },
    })
}

fn validate_chunk(
    name: &str,
    chunk: &Chunk,
    num_locals: usize,
    program: &IrProgram,
) -> Result<(), String> {
    let n = chunk.code.len();
    if n == 0 {
        return Err(format!("invalid IR in '{}': no instructions", name));
    }
    if chunk.spans.len() != n {
        return Err(format!(
            "invalid IR in '{}': code and spans have different lengths",
            name
        ));
    }

    // 1. operands are in range
    for (i, ins) in chunk.code.iter().enumerate() {
        match ins {
            I::Jump(t) | I::JumpIfFalse(t) | I::PushHandler(t) if *t >= n => {
                return Err(bad_at(name, i, format!("jump target {} out of range", t)));
            }
            I::ForNext {
                iter,
                idx,
                var,
                exit,
            } => {
                if *exit >= n {
                    return Err(bad_at(
                        name,
                        i,
                        format!("jump target {} out of range", exit),
                    ));
                }
                for s in [*iter, *idx, *var] {
                    if s >= num_locals {
                        return Err(bad_at(name, i, format!("local slot {} out of range", s)));
                    }
                }
            }
            I::LoadLocal(s) | I::StoreLocal(s) if *s >= num_locals => {
                return Err(bad_at(name, i, format!("local slot {} out of range", s)));
            }
            I::LoadGlobal(s) | I::StoreGlobal(s) if *s >= program.num_globals => {
                return Err(bad_at(name, i, format!("global slot {} out of range", s)));
            }
            _ => {}
        }
    }

    // 2. stack depth is consistent on every path
    let mut depth: Vec<Option<usize>> = vec![None; n];
    depth[0] = Some(0);
    let mut work = vec![0usize];
    while let Some(i) = work.pop() {
        let d = depth[i].unwrap();
        let ins = &chunk.code[i];
        let (pops, pushes) = stack_effect(ins, program).map_err(|m| bad_at(name, i, m))?;
        if d < pops {
            return Err(bad_at(
                name,
                i,
                format!("stack underflow (needs {} values, has {})", pops, d),
            ));
        }
        let nd = d - pops + pushes;
        if let I::Return = ins {
            if nd != 0 {
                return Err(bad_at(
                    name,
                    i,
                    format!("stack not empty at return ({} extra)", nd),
                ));
            }
            continue;
        }
        if let I::PushHandler(t) = ins {
            let hd = d + 1;
            match depth[*t] {
                None => {
                    depth[*t] = Some(hd);
                    work.push(*t);
                }
                Some(x) => {
                    if x != hd {
                        return Err(bad_at(
                            name,
                            *t,
                            format!("inconsistent stack depth ({} vs {})", x, hd),
                        ));
                    }
                }
            }
        }
        for s in ins.successors(i) {
            if let I::PushHandler(t) = ins
                && s == *t
                && s != i + 1
            {
                continue;
            }
            if s >= n {
                return Err(bad_at(name, i, "execution falls off the end of the code"));
            }
            match depth[s] {
                None => {
                    depth[s] = Some(nd);
                    work.push(s);
                }
                Some(x) => {
                    if x != nd {
                        return Err(bad_at(
                            name,
                            s,
                            format!("inconsistent stack depth ({} vs {})", x, nd),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::ir::Function;
    use crate::compiler::ir::lower;
    use crate::compiler::lexer::Lexer;
    use crate::compiler::lexer::token::Span;
    use crate::compiler::parser::Parser;
    use crate::compiler::semantic::Checker;

    fn program_with(code: Vec<I>) -> IrProgram {
        let spans = vec![Span { line: 1, column: 1 }; code.len()];
        IrProgram {
            main: Function {
                name: "<main>".to_string(),
                arity: 0,
                num_locals: 1,
                param_types: vec![],
                return_type: crate::compiler::types::Type::None,
                chunk: Chunk { code, spans },
            },
            functions: vec![],
            structs: vec![],
            num_globals: 1,
        }
    }

    fn error_of(code: Vec<I>) -> String {
        validate(&program_with(code)).unwrap_err()
    }

    #[test]
    fn minimal_program_is_valid() {
        assert!(validate(&program_with(vec![I::PushNone, I::Return])).is_ok());
    }

    #[test]
    fn stack_underflow_is_detected() {
        assert!(error_of(vec![I::Add, I::PushNone, I::Return]).contains("underflow"));
    }

    #[test]
    fn jump_out_of_range_is_detected() {
        assert!(error_of(vec![I::Jump(99), I::PushNone, I::Return]).contains("jump target"));
    }

    #[test]
    fn bad_local_slot_is_detected() {
        let e = error_of(vec![
            I::PushInt(1),
            I::StoreLocal(5),
            I::PushNone,
            I::Return,
        ]);
        assert!(e.contains("local slot"));
    }

    #[test]
    fn bad_global_slot_is_detected() {
        let e = error_of(vec![I::LoadGlobal(3), I::Pop, I::PushNone, I::Return]);
        assert!(e.contains("global slot"));
    }

    #[test]
    fn falling_off_the_end_is_detected() {
        assert!(error_of(vec![I::PushNone]).contains("falls off"));
    }

    #[test]
    fn inconsistent_stack_depth_is_detected() {
        let code = vec![
            I::PushBool(true),
            I::JumpIfFalse(3),
            I::PushInt(1),
            I::PushNone,
            I::Return,
        ];
        assert!(validate(&program_with(code)).is_err());
    }

    #[test]
    fn unknown_function_is_detected() {
        assert!(error_of(vec![I::Call(7), I::Pop, I::PushNone, I::Return]).contains("function id"));
    }

    #[test]
    fn compiler_output_is_valid_for_examples() {
        let sources = [
            include_str!("../../../examples/full.rca"),
            include_str!("../../../examples/run_demo.rca"),
            include_str!("../../../examples/ir_demo.rca"),
        ];
        for src in sources {
            let tokens = Lexer::new(src).tokenize().unwrap();
            let program = Parser::new(tokens).parse_program().unwrap();
            Checker::new().check_program(&program).unwrap();
            let ir = lower::compile(&program).unwrap();
            assert_eq!(validate(&ir), Ok(()));
        }
    }
}
