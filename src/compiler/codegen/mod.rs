//! Cranelift backend for generating native machine code.
//! See blueprint §5.2: native compilation must ultimately be supported.
use cranelift::prelude::*;
use cranelift_codegen::ir::{AbiParam, InstBuilder, StackSlotData, StackSlotKind, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::compiler::ir::{Instruction, IrProgram};

pub fn compile(program: &IrProgram) -> Result<Vec<u8>, String> {
    // Setup ISA
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "true").unwrap();
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| e.to_string())?;

    // Create object module
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "rca_module", default_libcall_names())
            .map_err(|e| e.to_string())?,
    );

    // Declare printf: i32 printf(i64, i64)
    let mut printf_sig = module.make_signature();
    printf_sig.params.push(AbiParam::new(types::I64)); // pointer to format string
    printf_sig.params.push(AbiParam::new(types::I64)); // argument
    printf_sig.returns.push(AbiParam::new(types::I32));
    let printf_func = module
        .declare_function("printf", Linkage::Import, &printf_sig)
        .map_err(|e| e.to_string())?;

    // Define format string data ("%lld\n\0")
    let mut fmt_data = DataDescription::new();
    fmt_data.define(b"%lld\n\0".to_vec().into_boxed_slice());
    let fmt_id = module
        .declare_data("fmt_str", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_id, &fmt_data)
        .map_err(|e| e.to_string())?;

    // Setup main function
    let mut sig = module.make_signature();
    sig.returns.push(AbiParam::new(types::I32));
    let main_id = module
        .declare_function("main", Linkage::Export, &sig)
        .map_err(|e| e.to_string())?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig.clone();
    let mut builder_ctx = FunctionBuilderContext::new();

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Setup VM stack
        let stack_slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8192));
        let sp_var = Variable::new(0);
        builder.declare_var(sp_var, types::I64);
        let zero = builder.ins().iconst(types::I64, 0);
        builder.def_var(sp_var, zero);

        let gv_fmt = module.declare_data_in_func(fmt_id, builder.func);
        let fmt_ptr = builder.ins().global_value(types::I64, gv_fmt);
        let printf_ref = module.declare_func_in_func(printf_func, builder.func);

        // Translate IR
        for ins in &program.main.chunk.code {
            match ins {
                Instruction::PushInt(n) => {
                    let val = builder.ins().iconst(types::I64, *n);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest_addr = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest_addr, 0);
                    let new_sp = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, new_sp);
                }
                Instruction::Print(n) => {
                    for _ in 0..*n {
                        let sp = builder.use_var(sp_var);
                        let new_sp = builder.ins().iadd_imm(sp, -1);
                        builder.def_var(sp_var, new_sp);

                        let sp_off = builder.ins().imul_imm(new_sp, 8);
                        let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src_addr = builder.ins().iadd(sp_addr, sp_off);
                        let val = builder.ins().load(types::I64, MemFlags::new(), src_addr, 0);

                        builder.ins().call(printf_ref, &[fmt_ptr, val]);
                    }
                }
                Instruction::Add
                | Instruction::Sub
                | Instruction::Mul
                | Instruction::Div
                | Instruction::Mod => {
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);

                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let b_addr = builder.ins().iadd(sp_addr, sp_off);
                    let b = builder.ins().load(types::I64, MemFlags::new(), b_addr, 0);

                    let a_addr = builder.ins().iadd_imm(b_addr, -8);
                    let a = builder.ins().load(types::I64, MemFlags::new(), a_addr, 0);

                    let r = match ins {
                        Instruction::Add => builder.ins().iadd(a, b),
                        Instruction::Sub => builder.ins().isub(a, b),
                        Instruction::Mul => builder.ins().imul(a, b),
                        Instruction::Div => builder.ins().sdiv(a, b),
                        Instruction::Mod => builder.ins().srem(a, b),
                        _ => unreachable!(),
                    };
                    builder.ins().store(MemFlags::new(), r, a_addr, 0);
                }
                Instruction::PushBool(b) => {
                    let val = builder.ins().iconst(types::I64, if *b { 1 } else { 0 });
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest_addr = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest_addr, 0);
                    let new_sp = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, new_sp);
                }
                Instruction::Eq
                | Instruction::NotEq
                | Instruction::Lt
                | Instruction::LtEq
                | Instruction::Gt
                | Instruction::GtEq => {
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);

                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let b_addr = builder.ins().iadd(sp_addr, sp_off);
                    let b = builder.ins().load(types::I64, MemFlags::new(), b_addr, 0);

                    let a_addr = builder.ins().iadd_imm(b_addr, -8);
                    let a = builder.ins().load(types::I64, MemFlags::new(), a_addr, 0);

                    let cmp = match ins {
                        Instruction::Eq => builder.ins().icmp(IntCC::Equal, a, b),
                        Instruction::NotEq => builder.ins().icmp(IntCC::NotEqual, a, b),
                        Instruction::Lt => builder.ins().icmp(IntCC::SignedLessThan, a, b),
                        Instruction::LtEq => builder.ins().icmp(IntCC::SignedLessThanOrEqual, a, b),
                        Instruction::Gt => builder.ins().icmp(IntCC::SignedGreaterThan, a, b),
                        Instruction::GtEq => {
                            builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, a, b)
                        }
                        _ => unreachable!(),
                    };
                    let r = builder.ins().uextend(types::I64, cmp);
                    builder.ins().store(MemFlags::new(), r, a_addr, 0);
                }
                _ => {} // Other instructions not yet implemented
            }
        }

        let ret = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[ret]);
        builder.finalize();
    }

    module
        .define_function(main_id, &mut ctx)
        .map_err(|e| e.to_string())?;

    let obj = module.finish();
    obj.emit().map_err(|e| e.to_string())
}
