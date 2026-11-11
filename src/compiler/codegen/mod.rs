//! Cranelift backend for generating native machine code.
//! See blueprint §5.2: native compilation must ultimately be supported.
use crate::compiler::types::Type;
use cranelift::prelude::*;
use cranelift_codegen::ir::{
    AbiParam, FuncRef, InstBuilder, MemFlags, StackSlot, StackSlotData, StackSlotKind, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::cell::Cell;
use std::collections::HashMap;

use crate::compiler::ir::{ConvKind, Instruction, IrProgram};

#[derive(Clone, Copy)]
enum StackType {
    Int,
    Float,
    Str,
}

pub fn compile(program: &IrProgram) -> Result<Vec<u8>, String> {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    // Platform-conditional PIC:
    // - Linux/macOS/ARM: needs is_pic=true (default)
    // - Windows: is_pic=true causes access violation at runtime (0xC0000005)
    let is_pic = if cfg!(target_os = "windows") {
        "false"
    } else {
        "true"
    };
    flag_builder.set("is_pic", is_pic).unwrap();
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| e.to_string())?;

    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "rca_module", default_libcall_names())
            .map_err(|e| e.to_string())?,
    );

    // printf(fmt, arg) -> i32  (2 args only to avoid Windows x64 ABI shadow space issues)
    let mut printf_sig = module.make_signature();
    printf_sig.params.push(AbiParam::new(types::I64));
    printf_sig.params.push(AbiParam::new(types::I64));
    printf_sig.returns.push(AbiParam::new(types::I32));
    let printf_func = module
        .declare_function("printf", Linkage::Import, &printf_sig)
        .map_err(|e| e.to_string())?;

    // C-runtime for strings
    let mut strlen_sig = module.make_signature();
    strlen_sig.params.push(AbiParam::new(types::I64));
    strlen_sig.returns.push(AbiParam::new(types::I64));
    let strlen_func = module
        .declare_function("strlen", Linkage::Import, &strlen_sig)
        .map_err(|e| e.to_string())?;

    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(types::I64));
    malloc_sig.returns.push(AbiParam::new(types::I64));
    let malloc_func = module
        .declare_function("malloc", Linkage::Import, &malloc_sig)
        .map_err(|e| e.to_string())?;

    let mut memcpy_sig = module.make_signature();
    memcpy_sig.params.push(AbiParam::new(types::I64));
    memcpy_sig.params.push(AbiParam::new(types::I64));
    memcpy_sig.params.push(AbiParam::new(types::I64));
    memcpy_sig.returns.push(AbiParam::new(types::I64));
    let memcpy_func = module
        .declare_function("memcpy", Linkage::Import, &memcpy_sig)
        .map_err(|e| e.to_string())?;

    let mut strcmp_sig = module.make_signature();
    strcmp_sig.params.push(AbiParam::new(types::I64));
    strcmp_sig.params.push(AbiParam::new(types::I64));
    strcmp_sig.returns.push(AbiParam::new(types::I32));
    let strcmp_func = module
        .declare_function("strcmp", Linkage::Import, &strcmp_sig)
        .map_err(|e| e.to_string())?;

    // Format strings
    let mut fmt_int_data = DataDescription::new();
    fmt_int_data.define(b"%lld\n\0".to_vec().into_boxed_slice());
    let fmt_int_id = module
        .declare_data("fmt_int", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_int_id, &fmt_int_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_str_data = DataDescription::new();
    fmt_str_data.define(b"%s\n\0".to_vec().into_boxed_slice());
    let fmt_str_id = module
        .declare_data("fmt_str", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_str_id, &fmt_str_data)
        .map_err(|e| e.to_string())?;

    // Float format pieces (2-arg printf safe)
    let mut fmt_float_int_data = DataDescription::new();
    fmt_float_int_data.define(b"%lld.\0".to_vec().into_boxed_slice());
    let fmt_float_int_id = module
        .declare_data("fmt_float_int", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_float_int_id, &fmt_float_int_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_float_dec_data = DataDescription::new();
    fmt_float_dec_data.define(b"%03lld\n\0".to_vec().into_boxed_slice());
    let fmt_float_dec_id = module
        .declare_data("fmt_float_dec", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_float_dec_id, &fmt_float_dec_data)
        .map_err(|e| e.to_string())?;

    // Declare user functions
    let mut func_ids: Vec<FuncId> = Vec::new();
    for f in &program.functions {
        let mut sig = module.make_signature();
        for _ in 0..f.arity {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function(&f.name, Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.push(id);
    }

    let mut main_sig = module.make_signature();
    main_sig.returns.push(AbiParam::new(types::I32));
    let main_id = module
        .declare_function("main", Linkage::Export, &main_sig)
        .map_err(|e| e.to_string())?;

    let str_counter = Cell::new(0usize);
    // Stack slot size: 1024 bytes each. Under Windows's 4KB stack-probe threshold,
    // so Cranelift doesn't emit __chkstk calls (which have ABI naming issues with clang).
    let slot_size = 1024u32;

    let compile_chunk = |module: &mut ObjectModule,
                         builder: &mut FunctionBuilder,
                         chunk: &crate::compiler::ir::Chunk,
                         stack_slot: StackSlot,
                         locals_slot: StackSlot,
                         globals_slot: StackSlot,
                         sp_var: Variable,
                         printf_ref: FuncRef,
                         strlen_ref: FuncRef,
                         malloc_ref: FuncRef,
                         memcpy_ref: FuncRef,
                         strcmp_ref: FuncRef,
                         fmt_int_ptr: Value,
                         fmt_str_ptr: Value,
                         fmt_float_int_ptr: Value,
                         fmt_float_dec_ptr: Value,
                         param_types: &[Type],
                         is_main: bool|
     -> Result<(), String> {
        let mut type_stack: Vec<StackType> = Vec::new();
        let mut local_types: HashMap<usize, StackType> = HashMap::new();
        for (i, ty) in param_types.iter().enumerate() {
            let st = match ty {
                Type::Int | Type::Bool => StackType::Int,
                Type::Float => StackType::Float,
                Type::String => StackType::Str,
                _ => StackType::Int,
            };
            local_types.insert(i, st);
        }
        let code = &chunk.code;
        let mut blocks = Vec::new();
        for _ in 0..code.len() + 1 {
            blocks.push(builder.create_block());
        }
        builder.ins().jump(blocks[0], &[]);

        for (ip, ins) in code.iter().enumerate() {
            builder.switch_to_block(blocks[ip]);
            match ins {
                Instruction::PushInt(n) => {
                    let val = builder.ins().iconst(types::I64, *n);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::PushFloat(f) => {
                    let f_val = builder.ins().f64const(*f);
                    let bits = builder.ins().bitcast(types::I64, MemFlags::new(), f_val);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), bits, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Float);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::PushBool(b) => {
                    let val = builder.ins().iconst(types::I64, if *b { 1 } else { 0 });
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::PushStr(s) => {
                    let id = {
                        let u = str_counter.get();
                        str_counter.set(u + 1);
                        let mut data = DataDescription::new();
                        let mut bytes = s.as_bytes().to_vec();
                        bytes.push(0);
                        data.define(bytes.into_boxed_slice());
                        let d_id = module
                            .declare_data(&format!("str_{}", u), Linkage::Local, false, false)
                            .map_err(|e| e.to_string())?;
                        module.define_data(d_id, &data).map_err(|e| e.to_string())?;
                        d_id
                    };
                    let gv = module.declare_data_in_func(id, builder.func);
                    let ptr = builder.ins().global_value(types::I64, gv);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), ptr, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Str);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Convert(kind) => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let src = builder.ins().iadd(sp_addr, sp_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let (result, new_ty) = match kind {
                        ConvKind::Int => match ty {
                            StackType::Int => (val, StackType::Int),
                            StackType::Float => {
                                let f = builder.ins().bitcast(types::F64, MemFlags::new(), val);
                                let i = builder.ins().fcvt_to_sint(types::I64, f);
                                (i, StackType::Int)
                            }
                            StackType::Str => {
                                return Err("str -> int not yet supported in native".to_string());
                            }
                        },
                        ConvKind::Float => match ty {
                            StackType::Int => {
                                let f = builder.ins().fcvt_from_sint(types::F64, val);
                                let bits = builder.ins().bitcast(types::I64, MemFlags::new(), f);
                                (bits, StackType::Float)
                            }
                            StackType::Float => (val, StackType::Float),
                            StackType::Str => {
                                return Err("str -> float not yet supported in native".to_string());
                            }
                        },
                        ConvKind::Str => {
                            return Err("str() not yet supported in native".to_string());
                        }
                    };
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), result, dest, 0);
                    let new_sp = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, new_sp);
                    type_stack.push(new_ty);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::Print(n) => {
                    for _ in 0..*n {
                        let ty = type_stack.pop().unwrap_or(StackType::Int);
                        let sp = builder.use_var(sp_var);
                        let new_sp = builder.ins().iadd_imm(sp, -1);
                        builder.def_var(sp_var, new_sp);
                        let sp_off = builder.ins().imul_imm(new_sp, 8);
                        let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src = builder.ins().iadd(sp_addr, sp_off);
                        let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                        match ty {
                            StackType::Int => {
                                builder.ins().call(printf_ref, &[fmt_int_ptr, val]);
                            }
                            StackType::Str => {
                                builder.ins().call(printf_ref, &[fmt_str_ptr, val]);
                            }
                            StackType::Float => {
                                let f_val = builder.ins().bitcast(types::F64, MemFlags::new(), val);
                                let thousand = builder.ins().f64const(1000.0);
                                let scaled = builder.ins().fmul(f_val, thousand);
                                let scaled_int = builder.ins().fcvt_to_sint(types::I64, scaled);
                                let divisor = builder.ins().iconst(types::I64, 1000);
                                let int_part = builder.ins().sdiv(scaled_int, divisor);
                                let dec_raw = builder.ins().srem(scaled_int, divisor);
                                let zero = builder.ins().iconst(types::I64, 0);
                                let is_neg =
                                    builder.ins().icmp(IntCC::SignedLessThan, dec_raw, zero);
                                let neg_dec = builder.ins().ineg(dec_raw);
                                let dec_part = builder.ins().select(is_neg, neg_dec, dec_raw);
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_float_int_ptr, int_part]);
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_float_dec_ptr, dec_part]);
                            }
                        }
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::StoreLocal(slot) => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    local_types.insert(*slot, ty);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let src = builder.ins().iadd(sp_addr, sp_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let slot_off = builder.ins().iconst(types::I64, (*slot as i64) * 8);
                    let locals_addr = builder.ins().stack_addr(types::I64, locals_slot, 0);
                    let dest = builder.ins().iadd(locals_addr, slot_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::LoadLocal(slot) => {
                    let ty = local_types.get(slot).copied().unwrap_or(StackType::Int);
                    let slot_off = builder.ins().iconst(types::I64, (*slot as i64) * 8);
                    let locals_addr = builder.ins().stack_addr(types::I64, locals_slot, 0);
                    let src = builder.ins().iadd(locals_addr, slot_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(ty);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::StoreGlobal(slot) => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    local_types.insert(100000 + slot, ty);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let src = builder.ins().iadd(sp_addr, sp_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let slot_off = builder.ins().iconst(types::I64, (*slot as i64) * 8);
                    let globals_addr = builder.ins().stack_addr(types::I64, globals_slot, 0);
                    let dest = builder.ins().iadd(globals_addr, slot_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::LoadGlobal(slot) => {
                    let ty = local_types
                        .get(&(100000 + slot))
                        .copied()
                        .unwrap_or(StackType::Int);
                    let slot_off = builder.ins().iconst(types::I64, (*slot as i64) * 8);
                    let globals_addr = builder.ins().stack_addr(types::I64, globals_slot, 0);
                    let src = builder.ins().iadd(globals_addr, slot_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(ty);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MakeRange => {
                    let sp = builder.use_var(sp_var);
                    let sp_end = builder.ins().iadd_imm(sp, -1);
                    let sp_off_end = builder.ins().imul_imm(sp_end, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let end_addr = builder.ins().iadd(sp_addr, sp_off_end);
                    let end = builder.ins().load(types::I64, MemFlags::new(), end_addr, 0);
                    let sp_start = builder.ins().iadd_imm(sp, -2);
                    let sp_off_start = builder.ins().imul_imm(sp_start, 8);
                    let start_addr = builder.ins().iadd(sp_addr, sp_off_start);
                    let start = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), start_addr, 0);
                    let start_shifted = builder.ins().ishl_imm(start, 32);
                    let end_masked = builder.ins().band_imm(end, 0xffffffff_u32 as i64);
                    let packed = builder.ins().bor(start_shifted, end_masked);
                    builder.ins().store(MemFlags::new(), packed, start_addr, 0);
                    builder.def_var(sp_var, sp_end);
                    type_stack.pop();
                    type_stack.pop();
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::ForNext {
                    iter,
                    idx,
                    var,
                    exit,
                } => {
                    let locals_addr = builder.ins().stack_addr(types::I64, locals_slot, 0);
                    let iter_off = builder.ins().iconst(types::I64, (*iter as i64) * 8);
                    let iter_addr = builder.ins().iadd(locals_addr, iter_off);
                    let handle = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), iter_addr, 0);
                    let start = builder.ins().ushr_imm(handle, 32);
                    let end = builder.ins().band_imm(handle, 0xffffffff_u32 as i64);
                    let idx_off = builder.ins().iconst(types::I64, (*idx as i64) * 8);
                    let idx_addr = builder.ins().iadd(locals_addr, idx_off);
                    let i = builder.ins().load(types::I64, MemFlags::new(), idx_addr, 0);
                    let current = builder.ins().iadd(start, i);
                    let less = builder.ins().icmp(IntCC::SignedLessThan, current, end);
                    let var_off = builder.ins().iconst(types::I64, (*var as i64) * 8);
                    let var_addr = builder.ins().iadd(locals_addr, var_off);
                    builder.ins().store(MemFlags::new(), current, var_addr, 0);
                    let next_i = builder.ins().iadd_imm(i, 1);
                    builder.ins().store(MemFlags::new(), next_i, idx_addr, 0);
                    builder
                        .ins()
                        .brif(less, blocks[ip + 1], &[], blocks[*exit], &[]);
                }
                Instruction::Add
                | Instruction::Sub
                | Instruction::Mul
                | Instruction::Div
                | Instruction::Mod => {
                    let ty_b = type_stack.pop().unwrap_or(StackType::Int);
                    let ty_a = type_stack.pop().unwrap_or(StackType::Int);
                    let is_float =
                        matches!(ty_a, StackType::Float) || matches!(ty_b, StackType::Float);
                    let is_str = matches!(ty_a, StackType::Str) && matches!(ty_b, StackType::Str);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let b_addr = builder.ins().iadd(sp_addr, sp_off);
                    let b_bits = builder.ins().load(types::I64, MemFlags::new(), b_addr, 0);
                    let a_addr = builder.ins().iadd_imm(b_addr, -8);
                    let a_bits = builder.ins().load(types::I64, MemFlags::new(), a_addr, 0);

                    if is_str && matches!(ins, Instruction::Add) {
                        let len1_call = builder.ins().call(strlen_ref, &[a_bits]);
                        let len1 = builder.inst_results(len1_call)[0];
                        let len2_call = builder.ins().call(strlen_ref, &[b_bits]);
                        let len2 = builder.inst_results(len2_call)[0];
                        let total_len = builder.ins().iadd(len1, len2);
                        let alloc_size = builder.ins().iadd_imm(total_len, 1);
                        let new_str_call = builder.ins().call(malloc_ref, &[alloc_size]);
                        let new_str = builder.inst_results(new_str_call)[0];
                        builder.ins().call(memcpy_ref, &[new_str, a_bits, len1]);
                        let dest2 = builder.ins().iadd(new_str, len1);
                        builder.ins().call(memcpy_ref, &[dest2, b_bits, len2]);
                        let null_pos = builder.ins().iadd(new_str, total_len);
                        let null_byte = builder.ins().iconst(types::I8, 0);
                        builder.ins().store(MemFlags::new(), null_byte, null_pos, 0);
                        builder.ins().store(MemFlags::new(), new_str, a_addr, 0);
                        type_stack.push(StackType::Str);
                    } else {
                        let r_bits = if is_float {
                            let a = builder.ins().bitcast(types::F64, MemFlags::new(), a_bits);
                            let b = builder.ins().bitcast(types::F64, MemFlags::new(), b_bits);
                            let r = match ins {
                                Instruction::Add => builder.ins().fadd(a, b),
                                Instruction::Sub => builder.ins().fsub(a, b),
                                Instruction::Mul => builder.ins().fmul(a, b),
                                Instruction::Div => builder.ins().fdiv(a, b),
                                _ => builder.ins().fadd(a, b),
                            };
                            builder.ins().bitcast(types::I64, MemFlags::new(), r)
                        } else {
                            match ins {
                                Instruction::Add => builder.ins().iadd(a_bits, b_bits),
                                Instruction::Sub => builder.ins().isub(a_bits, b_bits),
                                Instruction::Mul => builder.ins().imul(a_bits, b_bits),
                                Instruction::Div => builder.ins().sdiv(a_bits, b_bits),
                                Instruction::Mod => builder.ins().srem(a_bits, b_bits),
                                _ => unreachable!(),
                            }
                        };
                        builder.ins().store(MemFlags::new(), r_bits, a_addr, 0);
                        type_stack.push(if is_float {
                            StackType::Float
                        } else {
                            StackType::Int
                        });
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Eq
                | Instruction::NotEq
                | Instruction::Lt
                | Instruction::LtEq
                | Instruction::Gt
                | Instruction::GtEq => {
                    let ty_b = type_stack.pop().unwrap_or(StackType::Int);
                    let ty_a = type_stack.pop().unwrap_or(StackType::Int);
                    let is_float =
                        matches!(ty_a, StackType::Float) || matches!(ty_b, StackType::Float);
                    let is_str = matches!(ty_a, StackType::Str) && matches!(ty_b, StackType::Str);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let b_addr = builder.ins().iadd(sp_addr, sp_off);
                    let b_bits = builder.ins().load(types::I64, MemFlags::new(), b_addr, 0);
                    let a_addr = builder.ins().iadd_imm(b_addr, -8);
                    let a_bits = builder.ins().load(types::I64, MemFlags::new(), a_addr, 0);

                    let r = if is_str {
                        let cmp_res_call = builder.ins().call(strcmp_ref, &[a_bits, b_bits]);
                        let cmp_res = builder.inst_results(cmp_res_call)[0];
                        let zero_i32 = builder.ins().iconst(types::I32, 0);
                        let cmp = match ins {
                            Instruction::Eq => builder.ins().icmp(IntCC::Equal, cmp_res, zero_i32),
                            Instruction::NotEq => {
                                builder.ins().icmp(IntCC::NotEqual, cmp_res, zero_i32)
                            }
                            _ => return Err("String ordering not supported".to_string()),
                        };
                        builder.ins().uextend(types::I64, cmp)
                    } else if is_float {
                        let a = builder.ins().bitcast(types::F64, MemFlags::new(), a_bits);
                        let b = builder.ins().bitcast(types::F64, MemFlags::new(), b_bits);
                        let cmp = match ins {
                            Instruction::Eq => builder.ins().fcmp(FloatCC::Equal, a, b),
                            Instruction::NotEq => builder.ins().fcmp(FloatCC::NotEqual, a, b),
                            Instruction::Lt => builder.ins().fcmp(FloatCC::LessThan, a, b),
                            Instruction::LtEq => builder.ins().fcmp(FloatCC::LessThanOrEqual, a, b),
                            Instruction::Gt => builder.ins().fcmp(FloatCC::GreaterThan, a, b),
                            Instruction::GtEq => {
                                builder.ins().fcmp(FloatCC::GreaterThanOrEqual, a, b)
                            }
                            _ => unreachable!(),
                        };
                        builder.ins().uextend(types::I64, cmp)
                    } else {
                        let cmp = match ins {
                            Instruction::Eq => builder.ins().icmp(IntCC::Equal, a_bits, b_bits),
                            Instruction::NotEq => {
                                builder.ins().icmp(IntCC::NotEqual, a_bits, b_bits)
                            }
                            Instruction::Lt => {
                                builder.ins().icmp(IntCC::SignedLessThan, a_bits, b_bits)
                            }
                            Instruction::LtEq => {
                                builder
                                    .ins()
                                    .icmp(IntCC::SignedLessThanOrEqual, a_bits, b_bits)
                            }
                            Instruction::Gt => {
                                builder.ins().icmp(IntCC::SignedGreaterThan, a_bits, b_bits)
                            }
                            Instruction::GtEq => {
                                builder
                                    .ins()
                                    .icmp(IntCC::SignedGreaterThanOrEqual, a_bits, b_bits)
                            }
                            _ => unreachable!(),
                        };
                        builder.ins().uextend(types::I64, cmp)
                    };
                    builder.ins().store(MemFlags::new(), r, a_addr, 0);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Call(id) => {
                    let f_id = func_ids[*id];
                    let sig = module
                        .declarations()
                        .get_function_decl(f_id)
                        .signature
                        .clone();
                    let mut args = Vec::new();
                    for _ in 0..sig.params.len() {
                        let sp = builder.use_var(sp_var);
                        let new_sp = builder.ins().iadd_imm(sp, -1);
                        builder.def_var(sp_var, new_sp);
                        let sp_off = builder.ins().imul_imm(new_sp, 8);
                        let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let arg_addr = builder.ins().iadd(sp_addr, sp_off);
                        let arg = builder.ins().load(types::I64, MemFlags::new(), arg_addr, 0);
                        args.push(arg);
                        type_stack.pop();
                    }
                    args.reverse();
                    let f_ref = module.declare_func_in_func(f_id, builder.func);
                    let call = builder.ins().call(f_ref, &args);
                    let ret_val = builder.inst_results(call)[0];
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), ret_val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Jump(target) => {
                    builder.ins().jump(blocks[*target], &[]);
                }
                Instruction::JumpIfFalse(target) => {
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let sp_off = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let src = builder.ins().iadd(sp_addr, sp_off);
                    let cond = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let is_false = builder.ins().icmp(IntCC::Equal, cond, zero);
                    builder
                        .ins()
                        .brif(is_false, blocks[*target], &[], blocks[ip + 1], &[]);
                    type_stack.pop();
                }
                Instruction::Return => {
                    if is_main {
                        let ret = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[ret]);
                    } else {
                        let sp = builder.use_var(sp_var);
                        let new_sp = builder.ins().iadd_imm(sp, -1);
                        builder.def_var(sp_var, new_sp);
                        let sp_off = builder.ins().imul_imm(new_sp, 8);
                        let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src = builder.ins().iadd(sp_addr, sp_off);
                        let ret_val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                        builder.ins().return_(&[ret_val]);
                    }
                }
                _ => {
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
            }
        }

        builder.switch_to_block(blocks[code.len()]);
        if is_main {
            let ret = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[ret]);
        } else {
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
        }

        for block in &blocks {
            builder.seal_block(*block);
        }
        Ok(())
    };

    let mut ctx = module.make_context();
    ctx.func.signature = main_sig.clone();
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let stack_slot = builder
            .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, slot_size));
        let locals_slot = builder
            .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, slot_size));
        let globals_slot = builder
            .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, slot_size));
        let sp_var = Variable::new(0);
        builder.declare_var(sp_var, types::I64);
        let zero = builder.ins().iconst(types::I64, 0);
        builder.def_var(sp_var, zero);

        let fmt_int_gv = module.declare_data_in_func(fmt_int_id, builder.func);
        let fmt_int_ptr = builder.ins().global_value(types::I64, fmt_int_gv);
        let fmt_str_gv = module.declare_data_in_func(fmt_str_id, builder.func);
        let fmt_str_ptr = builder.ins().global_value(types::I64, fmt_str_gv);
        let fmt_float_int_gv = module.declare_data_in_func(fmt_float_int_id, builder.func);
        let fmt_float_int_ptr = builder.ins().global_value(types::I64, fmt_float_int_gv);
        let fmt_float_dec_gv = module.declare_data_in_func(fmt_float_dec_id, builder.func);
        let fmt_float_dec_ptr = builder.ins().global_value(types::I64, fmt_float_dec_gv);

        let printf_ref = module.declare_func_in_func(printf_func, builder.func);
        let strlen_ref = module.declare_func_in_func(strlen_func, builder.func);
        let malloc_ref = module.declare_func_in_func(malloc_func, builder.func);
        let memcpy_ref = module.declare_func_in_func(memcpy_func, builder.func);
        let strcmp_ref = module.declare_func_in_func(strcmp_func, builder.func);

        compile_chunk(
            &mut module,
            &mut builder,
            &program.main.chunk,
            stack_slot,
            locals_slot,
            globals_slot,
            sp_var,
            printf_ref,
            strlen_ref,
            malloc_ref,
            memcpy_ref,
            strcmp_ref,
            fmt_int_ptr,
            fmt_str_ptr,
            fmt_float_int_ptr,
            fmt_float_dec_ptr,
            &[],
            true,
        )?;
        builder.finalize();
    }
    module
        .define_function(main_id, &mut ctx)
        .map_err(|e| e.to_string())?;

    for (i, f) in program.functions.iter().enumerate() {
        let mut f_ctx = module.make_context();
        let mut f_sig = module.make_signature();
        for _ in 0..f.arity {
            f_sig.params.push(AbiParam::new(types::I64));
        }
        f_sig.returns.push(AbiParam::new(types::I64));
        f_ctx.func.signature = f_sig.clone();

        let mut f_builder_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut f_ctx.func, &mut f_builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            let stack_slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                slot_size,
            ));
            let locals_slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                slot_size,
            ));
            let globals_slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                slot_size,
            ));
            let sp_var = Variable::new(0);
            builder.declare_var(sp_var, types::I64);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.def_var(sp_var, zero);

            let fmt_int_gv = module.declare_data_in_func(fmt_int_id, builder.func);
            let fmt_int_ptr = builder.ins().global_value(types::I64, fmt_int_gv);
            let fmt_str_gv = module.declare_data_in_func(fmt_str_id, builder.func);
            let fmt_str_ptr = builder.ins().global_value(types::I64, fmt_str_gv);
            let fmt_float_int_gv = module.declare_data_in_func(fmt_float_int_id, builder.func);
            let fmt_float_int_ptr = builder.ins().global_value(types::I64, fmt_float_int_gv);
            let fmt_float_dec_gv = module.declare_data_in_func(fmt_float_dec_id, builder.func);
            let fmt_float_dec_ptr = builder.ins().global_value(types::I64, fmt_float_dec_gv);

            let printf_ref = module.declare_func_in_func(printf_func, builder.func);
            let strlen_ref = module.declare_func_in_func(strlen_func, builder.func);
            let malloc_ref = module.declare_func_in_func(malloc_func, builder.func);
            let memcpy_ref = module.declare_func_in_func(memcpy_func, builder.func);
            let strcmp_ref = module.declare_func_in_func(strcmp_func, builder.func);

            let params = builder.block_params(entry_block).to_vec();
            for (p_idx, p_val) in params.iter().enumerate() {
                let slot_off = builder.ins().iconst(types::I64, (p_idx as i64) * 8);
                let locals_addr = builder.ins().stack_addr(types::I64, locals_slot, 0);
                let dest = builder.ins().iadd(locals_addr, slot_off);
                builder.ins().store(MemFlags::new(), *p_val, dest, 0);
            }

            compile_chunk(
                &mut module,
                &mut builder,
                &f.chunk,
                stack_slot,
                locals_slot,
                globals_slot,
                sp_var,
                printf_ref,
                strlen_ref,
                malloc_ref,
                memcpy_ref,
                strcmp_ref,
                fmt_int_ptr,
                fmt_str_ptr,
                fmt_float_int_ptr,
                fmt_float_dec_ptr,
                &f.param_types,
                false,
            )?;
            builder.finalize();
        }
        module
            .define_function(func_ids[i], &mut f_ctx)
            .map_err(|e| e.to_string())?;
    }

    let obj = module.finish();
    obj.emit().map_err(|e| e.to_string())
}
