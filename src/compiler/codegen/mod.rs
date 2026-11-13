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

use crate::compiler::ir::{ConvKind, Instruction, IrProgram, StructInfo};

#[derive(Clone, Copy)]
enum ElemType {
    Int,
    Float,
    Str,
    Bool,
}

#[derive(Clone, Copy)]
enum StackType {
    Int,
    Float,
    Str,
    Bool,
    List(#[allow(dead_code)] ElemType),
    Tuple(u32),
    Map,
    Struct(usize),
}

#[allow(dead_code)]
#[allow(dead_code)]
fn emit_incref(builder: &mut FunctionBuilder, ptr: Value) {
    let zero = builder.ins().iconst(types::I64, 0);
    let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
    let then_blk = builder.create_block();
    let cont_blk = builder.create_block();
    builder.ins().brif(is_null, cont_blk, &[], then_blk, &[]);
    builder.switch_to_block(then_blk);
    builder.seal_block(then_blk);
    let rc = builder.ins().load(types::I64, MemFlags::new(), ptr, 0);
    let rc_new = builder.ins().iadd_imm(rc, 1);
    builder.ins().store(MemFlags::new(), rc_new, ptr, 0);
    builder.ins().jump(cont_blk, &[]);
    builder.switch_to_block(cont_blk);
    builder.seal_block(cont_blk);
}

#[allow(dead_code)]
#[allow(dead_code)]
fn emit_decref(builder: &mut FunctionBuilder, ptr: Value, free_ref: FuncRef) {
    let zero = builder.ins().iconst(types::I64, 0);
    let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
    let cont_blk = builder.create_block();
    let then_blk = builder.create_block();
    builder.ins().brif(is_null, cont_blk, &[], then_blk, &[]);
    builder.switch_to_block(then_blk);
    let rc = builder.ins().load(types::I64, MemFlags::new(), ptr, 0);
    let rc_new = builder.ins().iadd_imm(rc, -1);
    builder.ins().store(MemFlags::new(), rc_new, ptr, 0);
    let is_zero = builder.ins().icmp_imm(IntCC::Equal, rc_new, 0);
    let free_blk = builder.create_block();
    let after_free = builder.create_block();
    builder.ins().brif(is_zero, free_blk, &[], after_free, &[]);
    builder.switch_to_block(free_blk);
    builder.seal_block(free_blk);
    builder.ins().call(free_ref, &[ptr]);
    builder.ins().jump(after_free, &[]);
    builder.switch_to_block(after_free);
    builder.seal_block(after_free);
    builder.ins().jump(cont_blk, &[]);
    builder.switch_to_block(cont_blk);
    builder.seal_block(cont_blk);
}

#[allow(dead_code)]
#[allow(dead_code)]
fn is_heap(ty: &StackType) -> bool {
    matches!(
        ty,
        StackType::List(_) | StackType::Tuple(_) | StackType::Struct(_) | StackType::Map
    )
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
    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(types::I64));
    free_sig.returns.push(AbiParam::new(types::I32));
    let free_func = module
        .declare_function("free", Linkage::Import, &free_sig)
        .map_err(|e| e.to_string())?;

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

    let mut exit_sig = module.make_signature();
    exit_sig.params.push(AbiParam::new(types::I32));
    let exit_func = module
        .declare_function("exit", Linkage::Import, &exit_sig)
        .map_err(|e| e.to_string())?;

    let mut strcmp_sig = module.make_signature();
    strcmp_sig.params.push(AbiParam::new(types::I64));
    strcmp_sig.params.push(AbiParam::new(types::I64));
    strcmp_sig.returns.push(AbiParam::new(types::I32));
    let strcmp_func = module
        .declare_function("strcmp", Linkage::Import, &strcmp_sig)
        .map_err(|e| e.to_string())?;

    let mut pow_sig = module.make_signature();
    pow_sig.params.push(AbiParam::new(types::F64));
    pow_sig.params.push(AbiParam::new(types::F64));
    pow_sig.returns.push(AbiParam::new(types::F64));
    let pow_func = module
        .declare_function("pow", Linkage::Import, &pow_sig)
        .map_err(|e| e.to_string())?;

    let mut floor_sig = module.make_signature();
    floor_sig.params.push(AbiParam::new(types::F64));
    floor_sig.returns.push(AbiParam::new(types::F64));
    let floor_func = module
        .declare_function("floor", Linkage::Import, &floor_sig)
        .map_err(|e| e.to_string())?;

    let mut ceil_sig = module.make_signature();
    ceil_sig.params.push(AbiParam::new(types::F64));
    ceil_sig.returns.push(AbiParam::new(types::F64));
    let ceil_func = module
        .declare_function("ceil", Linkage::Import, &ceil_sig)
        .map_err(|e| e.to_string())?;

    let mut round_sig = module.make_signature();
    round_sig.params.push(AbiParam::new(types::F64));
    round_sig.returns.push(AbiParam::new(types::F64));
    let round_func = module
        .declare_function("round", Linkage::Import, &round_sig)
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

    let mut fmt_int_sp_data = DataDescription::new();
    fmt_int_sp_data.define(b"%lld \0".to_vec().into_boxed_slice());
    let fmt_int_sp_id = module
        .declare_data("fmt_int_sp", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_int_sp_id, &fmt_int_sp_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_str_sp_data = DataDescription::new();
    fmt_str_sp_data.define(b"%s \0".to_vec().into_boxed_slice());
    let fmt_str_sp_id = module
        .declare_data("fmt_str_sp", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_str_sp_id, &fmt_str_sp_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_float_dec_sp_data = DataDescription::new();
    fmt_float_dec_sp_data.define(b"%03lld \0".to_vec().into_boxed_slice());
    let fmt_float_dec_sp_id = module
        .declare_data("fmt_float_dec_sp", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_float_dec_sp_id, &fmt_float_dec_sp_data)
        .map_err(|e| e.to_string())?;

    let mut str_nan_data = DataDescription::new();
    str_nan_data.define(b"nan\0".to_vec().into_boxed_slice());
    let str_nan_id = module
        .declare_data("str_nan", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_nan_id, &str_nan_data)
        .map_err(|e| e.to_string())?;

    let mut str_inf_data = DataDescription::new();
    str_inf_data.define(b"inf\0".to_vec().into_boxed_slice());
    let str_inf_id = module
        .declare_data("str_inf", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_inf_id, &str_inf_data)
        .map_err(|e| e.to_string())?;

    let mut str_neg_inf_data = DataDescription::new();
    str_neg_inf_data.define(b"-inf\0".to_vec().into_boxed_slice());
    let str_neg_inf_id = module
        .declare_data("str_neg_inf", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_neg_inf_id, &str_neg_inf_data)
        .map_err(|e| e.to_string())?;
    let mut str_div0_data = DataDescription::new();
    str_div0_data.define(
        b"runtime error: division by zero\0"
            .to_vec()
            .into_boxed_slice(),
    );
    let str_div0_id = module
        .declare_data("str_div0", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_div0_id, &str_div0_data)
        .map_err(|e| e.to_string())?;

    let mut str_ovf_data = DataDescription::new();
    str_ovf_data.define(
        b"runtime error: integer overflow\0"
            .to_vec()
            .into_boxed_slice(),
    );
    let str_ovf_id = module
        .declare_data("str_ovf", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_ovf_id, &str_ovf_data)
        .map_err(|e| e.to_string())?;

    let mut str_true_data = DataDescription::new();
    str_true_data.define(b"true\0".to_vec().into_boxed_slice());
    let str_true_id = module
        .declare_data("str_true", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_true_id, &str_true_data)
        .map_err(|e| e.to_string())?;

    let mut str_false_data = DataDescription::new();
    str_false_data.define(b"false\0".to_vec().into_boxed_slice());
    let str_false_id = module
        .declare_data("str_false", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(str_false_id, &str_false_data)
        .map_err(|e| e.to_string())?;

    let mut globals_data = DataDescription::new();
    globals_data.define_zeroinit(8192);
    let globals_id = module
        .declare_data("rca_globals", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(globals_id, &globals_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_str_raw_data = DataDescription::new();
    fmt_str_raw_data.define(b"%s\0".to_vec().into_boxed_slice());
    let fmt_str_raw_id = module
        .declare_data("fmt_str_raw", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_str_raw_id, &fmt_str_raw_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_lb_data = DataDescription::new();
    fmt_lb_data.define(b"[\0".to_vec().into_boxed_slice());
    let fmt_lb_id = module
        .declare_data("fmt_lb", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_lb_id, &fmt_lb_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_rb_data = DataDescription::new();
    fmt_rb_data.define(b"]\n\0".to_vec().into_boxed_slice());
    let fmt_rb_id = module
        .declare_data("fmt_rb", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_rb_id, &fmt_rb_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_rb_sp_data = DataDescription::new();
    fmt_rb_sp_data.define(b"] \0".to_vec().into_boxed_slice());
    let fmt_rb_sp_id = module
        .declare_data("fmt_rb_sp", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_rb_sp_id, &fmt_rb_sp_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_comma_data = DataDescription::new();
    fmt_comma_data.define(b", \0".to_vec().into_boxed_slice());
    let fmt_comma_id = module
        .declare_data("fmt_comma", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_comma_id, &fmt_comma_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_empty_data = DataDescription::new();
    fmt_empty_data.define(b"\0".to_vec().into_boxed_slice());
    let fmt_empty_id = module
        .declare_data("fmt_empty", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_empty_id, &fmt_empty_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_int_elem_data = DataDescription::new();
    fmt_int_elem_data.define(b"%lld\0".to_vec().into_boxed_slice());
    let fmt_int_elem_id = module
        .declare_data("fmt_int_elem", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_int_elem_id, &fmt_int_elem_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_str_elem_data = DataDescription::new();
    fmt_str_elem_data.define(b"\"%s\"\0".to_vec().into_boxed_slice());
    let fmt_str_elem_id = module
        .declare_data("fmt_str_elem", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_str_elem_id, &fmt_str_elem_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_float_dec_elem_data = DataDescription::new();
    fmt_float_dec_elem_data.define(b"%03lld\0".to_vec().into_boxed_slice());
    let fmt_float_dec_elem_id = module
        .declare_data("fmt_float_dec_elem", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_float_dec_elem_id, &fmt_float_dec_elem_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_oob_data = DataDescription::new();
    fmt_oob_data.define(
        b"runtime error: index out of bounds\0"
            .to_vec()
            .into_boxed_slice(),
    );
    let fmt_oob_id = module
        .declare_data("fmt_oob", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_oob_id, &fmt_oob_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_lparen_data = DataDescription::new();
    fmt_lparen_data.define(b"(\0".to_vec().into_boxed_slice());
    let fmt_lparen_id = module
        .declare_data("fmt_lparen", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_lparen_id, &fmt_lparen_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_rparen_data = DataDescription::new();
    fmt_rparen_data.define(b")\n\0".to_vec().into_boxed_slice());
    let fmt_rparen_id = module
        .declare_data("fmt_rparen", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_rparen_id, &fmt_rparen_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_rparen_sp_data = DataDescription::new();
    fmt_rparen_sp_data.define(b") \0".to_vec().into_boxed_slice());
    let fmt_rparen_sp_id = module
        .declare_data("fmt_rparen_sp", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_rparen_sp_id, &fmt_rparen_sp_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_map_open_data = DataDescription::new();
    fmt_map_open_data.define(b"Map(size=\0".to_vec().into_boxed_slice());
    let fmt_map_open_id = module
        .declare_data("fmt_map_open", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_map_open_id, &fmt_map_open_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_map_err_data = DataDescription::new();
    fmt_map_err_data.define(
        b"runtime error: map_get(): key not found\0"
            .to_vec()
            .into_boxed_slice(),
    );
    let fmt_map_err_id = module
        .declare_data("fmt_map_err", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_map_err_id, &fmt_map_err_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_div0_msg_data = DataDescription::new();
    fmt_div0_msg_data.define(b"division by zero\0".to_vec().into_boxed_slice());
    let fmt_div0_msg_id = module
        .declare_data("fmt_div0_msg", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_div0_msg_id, &fmt_div0_msg_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_ovf_msg_data = DataDescription::new();
    fmt_ovf_msg_data.define(b"integer overflow\0".to_vec().into_boxed_slice());
    let fmt_ovf_msg_id = module
        .declare_data("fmt_ovf_msg", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_ovf_msg_id, &fmt_ovf_msg_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_oob_msg_data = DataDescription::new();
    fmt_oob_msg_data.define(b"index out of bounds\0".to_vec().into_boxed_slice());
    let fmt_oob_msg_id = module
        .declare_data("fmt_oob_msg", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_oob_msg_id, &fmt_oob_msg_data)
        .map_err(|e| e.to_string())?;

    let mut fmt_map_err_msg_data = DataDescription::new();
    fmt_map_err_msg_data.define(b"map_get(): key not found\0".to_vec().into_boxed_slice());
    let fmt_map_err_msg_id = module
        .declare_data("fmt_map_err_msg", Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(fmt_map_err_msg_id, &fmt_map_err_msg_data)
        .map_err(|e| e.to_string())?;

    // Declare user functions
    let func_ret_types: Vec<StackType> = program
        .functions
        .iter()
        .map(|f| match &f.return_type {
            Type::Float => StackType::Float,
            Type::String => StackType::Str,
            Type::Bool => StackType::Bool,
            _ => StackType::Int,
        })
        .collect();

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

    let tuple_types_registry: std::cell::RefCell<Vec<Vec<ElemType>>> =
        std::cell::RefCell::new(Vec::new());

    let enum_strs: std::cell::RefCell<
        std::collections::HashMap<(String, String), cranelift_module::DataId>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());

    let compile_chunk = |module: &mut ObjectModule,
                         builder: &mut FunctionBuilder,
                         chunk: &crate::compiler::ir::Chunk,
                         stack_slot: StackSlot,
                         locals_slot: StackSlot,
                         globals_gv: cranelift_codegen::ir::GlobalValue,
                         sp_var: Variable,
                         printf_ref: FuncRef,
                         strlen_ref: FuncRef,
                         malloc_ref: FuncRef,
                         memcpy_ref: FuncRef,
                         strcmp_ref: FuncRef,
                         exit_ref: FuncRef,
                         pow_ref: FuncRef,
                         floor_ref: FuncRef,
                         ceil_ref: FuncRef,
                         round_ref: FuncRef,
                         free_ref: FuncRef,
                         fmt_int_ptr: Value,
                         fmt_str_ptr: Value,
                         fmt_float_int_ptr: Value,
                         fmt_float_dec_ptr: Value,
                         fmt_nan_ptr: Value,
                         fmt_inf_ptr: Value,
                         fmt_neg_inf_ptr: Value,
                         fmt_true_ptr: Value,
                         fmt_false_ptr: Value,
                         fmt_int_sp_ptr: Value,
                         fmt_str_sp_ptr: Value,
                         fmt_float_dec_sp_ptr: Value,
                         fmt_div0_ptr: Value,
                         fmt_ovf_ptr: Value,
                         fmt_str_raw_ptr: Value,
                         fmt_lb_ptr: Value,
                         fmt_rb_ptr: Value,
                         fmt_rb_sp_ptr: Value,
                         fmt_comma_ptr: Value,
                         fmt_empty_ptr: Value,
                         fmt_int_elem_ptr: Value,
                         fmt_str_elem_ptr: Value,
                         fmt_float_dec_elem_ptr: Value,
                         fmt_oob_ptr: Value,
                         fmt_lparen_ptr: Value,
                         fmt_rparen_ptr: Value,
                         fmt_rparen_sp_ptr: Value,
                         fmt_map_open_ptr: Value,
                         fmt_map_err_ptr: Value,
                         fmt_div0_msg_ptr: Value,
                         fmt_ovf_msg_ptr: Value,
                         fmt_oob_msg_ptr: Value,
                         fmt_map_err_msg_ptr: Value,
                         param_types: &[Type],
                         struct_fields: &[StructInfo],
                         is_main: bool|
     -> Result<(), String> {
        let mut type_stack: Vec<StackType> = Vec::new();
        let mut local_types: HashMap<usize, StackType> = HashMap::new();
        let try_state_slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 1024));
        let mut try_depth: usize = 0;
        let mut handler_stack: Vec<cranelift_codegen::ir::Block> = Vec::new();
        let mut pending_seals: Vec<cranelift_codegen::ir::Block> = Vec::new();
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
            let this_block = blocks[ip];
            if pending_seals.contains(&this_block) {
                pending_seals.retain(|&b| b != this_block);
                type_stack.clear();
                type_stack.push(StackType::Str);
            }
            builder.switch_to_block(this_block);
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
                    type_stack.push(StackType::Bool);
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
                            StackType::Struct(_) => {
                                return Err("struct -> int not supported".to_string());
                            }
                            StackType::Int => (val, StackType::Int),
                            StackType::Bool => (val, StackType::Int),
                            StackType::List(_) => {
                                return Err("list -> int not supported in native".to_string());
                            }
                            StackType::Tuple(_) => {
                                return Err("tuple -> int not supported in native".to_string());
                            }
                            StackType::Map => {
                                return Err("map -> int not supported in native".to_string());
                            }
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
                            StackType::Struct(_) => {
                                return Err("struct -> float not supported".to_string());
                            }
                            StackType::Int => {
                                let f = builder.ins().fcvt_from_sint(types::F64, val);
                                let bits = builder.ins().bitcast(types::I64, MemFlags::new(), f);
                                (bits, StackType::Float)
                            }
                            StackType::Float => (val, StackType::Float),
                            StackType::Bool => {
                                let f = builder.ins().fcvt_from_sint(types::F64, val);
                                let bits = builder.ins().bitcast(types::I64, MemFlags::new(), f);
                                (bits, StackType::Float)
                            }
                            StackType::List(_) => {
                                return Err("list -> float not supported in native".to_string());
                            }
                            StackType::Tuple(_) => {
                                return Err("tuple -> float not supported in native".to_string());
                            }
                            StackType::Map => {
                                return Err("map -> float not supported in native".to_string());
                            }
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

                Instruction::PushEnum(name, variant) => {
                    let key = (name.clone(), variant.clone());
                    let data_id = {
                        let mut reg = enum_strs.borrow_mut();
                        if let Some(&existing) = reg.get(&key) {
                            existing
                        } else {
                            let mut data = DataDescription::new();
                            let mut bytes = format!("{}.{}", name, variant).into_bytes();
                            bytes.push(0);
                            data.define(bytes.into_boxed_slice());
                            let n = reg.len();
                            let new_id = module
                                .declare_data(
                                    &format!("enum_str_{}", n),
                                    Linkage::Local,
                                    false,
                                    false,
                                )
                                .map_err(|e| e.to_string())?;
                            module
                                .define_data(new_id, &data)
                                .map_err(|e| e.to_string())?;
                            reg.insert(key, new_id);
                            new_id
                        }
                    };
                    let gv = module.declare_data_in_func(data_id, builder.func);
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
                Instruction::MapGet(val_ty) => {
                    let key_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let map_ty = type_stack.pop().unwrap_or(StackType::Map);
                    if !matches!(map_ty, StackType::Map) {
                        return Err("map_get() needs a Map".to_string());
                    }
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let key_pos = builder.ins().iadd_imm(sp, -1);
                    let key_off = builder.ins().imul_imm(key_pos, 8);
                    let key_addr = builder.ins().iadd(stack_addr, key_off);
                    let key = builder.ins().load(types::I64, MemFlags::new(), key_addr, 0);
                    let map_pos = builder.ins().iadd_imm(sp, -2);
                    let map_off = builder.ins().imul_imm(map_pos, 8);
                    let map_addr = builder.ins().iadd(stack_addr, map_off);
                    let map_ptr = builder.ins().load(types::I64, MemFlags::new(), map_addr, 0);
                    let key_tag_val: i64 = match key_ty {
                        StackType::Float => 2,
                        StackType::Str => 3,
                        StackType::Bool => 4,
                        _ => 1,
                    };
                    let key_tag = builder.ins().iconst(types::I64, key_tag_val);
                    let len = builder.ins().load(types::I64, MemFlags::new(), map_ptr, 8);
                    let neg_one = builder.ins().iconst(types::I64, -1);
                    builder.def_var(Variable::new(2), neg_one);
                    let zero_i = builder.ins().iconst(types::I64, 0);
                    builder.def_var(Variable::new(3), zero_i);
                    let header = builder.create_block();
                    let body = builder.create_block();
                    let after = builder.create_block();
                    builder.ins().jump(header, &[]);
                    builder.switch_to_block(header);
                    let ci = builder.use_var(Variable::new(3));
                    let in_range = builder.ins().icmp(IntCC::SignedLessThan, ci, len);
                    builder.ins().brif(in_range, body, &[], after, &[]);
                    builder.seal_block(body);
                    builder.seal_block(after);
                    builder.switch_to_block(body);
                    let ci2 = builder.use_var(Variable::new(3));
                    let tag_off = builder.ins().imul_imm(ci2, 24);
                    let tag_pos = builder.ins().iadd_imm(tag_off, 16);
                    let tag_ptr = builder.ins().iadd(map_ptr, tag_pos);
                    let tag_i = builder.ins().load(types::I64, MemFlags::new(), tag_ptr, 0);
                    let tags_match = builder.ins().icmp(IntCC::Equal, tag_i, key_tag);
                    let key_off_b = builder.ins().imul_imm(ci2, 24);
                    let key_pos_b = builder.ins().iadd_imm(key_off_b, 24);
                    let key_ptr_b = builder.ins().iadd(map_ptr, key_pos_b);
                    let key_i = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), key_ptr_b, 0);
                    let cmp_result = match key_ty {
                        StackType::Str => {
                            let sc = builder.ins().call(strcmp_ref, &[key_i, key]);
                            let sr = builder.inst_results(sc)[0];
                            let zero_i32 = builder.ins().iconst(types::I32, 0);
                            builder.ins().icmp(IntCC::Equal, sr, zero_i32)
                        }
                        StackType::Float => {
                            let a = builder.ins().bitcast(types::F64, MemFlags::new(), key_i);
                            let b = builder.ins().bitcast(types::F64, MemFlags::new(), key);
                            builder.ins().fcmp(FloatCC::Equal, a, b)
                        }
                        _ => builder.ins().icmp(IntCC::Equal, key_i, key),
                    };
                    let match_both = builder.ins().band(tags_match, cmp_result);
                    let current = builder.use_var(Variable::new(2));
                    let next_found = builder.ins().select(match_both, ci2, current);
                    builder.def_var(Variable::new(2), next_found);
                    let next_ci = builder.ins().iadd_imm(ci2, 1);
                    builder.def_var(Variable::new(3), next_ci);
                    builder.ins().jump(header, &[]);
                    builder.seal_block(header);
                    builder.switch_to_block(after);
                    let found = builder.use_var(Variable::new(2));
                    let found_any =
                        builder
                            .ins()
                            .icmp_imm(IntCC::SignedGreaterThanOrEqual, found, 0);
                    let hit_blk = builder.create_block();
                    let miss_blk = builder.create_block();
                    builder.ins().brif(found_any, hit_blk, &[], miss_blk, &[]);
                    builder.seal_block(hit_blk);
                    builder.seal_block(miss_blk);
                    builder.switch_to_block(hit_blk);
                    let fv = builder.use_var(Variable::new(2));
                    let val_off_found = builder.ins().imul_imm(fv, 24);
                    let val_pos = builder.ins().iadd_imm(val_off_found, 32);
                    let val_addr = builder.ins().iadd(map_ptr, val_pos);
                    let result_val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
                    builder
                        .ins()
                        .store(MemFlags::new(), result_val, map_addr, 0);
                    let sp_after = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, sp_after);
                    type_stack.push(match val_ty {
                        Type::String => StackType::Str,
                        Type::Float => StackType::Float,
                        Type::Bool => StackType::Bool,
                        _ => StackType::Int,
                    });
                    builder.ins().jump(blocks[ip + 1], &[]);
                    builder.switch_to_block(miss_blk);
                    if try_depth > 0 {
                        let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                        let off = builder
                            .ins()
                            .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                        let saved_src = builder.ins().iadd(base, off);
                        let saved_sp =
                            builder
                                .ins()
                                .load(types::I64, MemFlags::new(), saved_src, 0);
                        let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let m_off = builder.ins().imul_imm(saved_sp, 8);
                        let m_dest = builder.ins().iadd(sp2, m_off);
                        builder
                            .ins()
                            .store(MemFlags::new(), fmt_map_err_msg_ptr, m_dest, 0);
                        let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                        builder.def_var(sp_var, new_sp_v);
                        let handler = *handler_stack.last().unwrap();
                        if !pending_seals.contains(&handler) {
                            pending_seals.push(handler);
                        }
                        builder.ins().jump(handler, &[]);
                    } else {
                        builder
                            .ins()
                            .call(printf_ref, &[fmt_str_ptr, fmt_map_err_ptr]);
                        let exit_arg = builder.ins().iconst(types::I32, 1);
                        builder.ins().call(exit_ref, &[exit_arg]);
                        if is_main {
                            let ret = builder.ins().iconst(types::I32, 1);
                            builder.ins().return_(&[ret]);
                        } else {
                            let ret = builder.ins().iconst(types::I64, 0);
                            builder.ins().return_(&[ret]);
                        }
                    }
                }
                Instruction::MapSet => {
                    let val_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let key_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let map_ty = type_stack.pop().unwrap_or(StackType::Map);
                    if !matches!(map_ty, StackType::Map) {
                        return Err("map_set() needs a Map".to_string());
                    }
                    let _ = val_ty;
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let val_pos = builder.ins().iadd_imm(sp, -1);
                    let val_off = builder.ins().imul_imm(val_pos, 8);
                    let val_addr = builder.ins().iadd(stack_addr, val_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
                    let key_pos = builder.ins().iadd_imm(sp, -2);
                    let key_off = builder.ins().imul_imm(key_pos, 8);
                    let key_addr = builder.ins().iadd(stack_addr, key_off);
                    let key = builder.ins().load(types::I64, MemFlags::new(), key_addr, 0);
                    let map_pos = builder.ins().iadd_imm(sp, -3);
                    let map_off = builder.ins().imul_imm(map_pos, 8);
                    let map_addr = builder.ins().iadd(stack_addr, map_off);
                    let map_ptr = builder.ins().load(types::I64, MemFlags::new(), map_addr, 0);
                    let key_tag_val: i64 = match key_ty {
                        StackType::Float => 2,
                        StackType::Str => 3,
                        StackType::Bool => 4,
                        _ => 1,
                    };
                    let key_tag = builder.ins().iconst(types::I64, key_tag_val);
                    let len = builder.ins().load(types::I64, MemFlags::new(), map_ptr, 8);
                    let neg_one = builder.ins().iconst(types::I64, -1);
                    builder.def_var(Variable::new(2), neg_one);
                    let zero_i = builder.ins().iconst(types::I64, 0);
                    builder.def_var(Variable::new(3), zero_i);
                    let header = builder.create_block();
                    let body = builder.create_block();
                    let after_lookup = builder.create_block();
                    builder.ins().jump(header, &[]);
                    builder.switch_to_block(header);
                    let ci = builder.use_var(Variable::new(3));
                    let in_range = builder.ins().icmp(IntCC::SignedLessThan, ci, len);
                    builder.ins().brif(in_range, body, &[], after_lookup, &[]);
                    builder.seal_block(body);
                    builder.seal_block(after_lookup);
                    builder.switch_to_block(body);
                    let ci2 = builder.use_var(Variable::new(3));
                    let tag_off = builder.ins().imul_imm(ci2, 24);
                    let tag_pos = builder.ins().iadd_imm(tag_off, 16);
                    let tag_ptr = builder.ins().iadd(map_ptr, tag_pos);
                    let tag_i = builder.ins().load(types::I64, MemFlags::new(), tag_ptr, 0);
                    let tags_match = builder.ins().icmp(IntCC::Equal, tag_i, key_tag);
                    let key_off_b = builder.ins().imul_imm(ci2, 24);
                    let key_pos_b = builder.ins().iadd_imm(key_off_b, 24);
                    let key_ptr_b = builder.ins().iadd(map_ptr, key_pos_b);
                    let key_i = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), key_ptr_b, 0);
                    let cmp_result = match key_ty {
                        StackType::Str => {
                            let sc = builder.ins().call(strcmp_ref, &[key_i, key]);
                            let sr = builder.inst_results(sc)[0];
                            let zero_i32 = builder.ins().iconst(types::I32, 0);
                            builder.ins().icmp(IntCC::Equal, sr, zero_i32)
                        }
                        StackType::Float => {
                            let a = builder.ins().bitcast(types::F64, MemFlags::new(), key_i);
                            let b = builder.ins().bitcast(types::F64, MemFlags::new(), key);
                            builder.ins().fcmp(FloatCC::Equal, a, b)
                        }
                        _ => builder.ins().icmp(IntCC::Equal, key_i, key),
                    };
                    let match_both = builder.ins().band(tags_match, cmp_result);
                    let current = builder.use_var(Variable::new(2));
                    let next_found = builder.ins().select(match_both, ci2, current);
                    builder.def_var(Variable::new(2), next_found);
                    let next_ci = builder.ins().iadd_imm(ci2, 1);
                    builder.def_var(Variable::new(3), next_ci);
                    builder.ins().jump(header, &[]);
                    builder.seal_block(header);
                    builder.switch_to_block(after_lookup);
                    let found = builder.use_var(Variable::new(2));
                    let found_any =
                        builder
                            .ins()
                            .icmp_imm(IntCC::SignedGreaterThanOrEqual, found, 0);
                    let upd_blk = builder.create_block();
                    let ins_blk = builder.create_block();
                    let after_all = builder.create_block();
                    builder.ins().brif(found_any, upd_blk, &[], ins_blk, &[]);
                    builder.seal_block(upd_blk);
                    builder.seal_block(ins_blk);
                    builder.switch_to_block(upd_blk);
                    let fv = builder.use_var(Variable::new(2));
                    let upd_off = builder.ins().imul_imm(fv, 24);
                    let upd_pos = builder.ins().iadd_imm(upd_off, 32);
                    let upd_ptr = builder.ins().iadd(map_ptr, upd_pos);
                    builder.ins().store(MemFlags::new(), val, upd_ptr, 0);
                    builder.ins().store(MemFlags::new(), map_ptr, map_addr, 0);
                    builder.ins().jump(after_all, &[]);
                    builder.switch_to_block(ins_blk);
                    let new_len = builder.ins().iadd_imm(len, 1);
                    let slots = builder.ins().imul_imm(new_len, 3);
                    let slots_p = builder.ins().iadd_imm(slots, 2);
                    let bytes = builder.ins().imul_imm(slots_p, 8);
                    let alloc = builder.ins().call(malloc_ref, &[bytes]);
                    let new_ptr = builder.inst_results(alloc)[0];
                    let old_slots = builder.ins().imul_imm(len, 3);
                    let old_slots_p = builder.ins().iadd_imm(old_slots, 2);
                    let old_bytes = builder.ins().imul_imm(old_slots_p, 8);
                    builder
                        .ins()
                        .call(memcpy_ref, &[new_ptr, map_ptr, old_bytes]);
                    let entry_off = builder.ins().imul_imm(len, 24);
                    let entry_pos = builder.ins().iadd_imm(entry_off, 16);
                    let entry_ptr = builder.ins().iadd(new_ptr, entry_pos);
                    builder.ins().store(MemFlags::new(), key_tag, entry_ptr, 0);
                    let entry_key_p = builder.ins().iadd_imm(entry_ptr, 8);
                    builder.ins().store(MemFlags::new(), key, entry_key_p, 0);
                    let entry_val_p = builder.ins().iadd_imm(entry_ptr, 16);
                    builder.ins().store(MemFlags::new(), val, entry_val_p, 0);
                    let one_rc = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one_rc, new_ptr, 0);
                    builder.ins().store(MemFlags::new(), new_len, new_ptr, 8);
                    builder.ins().store(MemFlags::new(), new_ptr, map_addr, 0);
                    builder.ins().jump(after_all, &[]);
                    builder.seal_block(after_all);
                    builder.switch_to_block(after_all);
                    let sp_after = builder.ins().iadd_imm(sp, -2);
                    builder.def_var(sp_var, sp_after);
                    type_stack.push(StackType::Map);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MapHas => {
                    let key_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let map_ty = type_stack.pop().unwrap_or(StackType::Map);
                    if !matches!(map_ty, StackType::Map) {
                        return Err("map_has() needs a Map".to_string());
                    }
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let key_pos = builder.ins().iadd_imm(sp, -1);
                    let key_off = builder.ins().imul_imm(key_pos, 8);
                    let key_addr = builder.ins().iadd(stack_addr, key_off);
                    let key = builder.ins().load(types::I64, MemFlags::new(), key_addr, 0);
                    let map_pos = builder.ins().iadd_imm(sp, -2);
                    let map_off = builder.ins().imul_imm(map_pos, 8);
                    let map_addr = builder.ins().iadd(stack_addr, map_off);
                    let map_ptr = builder.ins().load(types::I64, MemFlags::new(), map_addr, 0);
                    let key_tag_val: i64 = match key_ty {
                        StackType::Float => 2,
                        StackType::Str => 3,
                        StackType::Bool => 4,
                        _ => 1,
                    };
                    let key_tag = builder.ins().iconst(types::I64, key_tag_val);
                    let len = builder.ins().load(types::I64, MemFlags::new(), map_ptr, 8);
                    let zero_i8 = builder.ins().iconst(types::I8, 0);
                    builder.def_var(Variable::new(4), zero_i8);
                    let zero_i64 = builder.ins().iconst(types::I64, 0);
                    builder.def_var(Variable::new(5), zero_i64);
                    let header = builder.create_block();
                    let body = builder.create_block();
                    let after = builder.create_block();
                    builder.ins().jump(header, &[]);
                    builder.switch_to_block(header);
                    let ci = builder.use_var(Variable::new(5));
                    let in_range = builder.ins().icmp(IntCC::SignedLessThan, ci, len);
                    builder.ins().brif(in_range, body, &[], after, &[]);
                    builder.seal_block(body);
                    builder.seal_block(after);
                    builder.switch_to_block(body);
                    let ci2 = builder.use_var(Variable::new(5));
                    let tag_off = builder.ins().imul_imm(ci2, 24);
                    let tag_pos = builder.ins().iadd_imm(tag_off, 16);
                    let tag_ptr = builder.ins().iadd(map_ptr, tag_pos);
                    let tag_i = builder.ins().load(types::I64, MemFlags::new(), tag_ptr, 0);
                    let tags_match = builder.ins().icmp(IntCC::Equal, tag_i, key_tag);
                    let key_off_b = builder.ins().imul_imm(ci2, 24);
                    let key_pos_b = builder.ins().iadd_imm(key_off_b, 24);
                    let key_ptr_b = builder.ins().iadd(map_ptr, key_pos_b);
                    let key_i = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), key_ptr_b, 0);
                    let cmp_result = match key_ty {
                        StackType::Str => {
                            let sc = builder.ins().call(strcmp_ref, &[key_i, key]);
                            let sr = builder.inst_results(sc)[0];
                            let zero_i32 = builder.ins().iconst(types::I32, 0);
                            builder.ins().icmp(IntCC::Equal, sr, zero_i32)
                        }
                        StackType::Float => {
                            let a = builder.ins().bitcast(types::F64, MemFlags::new(), key_i);
                            let b = builder.ins().bitcast(types::F64, MemFlags::new(), key);
                            builder.ins().fcmp(FloatCC::Equal, a, b)
                        }
                        _ => builder.ins().icmp(IntCC::Equal, key_i, key),
                    };
                    let match_both = builder.ins().band(tags_match, cmp_result);
                    let current = builder.use_var(Variable::new(4));
                    let next_r = builder.ins().bor(current, match_both);
                    builder.def_var(Variable::new(4), next_r);
                    let next_ci = builder.ins().iadd_imm(ci2, 1);
                    builder.def_var(Variable::new(5), next_ci);
                    builder.ins().jump(header, &[]);
                    builder.seal_block(header);
                    builder.switch_to_block(after);
                    let result_i8 = builder.use_var(Variable::new(4));
                    let result_i64 = builder.ins().uextend(types::I64, result_i8);
                    builder
                        .ins()
                        .store(MemFlags::new(), result_i64, map_addr, 0);
                    let sp_after = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, sp_after);
                    type_stack.push(StackType::Bool);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MapNew => {
                    let size = builder.ins().iconst(types::I64, 16);
                    let malloc_call = builder.ins().call(malloc_ref, &[size]);
                    let ptr = builder.inst_results(malloc_call)[0];
                    let one = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one, ptr, 0);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().store(MemFlags::new(), zero, ptr, 8);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    builder.ins().store(MemFlags::new(), ptr, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Map);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MapLen => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    if !matches!(ty, StackType::Map) {
                        return Err("map_len() needs a Map".to_string());
                    }
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let len = builder.ins().load(types::I64, MemFlags::new(), val, 8);
                    builder.ins().store(MemFlags::new(), len, src, 0);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MakeTuple(n) => {
                    let mut popped = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        popped.push(type_stack.pop().unwrap_or(StackType::Int));
                    }
                    popped.reverse();
                    let elem_types: Vec<ElemType> = popped
                        .iter()
                        .map(|st| match st {
                            StackType::Int => ElemType::Int,
                            StackType::Float => ElemType::Float,
                            StackType::Str => ElemType::Str,
                            StackType::Bool => ElemType::Bool,
                            _ => ElemType::Int,
                        })
                        .collect();
                    let tuple_id = {
                        let mut reg = tuple_types_registry.borrow_mut();
                        let id = reg.len() as u32;
                        reg.push(elem_types);
                        id
                    };
                    let n_i = *n as i64;
                    let sp = builder.use_var(sp_var);
                    let sp_new = builder.ins().iadd_imm(sp, -n_i);
                    let total_size = builder.ins().iconst(types::I64, (n_i + 2) * 8);
                    let malloc_call = builder.ins().call(malloc_ref, &[total_size]);
                    let ptr = builder.inst_results(malloc_call)[0];
                    let one = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one, ptr, 0);
                    let len_val = builder.ins().iconst(types::I64, n_i);
                    builder.ins().store(MemFlags::new(), len_val, ptr, 8);
                    for i in 0..*n {
                        let src_idx = builder.ins().iadd_imm(sp_new, i as i64);
                        let src_off = builder.ins().imul_imm(src_idx, 8);
                        let src_base = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src_final = builder.ins().iadd(src_base, src_off);
                        let val = builder
                            .ins()
                            .load(types::I64, MemFlags::new(), src_final, 0);
                        let dst_off = builder.ins().iconst(types::I64, ((i as i64) + 2) * 8);
                        let dst_final = builder.ins().iadd(ptr, dst_off);
                        builder.ins().store(MemFlags::new(), val, dst_final, 0);
                    }
                    let dst_off2 = builder.ins().imul_imm(sp_new, 8);
                    let dst_base2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dst_final2 = builder.ins().iadd(dst_base2, dst_off2);
                    builder.ins().store(MemFlags::new(), ptr, dst_final2, 0);
                    let sp_inc = builder.ins().iadd_imm(sp_new, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Tuple(tuple_id));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MakeList(n) => {
                    let sp = builder.use_var(sp_var);
                    let n_i = *n as i64;
                    let sp_new = builder.ins().iadd_imm(sp, -n_i);
                    let total_size = builder.ins().iconst(types::I64, (n_i + 2) * 8);
                    let malloc_call = builder.ins().call(malloc_ref, &[total_size]);
                    let ptr = builder.inst_results(malloc_call)[0];
                    let one = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one, ptr, 0);
                    let len_val = builder.ins().iconst(types::I64, n_i);
                    builder.ins().store(MemFlags::new(), len_val, ptr, 8);
                    for i in 0..*n {
                        let src_idx = builder.ins().iadd_imm(sp_new, i as i64);
                        let src_off = builder.ins().imul_imm(src_idx, 8);
                        let src_base = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src_final = builder.ins().iadd(src_base, src_off);
                        let val = builder
                            .ins()
                            .load(types::I64, MemFlags::new(), src_final, 0);
                        let dst_off = builder.ins().iconst(types::I64, ((i as i64) + 2) * 8);
                        let dst_final = builder.ins().iadd(ptr, dst_off);
                        builder.ins().store(MemFlags::new(), val, dst_final, 0);
                    }
                    let dst_off2 = builder.ins().imul_imm(sp_new, 8);
                    let dst_base2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dst_final2 = builder.ins().iadd(dst_base2, dst_off2);
                    builder.ins().store(MemFlags::new(), ptr, dst_final2, 0);
                    let sp_inc = builder.ins().iadd_imm(sp_new, 1);
                    builder.def_var(sp_var, sp_inc);
                    let mut elem_ty = ElemType::Int;
                    for i in 0..*n {
                        let popped = type_stack.pop().unwrap_or(StackType::Int);
                        if i + 1 == *n {
                            elem_ty = match popped {
                                StackType::Int => ElemType::Int,
                                StackType::Float => ElemType::Float,
                                StackType::Str => ElemType::Str,
                                StackType::Bool => ElemType::Bool,
                                _ => ElemType::Int,
                            };
                        }
                    }
                    type_stack.push(StackType::List(elem_ty));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MakeStruct(id) => {
                    let info = &struct_fields[*id];
                    let n = info.fields.len();
                    let size_bytes = (n as i64) * 8;
                    let sp = builder.use_var(sp_var);
                    let sp_new = builder.ins().iadd_imm(sp, -(n as i64));
                    let size_c = builder.ins().iconst(types::I64, size_bytes + 8);
                    let malloc_call = builder.ins().call(malloc_ref, &[size_c]);
                    let ptr = builder.inst_results(malloc_call)[0];
                    let one = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one, ptr, 0);
                    for i in 0..n {
                        let src_idx = builder.ins().iadd_imm(sp_new, i as i64);
                        let src_off = builder.ins().imul_imm(src_idx, 8);
                        let src_base = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src_final = builder.ins().iadd(src_base, src_off);
                        let val = builder
                            .ins()
                            .load(types::I64, MemFlags::new(), src_final, 0);
                        let dst_off = builder.ins().iconst(types::I64, ((i as i64) + 1) * 8);
                        let dst_final = builder.ins().iadd(ptr, dst_off);
                        builder.ins().store(MemFlags::new(), val, dst_final, 0);
                    }
                    let dst_off2 = builder.ins().imul_imm(sp_new, 8);
                    let dst_base2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dst_final2 = builder.ins().iadd(dst_base2, dst_off2);
                    builder.ins().store(MemFlags::new(), ptr, dst_final2, 0);
                    let sp_inc = builder.ins().iadd_imm(sp_new, 1);
                    builder.def_var(sp_var, sp_inc);
                    for _ in 0..n {
                        type_stack.pop();
                    }
                    type_stack.push(StackType::Struct(*id));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::GetField(name) => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let struct_id = match ty {
                        StackType::Struct(id) => id,
                        _ => return Err("GetField on non-struct".to_string()),
                    };
                    let info = &struct_fields[struct_id];
                    let field_idx = info
                        .fields
                        .iter()
                        .position(|f| f == name)
                        .ok_or_else(|| format!("no field {}", name))?;
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    let ptr_off = builder.ins().imul_imm(new_sp, 8);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let ptr_addr = builder.ins().iadd(stack_addr, ptr_off);
                    let ptr = builder.ins().load(types::I64, MemFlags::new(), ptr_addr, 0);
                    let field_off = builder
                        .ins()
                        .iconst(types::I64, ((field_idx as i64) + 1) * 8);
                    let field_addr = builder.ins().iadd(ptr, field_off);
                    let val = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), field_addr, 0);
                    let sp_off2 = builder.ins().imul_imm(new_sp, 8);
                    let sp_addr2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr2, sp_off2);
                    builder.ins().store(MemFlags::new(), val, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(new_sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    let field_type = &info.field_types[field_idx];
                    let st = match field_type {
                        Type::Int | Type::Bool => StackType::Int,
                        Type::Float => StackType::Float,
                        Type::String => StackType::Str,
                        _ => StackType::Int,
                    };
                    type_stack.push(st);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::SetField(name) => {
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let ptr_off = builder.ins().imul_imm(new_sp, 8);
                    let ptr_addr = builder.ins().iadd(stack_addr, ptr_off);
                    let ptr = builder.ins().load(types::I64, MemFlags::new(), ptr_addr, 0);
                    let sp_after_ptr = builder.ins().iadd_imm(new_sp, -1);
                    let val_off = builder.ins().imul_imm(sp_after_ptr, 8);
                    let val_addr = builder.ins().iadd(stack_addr, val_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
                    let struct_ty = type_stack.pop().unwrap_or(StackType::Int);
                    type_stack.pop();
                    let struct_id = match struct_ty {
                        StackType::Struct(id) => id,
                        _ => return Err("SetField on non-struct".to_string()),
                    };
                    let info = &struct_fields[struct_id];
                    let field_idx = info
                        .fields
                        .iter()
                        .position(|f| f == name)
                        .ok_or_else(|| format!("no field {}", name))?;
                    let field_off = builder
                        .ins()
                        .iconst(types::I64, ((field_idx as i64) + 1) * 8);
                    let field_addr = builder.ins().iadd(ptr, field_off);
                    builder.ins().store(MemFlags::new(), val, field_addr, 0);
                    let dst_off = builder.ins().imul_imm(sp_after_ptr, 8);
                    let dst_addr = builder.ins().iadd(stack_addr, dst_off);
                    builder.ins().store(MemFlags::new(), ptr, dst_addr, 0);
                    let sp_inc = builder.ins().iadd_imm(sp_after_ptr, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Struct(struct_id));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::PushNone => {
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest = builder.ins().iadd(sp_addr, sp_off);
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.ins().store(MemFlags::new(), zero, dest, 0);
                    let sp_inc = builder.ins().iadd_imm(sp, 1);
                    builder.def_var(sp_var, sp_inc);
                    type_stack.push(StackType::Int);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::Pop => {
                    let sp = builder.use_var(sp_var);
                    let sp_dec = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, sp_dec);
                    type_stack.pop();
                    builder.ins().jump(blocks[ip + 1], &[]);
                }

                Instruction::Print(n) => {
                    let sp_start = builder.use_var(sp_var);
                    let n_i64 = *n as i64;
                    let mut types = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        types.push(type_stack.pop().unwrap_or(StackType::Int));
                    }
                    types.reverse();
                    for (i, ty) in types.into_iter().enumerate() {
                        let is_last = i + 1 == *n;
                        let pos = builder.ins().iadd_imm(sp_start, -n_i64 + (i as i64));
                        let pos_off = builder.ins().imul_imm(pos, 8);
                        let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let src = builder.ins().iadd(stack_addr, pos_off);
                        let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                        match ty {
                            StackType::Struct(_) => {
                                return Err("struct display not supported".to_string());
                            }
                            StackType::Int => {
                                let fmt = if is_last { fmt_int_ptr } else { fmt_int_sp_ptr };
                                builder.ins().call(printf_ref, &[fmt, val]);
                            }
                            StackType::Str => {
                                let fmt = if is_last { fmt_str_ptr } else { fmt_str_sp_ptr };
                                builder.ins().call(printf_ref, &[fmt, val]);
                            }
                            StackType::Bool => {
                                let non_zero = builder.ins().icmp_imm(IntCC::NotEqual, val, 0);
                                let ptr =
                                    builder.ins().select(non_zero, fmt_true_ptr, fmt_false_ptr);
                                let fmt = if is_last { fmt_str_ptr } else { fmt_str_sp_ptr };
                                builder.ins().call(printf_ref, &[fmt, ptr]);
                            }
                            StackType::Tuple(tuple_id) => {
                                let elem_types = {
                                    let reg = tuple_types_registry.borrow();
                                    reg[tuple_id as usize].clone()
                                };
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, fmt_lparen_ptr]);
                                for (idx, et) in elem_types.iter().enumerate() {
                                    if idx > 0 {
                                        builder
                                            .ins()
                                            .call(printf_ref, &[fmt_str_raw_ptr, fmt_comma_ptr]);
                                    }
                                    let off =
                                        builder.ins().iconst(types::I64, ((idx + 2) as i64) * 8);
                                    let addr = builder.ins().iadd(val, off);
                                    let elem =
                                        builder.ins().load(types::I64, MemFlags::new(), addr, 0);
                                    match et {
                                        ElemType::Int => {
                                            builder
                                                .ins()
                                                .call(printf_ref, &[fmt_int_elem_ptr, elem]);
                                        }
                                        ElemType::Str => {
                                            builder
                                                .ins()
                                                .call(printf_ref, &[fmt_str_elem_ptr, elem]);
                                        }
                                        ElemType::Bool => {
                                            let nz =
                                                builder.ins().icmp_imm(IntCC::NotEqual, elem, 0);
                                            let bptr = builder.ins().select(
                                                nz,
                                                fmt_true_ptr,
                                                fmt_false_ptr,
                                            );
                                            builder
                                                .ins()
                                                .call(printf_ref, &[fmt_str_raw_ptr, bptr]);
                                        }
                                        ElemType::Float => {
                                            let fe = builder.ins().bitcast(
                                                types::F64,
                                                MemFlags::new(),
                                                elem,
                                            );
                                            let th = builder.ins().f64const(1000.0);
                                            let sc = builder.ins().fmul(fe, th);
                                            let si = builder.ins().fcvt_to_sint(types::I64, sc);
                                            let dv = builder.ins().iconst(types::I64, 1000);
                                            let ip = builder.ins().sdiv(si, dv);
                                            let dr = builder.ins().srem(si, dv);
                                            let z3 = builder.ins().iconst(types::I64, 0);
                                            let neg =
                                                builder.ins().icmp(IntCC::SignedLessThan, dr, z3);
                                            let nd = builder.ins().ineg(dr);
                                            let dp = builder.ins().select(neg, nd, dr);
                                            builder
                                                .ins()
                                                .call(printf_ref, &[fmt_float_int_ptr, ip]);
                                            builder
                                                .ins()
                                                .call(printf_ref, &[fmt_float_dec_elem_ptr, dp]);
                                        }
                                    }
                                }
                                let close_ptr = if is_last {
                                    fmt_rparen_ptr
                                } else {
                                    fmt_rparen_sp_ptr
                                };
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, close_ptr]);
                            }
                            StackType::Map => {
                                let len = builder.ins().load(types::I64, MemFlags::new(), val, 0);
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, fmt_map_open_ptr]);
                                builder.ins().call(printf_ref, &[fmt_int_elem_ptr, len]);
                                let close_ptr = if is_last {
                                    fmt_rparen_ptr
                                } else {
                                    fmt_rparen_sp_ptr
                                };
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, close_ptr]);
                            }
                            StackType::List(elem_ty) => {
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, fmt_lb_ptr]);
                                let counter_var = Variable::new(1);
                                let zero_c = builder.ins().iconst(types::I64, 0);
                                builder.def_var(counter_var, zero_c);
                                let header = builder.create_block();
                                let body = builder.create_block();
                                let after = builder.create_block();
                                builder.ins().jump(header, &[]);
                                builder.switch_to_block(header);
                                let ci = builder.use_var(counter_var);
                                let clen = builder.ins().load(types::I64, MemFlags::new(), val, 8);
                                let in_range = builder.ins().icmp(IntCC::SignedLessThan, ci, clen);
                                builder.ins().brif(in_range, body, &[], after, &[]);
                                builder.seal_block(body);
                                builder.seal_block(after);
                                builder.switch_to_block(body);
                                let ci2 = builder.use_var(counter_var);
                                let zero2 = builder.ins().iconst(types::I64, 0);
                                let has_sep =
                                    builder.ins().icmp(IntCC::SignedGreaterThan, ci2, zero2);
                                let sep_ptr =
                                    builder.ins().select(has_sep, fmt_comma_ptr, fmt_empty_ptr);
                                builder.ins().call(printf_ref, &[fmt_str_raw_ptr, sep_ptr]);
                                let elem_idx = builder.ins().iadd_imm(ci2, 2);
                                let elem_off = builder.ins().imul_imm(elem_idx, 8);
                                let elem_addr = builder.ins().iadd(val, elem_off);
                                let elem =
                                    builder
                                        .ins()
                                        .load(types::I64, MemFlags::new(), elem_addr, 0);
                                match elem_ty {
                                    ElemType::Int => {
                                        builder.ins().call(printf_ref, &[fmt_int_elem_ptr, elem]);
                                    }
                                    ElemType::Str => {
                                        builder.ins().call(printf_ref, &[fmt_str_elem_ptr, elem]);
                                    }
                                    ElemType::Bool => {
                                        let nz = builder.ins().icmp_imm(IntCC::NotEqual, elem, 0);
                                        let bptr =
                                            builder.ins().select(nz, fmt_true_ptr, fmt_false_ptr);
                                        builder.ins().call(printf_ref, &[fmt_str_raw_ptr, bptr]);
                                    }
                                    ElemType::Float => {
                                        let fe = builder.ins().bitcast(
                                            types::F64,
                                            MemFlags::new(),
                                            elem,
                                        );
                                        let is_nan = builder.ins().fcmp(FloatCC::NotEqual, fe, fe);
                                        let pos_inf = builder.ins().f64const(f64::INFINITY);
                                        let neg_inf = builder.ins().f64const(f64::NEG_INFINITY);
                                        let is_pi = builder.ins().fcmp(FloatCC::Equal, fe, pos_inf);
                                        let is_ni = builder.ins().fcmp(FloatCC::Equal, fe, neg_inf);
                                        let is_inf = builder.ins().bor(is_pi, is_ni);
                                        let is_sp = builder.ins().bor(is_nan, is_inf);
                                        let sp_blk = builder.create_block();
                                        let fin_blk = builder.create_block();
                                        let after_e = builder.create_block();
                                        builder.ins().brif(is_sp, sp_blk, &[], fin_blk, &[]);
                                        builder.seal_block(sp_blk);
                                        builder.seal_block(fin_blk);
                                        builder.switch_to_block(sp_blk);
                                        let inf_ptr = builder.ins().select(
                                            is_ni,
                                            fmt_neg_inf_ptr,
                                            fmt_inf_ptr,
                                        );
                                        let fp = builder.ins().select(is_nan, fmt_nan_ptr, inf_ptr);
                                        builder.ins().call(printf_ref, &[fmt_str_raw_ptr, fp]);
                                        builder.ins().jump(after_e, &[]);
                                        builder.switch_to_block(fin_blk);
                                        let th = builder.ins().f64const(1000.0);
                                        let sc = builder.ins().fmul(fe, th);
                                        let si = builder.ins().fcvt_to_sint(types::I64, sc);
                                        let dv = builder.ins().iconst(types::I64, 1000);
                                        let ip = builder.ins().sdiv(si, dv);
                                        let dr = builder.ins().srem(si, dv);
                                        let z3 = builder.ins().iconst(types::I64, 0);
                                        let neg = builder.ins().icmp(IntCC::SignedLessThan, dr, z3);
                                        let nd = builder.ins().ineg(dr);
                                        let dp = builder.ins().select(neg, nd, dr);
                                        builder.ins().call(printf_ref, &[fmt_float_int_ptr, ip]);
                                        builder
                                            .ins()
                                            .call(printf_ref, &[fmt_float_dec_elem_ptr, dp]);
                                        builder.ins().jump(after_e, &[]);
                                        builder.seal_block(after_e);
                                        builder.switch_to_block(after_e);
                                    }
                                }
                                let next_i = builder.ins().iadd_imm(ci2, 1);
                                builder.def_var(counter_var, next_i);
                                builder.ins().jump(header, &[]);
                                builder.seal_block(header);
                                builder.switch_to_block(after);
                                let close_ptr = if is_last { fmt_rb_ptr } else { fmt_rb_sp_ptr };
                                builder
                                    .ins()
                                    .call(printf_ref, &[fmt_str_raw_ptr, close_ptr]);
                            }
                            StackType::Float => {
                                let f_val = builder.ins().bitcast(types::F64, MemFlags::new(), val);
                                let is_nan = builder.ins().fcmp(FloatCC::NotEqual, f_val, f_val);
                                let pos_inf = builder.ins().f64const(f64::INFINITY);
                                let neg_inf = builder.ins().f64const(f64::NEG_INFINITY);
                                let is_pos_inf = builder.ins().fcmp(FloatCC::Equal, f_val, pos_inf);
                                let is_neg_inf = builder.ins().fcmp(FloatCC::Equal, f_val, neg_inf);
                                let is_inf = builder.ins().bor(is_pos_inf, is_neg_inf);
                                let is_special = builder.ins().bor(is_nan, is_inf);

                                let special_block = builder.create_block();
                                let finite_block = builder.create_block();
                                let after_block = builder.create_block();
                                builder.ins().brif(
                                    is_special,
                                    special_block,
                                    &[],
                                    finite_block,
                                    &[],
                                );

                                builder.seal_block(special_block);
                                builder.seal_block(finite_block);

                                builder.switch_to_block(special_block);
                                let inf_ptr =
                                    builder
                                        .ins()
                                        .select(is_neg_inf, fmt_neg_inf_ptr, fmt_inf_ptr);
                                let final_ptr = builder.ins().select(is_nan, fmt_nan_ptr, inf_ptr);
                                let str_fmt = if is_last { fmt_str_ptr } else { fmt_str_sp_ptr };
                                builder.ins().call(printf_ref, &[str_fmt, final_ptr]);
                                builder.ins().jump(after_block, &[]);

                                builder.switch_to_block(finite_block);
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
                                let dec_fmt = if is_last {
                                    fmt_float_dec_ptr
                                } else {
                                    fmt_float_dec_sp_ptr
                                };
                                builder.ins().call(printf_ref, &[dec_fmt, dec_part]);
                                builder.ins().jump(after_block, &[]);
                                builder.seal_block(after_block);

                                builder.switch_to_block(after_block);
                            }
                        }
                    }
                    let sp_after_pop = builder.ins().iadd_imm(sp_start, -n_i64);
                    let sp_off_pn = builder.ins().imul_imm(sp_after_pop, 8);
                    let sp_addr_pn = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let dest_pn = builder.ins().iadd(sp_addr_pn, sp_off_pn);
                    let zero_pn = builder.ins().iconst(types::I64, 0);
                    builder.ins().store(MemFlags::new(), zero_pn, dest_pn, 0);
                    let sp_inc_pn = builder.ins().iadd_imm(sp_after_pop, 1);
                    builder.def_var(sp_var, sp_inc_pn);
                    type_stack.push(StackType::Int);
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
                    let globals_addr = builder.ins().global_value(types::I64, globals_gv);
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
                    let globals_addr = builder.ins().global_value(types::I64, globals_gv);
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
                    let idx_off = builder.ins().iconst(types::I64, (*idx as i64) * 8);
                    let idx_addr = builder.ins().iadd(locals_addr, idx_off);
                    let i = builder.ins().load(types::I64, MemFlags::new(), idx_addr, 0);
                    let iter_ty = local_types.get(iter).copied().unwrap_or(StackType::Int);
                    let (less, var_value) = match iter_ty {
                        StackType::List(_) => {
                            let len = builder.ins().load(types::I64, MemFlags::new(), handle, 8);
                            let in_range = builder.ins().icmp(IntCC::SignedLessThan, i, len);
                            let elem_idx = builder.ins().iadd_imm(i, 2);
                            let elem_off = builder.ins().imul_imm(elem_idx, 8);
                            let elem_addr = builder.ins().iadd(handle, elem_off);
                            let elem =
                                builder
                                    .ins()
                                    .load(types::I64, MemFlags::new(), elem_addr, 0);
                            (in_range, elem)
                        }
                        _ => {
                            let start = builder.ins().ushr_imm(handle, 32);
                            let end = builder.ins().band_imm(handle, 0xffffffff_u32 as i64);
                            let current = builder.ins().iadd(start, i);
                            let in_range = builder.ins().icmp(IntCC::SignedLessThan, current, end);
                            (in_range, current)
                        }
                    };
                    let var_off = builder.ins().iconst(types::I64, (*var as i64) * 8);
                    let var_addr = builder.ins().iadd(locals_addr, var_off);
                    builder.ins().store(MemFlags::new(), var_value, var_addr, 0);
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
                                Instruction::Div | Instruction::Mod => {
                                    let is_zero = builder.ins().icmp_imm(IntCC::Equal, b_bits, 0);
                                    let is_overflow = if matches!(ins, Instruction::Div) {
                                        let is_neg_one =
                                            builder.ins().icmp_imm(IntCC::Equal, b_bits, -1);
                                        let int_min_v = builder.ins().iconst(types::I64, i64::MIN);
                                        let is_int_min =
                                            builder.ins().icmp(IntCC::Equal, a_bits, int_min_v);
                                        builder.ins().band(is_neg_one, is_int_min)
                                    } else {
                                        is_zero
                                    };
                                    let is_bad = builder.ins().bor(is_zero, is_overflow);
                                    let err_block = builder.create_block();
                                    let ok_block = builder.create_block();
                                    builder.ins().brif(is_bad, err_block, &[], ok_block, &[]);

                                    builder.seal_block(err_block);
                                    builder.seal_block(ok_block);

                                    builder.switch_to_block(err_block);
                                    let err_ptr = builder.ins().select(
                                        is_zero,
                                        fmt_div0_msg_ptr,
                                        fmt_ovf_msg_ptr,
                                    );
                                    if try_depth > 0 {
                                        let base =
                                            builder.ins().stack_addr(types::I64, try_state_slot, 0);
                                        let off = builder
                                            .ins()
                                            .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                                        let saved_src = builder.ins().iadd(base, off);
                                        let saved_sp = builder.ins().load(
                                            types::I64,
                                            MemFlags::new(),
                                            saved_src,
                                            0,
                                        );
                                        let sp2 =
                                            builder.ins().stack_addr(types::I64, stack_slot, 0);
                                        let m_off = builder.ins().imul_imm(saved_sp, 8);
                                        let m_dest = builder.ins().iadd(sp2, m_off);
                                        builder.ins().store(MemFlags::new(), err_ptr, m_dest, 0);
                                        let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                                        builder.def_var(sp_var, new_sp_v);
                                        let handler = *handler_stack.last().unwrap();
                                        if !pending_seals.contains(&handler) {
                                            pending_seals.push(handler);
                                        }
                                        builder.ins().jump(handler, &[]);
                                    } else {
                                        let err_ptr_p = builder.ins().select(
                                            is_zero,
                                            fmt_div0_ptr,
                                            fmt_ovf_ptr,
                                        );
                                        builder.ins().call(printf_ref, &[fmt_str_ptr, err_ptr_p]);
                                        let exit_arg = builder.ins().iconst(types::I32, 1);
                                        builder.ins().call(exit_ref, &[exit_arg]);
                                        if is_main {
                                            let ret = builder.ins().iconst(types::I32, 1);
                                            builder.ins().return_(&[ret]);
                                        } else {
                                            let ret = builder.ins().iconst(types::I64, 0);
                                            builder.ins().return_(&[ret]);
                                        }
                                    }

                                    builder.switch_to_block(ok_block);
                                    match ins {
                                        Instruction::Div => builder.ins().sdiv(a_bits, b_bits),
                                        Instruction::Mod => builder.ins().srem(a_bits, b_bits),
                                        _ => unreachable!(),
                                    }
                                }
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
                    type_stack.push(StackType::Bool);
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
                    type_stack.push(func_ret_types[*id]);
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
                Instruction::Index => {
                    let idx_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let obj_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let idx_pos = builder.ins().iadd_imm(sp, -1);
                    let idx_off = builder.ins().imul_imm(idx_pos, 8);
                    let idx_addr = builder.ins().iadd(stack_addr, idx_off);
                    let idx = builder.ins().load(types::I64, MemFlags::new(), idx_addr, 0);
                    let obj_pos = builder.ins().iadd_imm(sp, -2);
                    let obj_off = builder.ins().imul_imm(obj_pos, 8);
                    let obj_addr = builder.ins().iadd(stack_addr, obj_off);
                    let obj = builder.ins().load(types::I64, MemFlags::new(), obj_addr, 0);
                    if !matches!(idx_ty, StackType::Int) {
                        return Err("list index must be Int in native".to_string());
                    }
                    match obj_ty {
                        StackType::List(elem_ty) => {
                            let zero = builder.ins().iconst(types::I64, 0);
                            let len = builder.ins().load(types::I64, MemFlags::new(), obj, 8);
                            let is_neg = builder.ins().icmp(IntCC::SignedLessThan, idx, zero);
                            let is_oob_hi =
                                builder
                                    .ins()
                                    .icmp(IntCC::SignedGreaterThanOrEqual, idx, len);
                            let is_oob = builder.ins().bor(is_neg, is_oob_hi);
                            let err_block = builder.create_block();
                            let ok_block = builder.create_block();
                            builder.ins().brif(is_oob, err_block, &[], ok_block, &[]);
                            builder.seal_block(err_block);
                            builder.seal_block(ok_block);
                            builder.switch_to_block(err_block);
                            if try_depth > 0 {
                                let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                                let off = builder
                                    .ins()
                                    .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                                let saved_src = builder.ins().iadd(base, off);
                                let saved_sp =
                                    builder
                                        .ins()
                                        .load(types::I64, MemFlags::new(), saved_src, 0);
                                let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                                let m_off = builder.ins().imul_imm(saved_sp, 8);
                                let m_dest = builder.ins().iadd(sp2, m_off);
                                builder
                                    .ins()
                                    .store(MemFlags::new(), fmt_oob_msg_ptr, m_dest, 0);
                                let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                                builder.def_var(sp_var, new_sp_v);
                                let handler = *handler_stack.last().unwrap();
                                if !pending_seals.contains(&handler) {
                                    pending_seals.push(handler);
                                }
                                builder.ins().jump(handler, &[]);
                            } else {
                                builder.ins().call(printf_ref, &[fmt_str_ptr, fmt_oob_ptr]);
                                let exit_arg = builder.ins().iconst(types::I32, 1);
                                builder.ins().call(exit_ref, &[exit_arg]);
                                if is_main {
                                    let ret = builder.ins().iconst(types::I32, 1);
                                    builder.ins().return_(&[ret]);
                                } else {
                                    let ret = builder.ins().iconst(types::I64, 0);
                                    builder.ins().return_(&[ret]);
                                }
                            }
                            builder.switch_to_block(ok_block);
                            let elem_idx = builder.ins().iadd_imm(idx, 2);
                            let elem_off = builder.ins().imul_imm(elem_idx, 8);
                            let elem_addr = builder.ins().iadd(obj, elem_off);
                            let elem =
                                builder
                                    .ins()
                                    .load(types::I64, MemFlags::new(), elem_addr, 0);
                            builder.ins().store(MemFlags::new(), elem, obj_addr, 0);
                            let sp_after = builder.ins().iadd_imm(sp, -1);
                            builder.def_var(sp_var, sp_after);
                            type_stack.push(match elem_ty {
                                ElemType::Int => StackType::Int,
                                ElemType::Float => StackType::Float,
                                ElemType::Str => StackType::Str,
                                ElemType::Bool => StackType::Bool,
                            });
                        }
                        _ => return Err("index on non-list type in native".to_string()),
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::IndexAt(i) => {
                    let obj_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let elem_ty: Option<ElemType> = match obj_ty {
                        StackType::List(e) => Some(e),
                        StackType::Tuple(id) => {
                            if *i < 0 {
                                None
                            } else {
                                let reg = tuple_types_registry.borrow();
                                reg.get(id as usize)
                                    .and_then(|types| types.get(*i as usize).copied())
                            }
                        }
                        _ => None,
                    };
                    let elem_ty =
                        elem_ty.ok_or_else(|| "IndexAt on non-list/tuple in native".to_string())?;
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let obj_pos = builder.ins().iadd_imm(sp, -1);
                    let obj_off = builder.ins().imul_imm(obj_pos, 8);
                    let obj_addr = builder.ins().iadd(stack_addr, obj_off);
                    let obj = builder.ins().load(types::I64, MemFlags::new(), obj_addr, 0);
                    let i_v = builder.ins().iconst(types::I64, *i);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let len = builder.ins().load(types::I64, MemFlags::new(), obj, 8);
                    let is_neg = builder.ins().icmp(IntCC::SignedLessThan, i_v, zero);
                    let is_oob_hi = builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, i_v, len);
                    let is_oob = builder.ins().bor(is_neg, is_oob_hi);
                    let err_block = builder.create_block();
                    let ok_block = builder.create_block();
                    builder.ins().brif(is_oob, err_block, &[], ok_block, &[]);
                    builder.seal_block(err_block);
                    builder.seal_block(ok_block);
                    builder.switch_to_block(err_block);
                    if try_depth > 0 {
                        let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                        let off = builder
                            .ins()
                            .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                        let saved_src = builder.ins().iadd(base, off);
                        let saved_sp =
                            builder
                                .ins()
                                .load(types::I64, MemFlags::new(), saved_src, 0);
                        let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let m_off = builder.ins().imul_imm(saved_sp, 8);
                        let m_dest = builder.ins().iadd(sp2, m_off);
                        builder
                            .ins()
                            .store(MemFlags::new(), fmt_oob_msg_ptr, m_dest, 0);
                        let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                        builder.def_var(sp_var, new_sp_v);
                        let handler = *handler_stack.last().unwrap();
                        if !pending_seals.contains(&handler) {
                            pending_seals.push(handler);
                        }
                        builder.ins().jump(handler, &[]);
                    } else {
                        builder.ins().call(printf_ref, &[fmt_str_ptr, fmt_oob_ptr]);
                        let exit_arg = builder.ins().iconst(types::I32, 1);
                        builder.ins().call(exit_ref, &[exit_arg]);
                        if is_main {
                            let ret = builder.ins().iconst(types::I32, 1);
                            builder.ins().return_(&[ret]);
                        } else {
                            let ret = builder.ins().iconst(types::I64, 0);
                            builder.ins().return_(&[ret]);
                        }
                    }
                    builder.switch_to_block(ok_block);
                    let elem_idx = builder.ins().iadd_imm(i_v, 2);
                    let elem_off = builder.ins().imul_imm(elem_idx, 8);
                    let elem_addr = builder.ins().iadd(obj, elem_off);
                    let elem = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), elem_addr, 0);
                    builder.ins().store(MemFlags::new(), elem, obj_addr, 0);
                    type_stack.push(match elem_ty {
                        ElemType::Int => StackType::Int,
                        ElemType::Float => StackType::Float,
                        ElemType::Str => StackType::Str,
                        ElemType::Bool => StackType::Bool,
                    });
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::StoreIndex => {
                    let idx_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let obj_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let _val_ty = type_stack.pop().unwrap_or(StackType::Int);
                    if !matches!(idx_ty, StackType::Int) {
                        return Err("list index must be Int in native".to_string());
                    }
                    let elem_ty = match obj_ty {
                        StackType::List(e) => e,
                        _ => return Err("StoreIndex on non-list type in native".to_string()),
                    };
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let val_pos = builder.ins().iadd_imm(sp, -3);
                    let val_off = builder.ins().imul_imm(val_pos, 8);
                    let val_addr = builder.ins().iadd(stack_addr, val_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
                    let obj_pos = builder.ins().iadd_imm(sp, -2);
                    let obj_off = builder.ins().imul_imm(obj_pos, 8);
                    let obj_addr = builder.ins().iadd(stack_addr, obj_off);
                    let obj = builder.ins().load(types::I64, MemFlags::new(), obj_addr, 0);
                    let idx_pos = builder.ins().iadd_imm(sp, -1);
                    let idx_off = builder.ins().imul_imm(idx_pos, 8);
                    let idx_addr = builder.ins().iadd(stack_addr, idx_off);
                    let idx = builder.ins().load(types::I64, MemFlags::new(), idx_addr, 0);
                    let zero = builder.ins().iconst(types::I64, 0);
                    let len = builder.ins().load(types::I64, MemFlags::new(), obj, 8);
                    let is_neg = builder.ins().icmp(IntCC::SignedLessThan, idx, zero);
                    let is_oob_hi = builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, idx, len);
                    let is_oob = builder.ins().bor(is_neg, is_oob_hi);
                    let err_block = builder.create_block();
                    let ok_block = builder.create_block();
                    builder.ins().brif(is_oob, err_block, &[], ok_block, &[]);
                    builder.seal_block(err_block);
                    builder.seal_block(ok_block);
                    builder.switch_to_block(err_block);
                    if try_depth > 0 {
                        let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                        let off = builder
                            .ins()
                            .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                        let saved_src = builder.ins().iadd(base, off);
                        let saved_sp =
                            builder
                                .ins()
                                .load(types::I64, MemFlags::new(), saved_src, 0);
                        let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let m_off = builder.ins().imul_imm(saved_sp, 8);
                        let m_dest = builder.ins().iadd(sp2, m_off);
                        builder
                            .ins()
                            .store(MemFlags::new(), fmt_oob_msg_ptr, m_dest, 0);
                        let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                        builder.def_var(sp_var, new_sp_v);
                        let handler = *handler_stack.last().unwrap();
                        if !pending_seals.contains(&handler) {
                            pending_seals.push(handler);
                        }
                        builder.ins().jump(handler, &[]);
                    } else {
                        builder.ins().call(printf_ref, &[fmt_str_ptr, fmt_oob_ptr]);
                        let exit_arg = builder.ins().iconst(types::I32, 1);
                        builder.ins().call(exit_ref, &[exit_arg]);
                        if is_main {
                            let ret = builder.ins().iconst(types::I32, 1);
                            builder.ins().return_(&[ret]);
                        } else {
                            let ret = builder.ins().iconst(types::I64, 0);
                            builder.ins().return_(&[ret]);
                        }
                    }
                    builder.switch_to_block(ok_block);
                    let elem_idx = builder.ins().iadd_imm(idx, 2);
                    let elem_off = builder.ins().imul_imm(elem_idx, 8);
                    let elem_addr = builder.ins().iadd(obj, elem_off);
                    builder.ins().store(MemFlags::new(), val, elem_addr, 0);
                    builder.ins().store(MemFlags::new(), obj, val_addr, 0);
                    let sp_after = builder.ins().iadd_imm(sp, -2);
                    builder.def_var(sp_var, sp_after);
                    type_stack.push(StackType::List(elem_ty));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::ListPush => {
                    let list_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let _val_ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let val_pos = builder.ins().iadd_imm(sp, -2);
                    let val_off = builder.ins().imul_imm(val_pos, 8);
                    let val_addr = builder.ins().iadd(stack_addr, val_off);
                    let val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
                    let list_pos = builder.ins().iadd_imm(sp, -1);
                    let list_off = builder.ins().imul_imm(list_pos, 8);
                    let list_addr = builder.ins().iadd(stack_addr, list_off);
                    let list_ptr = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), list_addr, 0);
                    let elem_ty = match list_ty {
                        StackType::List(e) => e,
                        _ => return Err("push() needs a List".to_string()),
                    };
                    let old_len = builder.ins().load(types::I64, MemFlags::new(), list_ptr, 8);
                    let new_len = builder.ins().iadd_imm(old_len, 1);
                    let trip_old = builder.ins().imul_imm(old_len, 3);
                    let size_elems = builder.ins().iadd_imm(trip_old, 5);
                    let size_bytes = builder.ins().imul_imm(size_elems, 8);
                    let malloc_call = builder.ins().call(malloc_ref, &[size_bytes]);
                    let new_ptr = builder.inst_results(malloc_call)[0];
                    let old_size_elems = builder.ins().iadd_imm(trip_old, 2);
                    let old_size_bytes = builder.ins().imul_imm(old_size_elems, 8);
                    builder
                        .ins()
                        .call(memcpy_ref, &[new_ptr, list_ptr, old_size_bytes]);
                    let one = builder.ins().iconst(types::I64, 1);
                    builder.ins().store(MemFlags::new(), one, new_ptr, 0);
                    builder.ins().store(MemFlags::new(), new_len, new_ptr, 8);
                    let ne_plus1 = builder.ins().iadd_imm(new_len, 1);
                    let new_elem_off = builder.ins().imul_imm(ne_plus1, 8);
                    let new_elem_addr = builder.ins().iadd(new_ptr, new_elem_off);
                    builder.ins().store(MemFlags::new(), val, new_elem_addr, 0);
                    builder.ins().store(MemFlags::new(), new_ptr, val_addr, 0);
                    let sp_after = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, sp_after);
                    type_stack.push(StackType::List(elem_ty));
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Neg => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    match ty {
                        StackType::Int => {
                            let int_min_v = builder.ins().iconst(types::I64, i64::MIN);
                            let is_int_min = builder.ins().icmp(IntCC::Equal, val, int_min_v);
                            let err_block = builder.create_block();
                            let ok_block = builder.create_block();
                            builder
                                .ins()
                                .brif(is_int_min, err_block, &[], ok_block, &[]);
                            builder.seal_block(err_block);
                            builder.seal_block(ok_block);
                            builder.switch_to_block(err_block);
                            if try_depth > 0 {
                                let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                                let off = builder
                                    .ins()
                                    .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                                let saved_src = builder.ins().iadd(base, off);
                                let saved_sp =
                                    builder
                                        .ins()
                                        .load(types::I64, MemFlags::new(), saved_src, 0);
                                let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                                let m_off = builder.ins().imul_imm(saved_sp, 8);
                                let m_dest = builder.ins().iadd(sp2, m_off);
                                builder.ins().store(MemFlags::new(), fmt_ovf_ptr, m_dest, 0);
                                let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                                builder.def_var(sp_var, new_sp_v);
                                let handler = *handler_stack.last().unwrap();
                                if !pending_seals.contains(&handler) {
                                    pending_seals.push(handler);
                                }
                                builder.ins().jump(handler, &[]);
                            } else {
                                builder.ins().call(printf_ref, &[fmt_str_ptr, fmt_ovf_ptr]);
                                let exit_arg = builder.ins().iconst(types::I32, 1);
                                builder.ins().call(exit_ref, &[exit_arg]);
                                if is_main {
                                    let ret = builder.ins().iconst(types::I32, 1);
                                    builder.ins().return_(&[ret]);
                                } else {
                                    let ret = builder.ins().iconst(types::I64, 0);
                                    builder.ins().return_(&[ret]);
                                }
                            }
                            builder.switch_to_block(ok_block);
                            let r = builder.ins().ineg(val);
                            builder.ins().store(MemFlags::new(), r, src, 0);
                            type_stack.push(StackType::Int);
                        }
                        StackType::Float => {
                            let f_v = builder.ins().bitcast(types::F64, MemFlags::new(), val);
                            let fneg = builder.ins().fneg(f_v);
                            let r = builder.ins().bitcast(types::I64, MemFlags::new(), fneg);
                            builder.ins().store(MemFlags::new(), r, src, 0);
                            type_stack.push(StackType::Float);
                        }
                        _ => return Err("neg on non-numeric".to_string()),
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Not => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    match ty {
                        StackType::Bool => {
                            let one = builder.ins().iconst(types::I64, 1);
                            let r = builder.ins().bxor(val, one);
                            builder.ins().store(MemFlags::new(), r, src, 0);
                            type_stack.push(StackType::Bool);
                        }
                        _ => return Err("not on non-bool".to_string()),
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Len => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    match ty {
                        StackType::List(_) => {
                            let len = builder.ins().load(types::I64, MemFlags::new(), val, 8);
                            builder.ins().store(MemFlags::new(), len, src, 0);
                            type_stack.push(StackType::Int);
                        }
                        StackType::Tuple(_) => {
                            let len = builder.ins().load(types::I64, MemFlags::new(), val, 8);
                            builder.ins().store(MemFlags::new(), len, src, 0);
                            type_stack.push(StackType::Int);
                        }
                        StackType::Str => {
                            let len_call = builder.ins().call(strlen_ref, &[val]);
                            let len = builder.inst_results(len_call)[0];
                            builder.ins().store(MemFlags::new(), len, src, 0);
                            type_stack.push(StackType::Int);
                        }
                        _ => {
                            return Err("len() not supported for this type in native".to_string());
                        }
                    }
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MathAbs => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let r = match ty {
                        StackType::Float => {
                            let mask = builder
                                .ins()
                                .iconst(types::I64, 0x7FFF_FFFF_FFFF_FFFF_u64 as i64);
                            builder.ins().band(val, mask)
                        }
                        StackType::Int => {
                            let zero = builder.ins().iconst(types::I64, 0);
                            let neg = builder.ins().icmp(IntCC::SignedLessThan, val, zero);
                            let neg_val = builder.ins().ineg(val);
                            builder.ins().select(neg, neg_val, val)
                        }
                        _ => return Err("abs() needs a number".to_string()),
                    };
                    builder.ins().store(MemFlags::new(), r, src, 0);
                    type_stack.push(ty);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MathPow => {
                    let ty_b = type_stack.pop().unwrap_or(StackType::Int);
                    let ty_a = type_stack.pop().unwrap_or(StackType::Int);
                    let both_int = matches!(ty_a, StackType::Int) && matches!(ty_b, StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let new_sp = builder.ins().iadd_imm(sp, -1);
                    builder.def_var(sp_var, new_sp);
                    let stack_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let b_off = builder.ins().imul_imm(new_sp, 8);
                    let b_addr = builder.ins().iadd(stack_addr, b_off);
                    let b_bits = builder.ins().load(types::I64, MemFlags::new(), b_addr, 0);
                    let a_addr = builder.ins().iadd_imm(b_addr, -8);
                    let a_bits = builder.ins().load(types::I64, MemFlags::new(), a_addr, 0);
                    let b_f = if matches!(ty_b, StackType::Int) {
                        builder.ins().fcvt_from_sint(types::F64, b_bits)
                    } else {
                        builder.ins().bitcast(types::F64, MemFlags::new(), b_bits)
                    };
                    let a_f = if matches!(ty_a, StackType::Int) {
                        builder.ins().fcvt_from_sint(types::F64, a_bits)
                    } else {
                        builder.ins().bitcast(types::F64, MemFlags::new(), a_bits)
                    };
                    let call = builder.ins().call(pow_ref, &[a_f, b_f]);
                    let r_f = builder.inst_results(call)[0];
                    let r_bits = if both_int {
                        builder.ins().fcvt_to_sint(types::I64, r_f)
                    } else {
                        builder.ins().bitcast(types::I64, MemFlags::new(), r_f)
                    };
                    builder.ins().store(MemFlags::new(), r_bits, a_addr, 0);
                    type_stack.push(if both_int {
                        StackType::Int
                    } else {
                        StackType::Float
                    });
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::MathFloor | Instruction::MathCeil | Instruction::MathRound => {
                    let ty = type_stack.pop().unwrap_or(StackType::Int);
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let val = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    let f_val = match ty {
                        StackType::Int => builder.ins().fcvt_from_sint(types::F64, val),
                        StackType::Float => builder.ins().bitcast(types::F64, MemFlags::new(), val),
                        _ => {
                            return Err("math function needs a number".to_string());
                        }
                    };
                    let call = match ins {
                        Instruction::MathFloor => builder.ins().call(floor_ref, &[f_val]),
                        Instruction::MathCeil => builder.ins().call(ceil_ref, &[f_val]),
                        _ => builder.ins().call(round_ref, &[f_val]),
                    };
                    let r_f = builder.inst_results(call)[0];
                    let r_bits = builder.ins().bitcast(types::I64, MemFlags::new(), r_f);
                    builder.ins().store(MemFlags::new(), r_bits, src, 0);
                    type_stack.push(StackType::Float);
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::PushHandler(target) => {
                    let sp = builder.use_var(sp_var);
                    let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                    let off = builder.ins().iconst(types::I64, (try_depth * 8) as i64);
                    let dest = builder.ins().iadd(base, off);
                    builder.ins().store(MemFlags::new(), sp, dest, 0);
                    handler_stack.push(blocks[*target]);
                    try_depth += 1;
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::PopHandler => {
                    try_depth = try_depth.saturating_sub(1);
                    handler_stack.pop();
                    builder.ins().jump(blocks[ip + 1], &[]);
                }
                Instruction::Throw => {
                    let sp = builder.use_var(sp_var);
                    let sp_off = builder.ins().imul_imm(sp, 8);
                    let sp_addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                    let top = builder.ins().iadd(sp_addr, sp_off);
                    let src = builder.ins().iadd_imm(top, -8);
                    let msg_ptr = builder.ins().load(types::I64, MemFlags::new(), src, 0);
                    type_stack.pop();
                    if try_depth > 0 {
                        let base = builder.ins().stack_addr(types::I64, try_state_slot, 0);
                        let off = builder
                            .ins()
                            .iconst(types::I64, ((try_depth - 1) * 8) as i64);
                        let saved_src = builder.ins().iadd(base, off);
                        let saved_sp =
                            builder
                                .ins()
                                .load(types::I64, MemFlags::new(), saved_src, 0);
                        let sp2 = builder.ins().stack_addr(types::I64, stack_slot, 0);
                        let m_off = builder.ins().imul_imm(saved_sp, 8);
                        let m_dest = builder.ins().iadd(sp2, m_off);
                        builder.ins().store(MemFlags::new(), msg_ptr, m_dest, 0);
                        let new_sp_v = builder.ins().iadd_imm(saved_sp, 1);
                        builder.def_var(sp_var, new_sp_v);
                        type_stack.push(StackType::Str);
                        let handler = *handler_stack.last().unwrap();
                        if !pending_seals.contains(&handler) {
                            pending_seals.push(handler);
                        }
                        builder.ins().jump(handler, &[]);
                    } else {
                        let new_sp = builder.ins().iadd_imm(sp, -1);
                        builder.def_var(sp_var, new_sp);
                        builder.ins().call(printf_ref, &[fmt_str_ptr, msg_ptr]);
                        let exit_arg = builder.ins().iconst(types::I32, 1);
                        builder.ins().call(exit_ref, &[exit_arg]);
                        if is_main {
                            let ret = builder.ins().iconst(types::I32, 1);
                            builder.ins().return_(&[ret]);
                        } else {
                            let ret = builder.ins().iconst(types::I64, 0);
                            builder.ins().return_(&[ret]);
                        }
                    }
                }
                other => {
                    return Err(format!(
                        "native codegen: {:?} not yet implemented (use `rca run` for VM)",
                        other
                    ));
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
        let globals_gv = module.declare_data_in_func(globals_id, builder.func);
        let sp_var = Variable::new(0);
        builder.declare_var(sp_var, types::I64);
        builder.declare_var(Variable::new(1), types::I64);
        builder.declare_var(Variable::new(2), types::I64);
        builder.declare_var(Variable::new(3), types::I64);
        builder.declare_var(Variable::new(4), types::I8);
        builder.declare_var(Variable::new(5), types::I64);
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

        let fmt_nan_gv = module.declare_data_in_func(str_nan_id, builder.func);
        let fmt_nan_ptr = builder.ins().global_value(types::I64, fmt_nan_gv);
        let fmt_inf_gv = module.declare_data_in_func(str_inf_id, builder.func);
        let fmt_inf_ptr = builder.ins().global_value(types::I64, fmt_inf_gv);
        let fmt_neg_inf_gv = module.declare_data_in_func(str_neg_inf_id, builder.func);
        let fmt_neg_inf_ptr = builder.ins().global_value(types::I64, fmt_neg_inf_gv);
        let fmt_true_gv = module.declare_data_in_func(str_true_id, builder.func);
        let fmt_true_ptr = builder.ins().global_value(types::I64, fmt_true_gv);
        let fmt_false_gv = module.declare_data_in_func(str_false_id, builder.func);
        let fmt_false_ptr = builder.ins().global_value(types::I64, fmt_false_gv);
        let fmt_int_sp_gv = module.declare_data_in_func(fmt_int_sp_id, builder.func);
        let fmt_int_sp_ptr = builder.ins().global_value(types::I64, fmt_int_sp_gv);
        let fmt_str_sp_gv = module.declare_data_in_func(fmt_str_sp_id, builder.func);
        let fmt_str_sp_ptr = builder.ins().global_value(types::I64, fmt_str_sp_gv);
        let fmt_float_dec_sp_gv = module.declare_data_in_func(fmt_float_dec_sp_id, builder.func);
        let fmt_float_dec_sp_ptr = builder.ins().global_value(types::I64, fmt_float_dec_sp_gv);

        let printf_ref = module.declare_func_in_func(printf_func, builder.func);
        let strlen_ref = module.declare_func_in_func(strlen_func, builder.func);
        let malloc_ref = module.declare_func_in_func(malloc_func, builder.func);
        let memcpy_ref = module.declare_func_in_func(memcpy_func, builder.func);
        let strcmp_ref = module.declare_func_in_func(strcmp_func, builder.func);
        let exit_ref = module.declare_func_in_func(exit_func, builder.func);
        let pow_ref = module.declare_func_in_func(pow_func, builder.func);
        let floor_ref = module.declare_func_in_func(floor_func, builder.func);
        let ceil_ref = module.declare_func_in_func(ceil_func, builder.func);
        let round_ref = module.declare_func_in_func(round_func, builder.func);
        let free_ref = module.declare_func_in_func(free_func, builder.func);
        let free_ref = module.declare_func_in_func(free_func, builder.func);
        let fmt_div0_gv = module.declare_data_in_func(str_div0_id, builder.func);
        let fmt_div0_ptr = builder.ins().global_value(types::I64, fmt_div0_gv);
        let fmt_ovf_gv = module.declare_data_in_func(str_ovf_id, builder.func);
        let fmt_ovf_ptr = builder.ins().global_value(types::I64, fmt_ovf_gv);
        let fmt_str_raw_gv = module.declare_data_in_func(fmt_str_raw_id, builder.func);
        let fmt_str_raw_ptr = builder.ins().global_value(types::I64, fmt_str_raw_gv);
        let fmt_lb_gv = module.declare_data_in_func(fmt_lb_id, builder.func);
        let fmt_lb_ptr = builder.ins().global_value(types::I64, fmt_lb_gv);
        let fmt_rb_gv = module.declare_data_in_func(fmt_rb_id, builder.func);
        let fmt_rb_ptr = builder.ins().global_value(types::I64, fmt_rb_gv);
        let fmt_rb_sp_gv = module.declare_data_in_func(fmt_rb_sp_id, builder.func);
        let fmt_rb_sp_ptr = builder.ins().global_value(types::I64, fmt_rb_sp_gv);
        let fmt_comma_gv = module.declare_data_in_func(fmt_comma_id, builder.func);
        let fmt_comma_ptr = builder.ins().global_value(types::I64, fmt_comma_gv);
        let fmt_empty_gv = module.declare_data_in_func(fmt_empty_id, builder.func);
        let fmt_empty_ptr = builder.ins().global_value(types::I64, fmt_empty_gv);
        let fmt_int_elem_gv = module.declare_data_in_func(fmt_int_elem_id, builder.func);
        let fmt_int_elem_ptr = builder.ins().global_value(types::I64, fmt_int_elem_gv);
        let fmt_str_elem_gv = module.declare_data_in_func(fmt_str_elem_id, builder.func);
        let fmt_str_elem_ptr = builder.ins().global_value(types::I64, fmt_str_elem_gv);
        let fmt_float_dec_elem_gv =
            module.declare_data_in_func(fmt_float_dec_elem_id, builder.func);
        let fmt_float_dec_elem_ptr = builder
            .ins()
            .global_value(types::I64, fmt_float_dec_elem_gv);
        let fmt_oob_gv = module.declare_data_in_func(fmt_oob_id, builder.func);
        let fmt_oob_ptr = builder.ins().global_value(types::I64, fmt_oob_gv);
        let fmt_lparen_gv = module.declare_data_in_func(fmt_lparen_id, builder.func);
        let fmt_lparen_ptr = builder.ins().global_value(types::I64, fmt_lparen_gv);
        let fmt_rparen_gv = module.declare_data_in_func(fmt_rparen_id, builder.func);
        let fmt_rparen_ptr = builder.ins().global_value(types::I64, fmt_rparen_gv);
        let fmt_rparen_sp_gv = module.declare_data_in_func(fmt_rparen_sp_id, builder.func);
        let fmt_rparen_sp_ptr = builder.ins().global_value(types::I64, fmt_rparen_sp_gv);
        let fmt_map_open_gv = module.declare_data_in_func(fmt_map_open_id, builder.func);
        let fmt_map_open_ptr = builder.ins().global_value(types::I64, fmt_map_open_gv);
        let fmt_map_err_gv = module.declare_data_in_func(fmt_map_err_id, builder.func);
        let fmt_map_err_ptr = builder.ins().global_value(types::I64, fmt_map_err_gv);
        let fmt_div0_msg_gv = module.declare_data_in_func(fmt_div0_msg_id, builder.func);
        let fmt_div0_msg_ptr = builder.ins().global_value(types::I64, fmt_div0_msg_gv);
        let fmt_ovf_msg_gv = module.declare_data_in_func(fmt_ovf_msg_id, builder.func);
        let fmt_ovf_msg_ptr = builder.ins().global_value(types::I64, fmt_ovf_msg_gv);
        let fmt_oob_msg_gv = module.declare_data_in_func(fmt_oob_msg_id, builder.func);
        let fmt_oob_msg_ptr = builder.ins().global_value(types::I64, fmt_oob_msg_gv);
        let fmt_map_err_msg_gv = module.declare_data_in_func(fmt_map_err_msg_id, builder.func);
        let fmt_map_err_msg_ptr = builder.ins().global_value(types::I64, fmt_map_err_msg_gv);

        compile_chunk(
            &mut module,
            &mut builder,
            &program.main.chunk,
            stack_slot,
            locals_slot,
            globals_gv,
            sp_var,
            printf_ref,
            strlen_ref,
            malloc_ref,
            memcpy_ref,
            strcmp_ref,
            exit_ref,
            pow_ref,
            floor_ref,
            ceil_ref,
            round_ref,
            free_ref,
            fmt_int_ptr,
            fmt_str_ptr,
            fmt_float_int_ptr,
            fmt_float_dec_ptr,
            fmt_nan_ptr,
            fmt_inf_ptr,
            fmt_neg_inf_ptr,
            fmt_true_ptr,
            fmt_false_ptr,
            fmt_int_sp_ptr,
            fmt_str_sp_ptr,
            fmt_float_dec_sp_ptr,
            fmt_div0_ptr,
            fmt_ovf_ptr,
            fmt_str_raw_ptr,
            fmt_lb_ptr,
            fmt_rb_ptr,
            fmt_rb_sp_ptr,
            fmt_comma_ptr,
            fmt_empty_ptr,
            fmt_int_elem_ptr,
            fmt_str_elem_ptr,
            fmt_float_dec_elem_ptr,
            fmt_oob_ptr,
            fmt_lparen_ptr,
            fmt_rparen_ptr,
            fmt_rparen_sp_ptr,
            fmt_map_open_ptr,
            fmt_map_err_ptr,
            fmt_div0_msg_ptr,
            fmt_ovf_msg_ptr,
            fmt_oob_msg_ptr,
            fmt_map_err_msg_ptr,
            &[],
            &program.structs,
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
            let globals_gv = module.declare_data_in_func(globals_id, builder.func);
            let sp_var = Variable::new(0);
            builder.declare_var(sp_var, types::I64);
            builder.declare_var(Variable::new(1), types::I64);
            builder.declare_var(Variable::new(2), types::I64);
            builder.declare_var(Variable::new(3), types::I64);
            builder.declare_var(Variable::new(4), types::I8);
            builder.declare_var(Variable::new(5), types::I64);
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

            let fmt_nan_gv = module.declare_data_in_func(str_nan_id, builder.func);
            let fmt_nan_ptr = builder.ins().global_value(types::I64, fmt_nan_gv);
            let fmt_inf_gv = module.declare_data_in_func(str_inf_id, builder.func);
            let fmt_inf_ptr = builder.ins().global_value(types::I64, fmt_inf_gv);
            let fmt_neg_inf_gv = module.declare_data_in_func(str_neg_inf_id, builder.func);
            let fmt_neg_inf_ptr = builder.ins().global_value(types::I64, fmt_neg_inf_gv);
            let fmt_true_gv = module.declare_data_in_func(str_true_id, builder.func);
            let fmt_true_ptr = builder.ins().global_value(types::I64, fmt_true_gv);
            let fmt_false_gv = module.declare_data_in_func(str_false_id, builder.func);
            let fmt_false_ptr = builder.ins().global_value(types::I64, fmt_false_gv);
            let fmt_int_sp_gv = module.declare_data_in_func(fmt_int_sp_id, builder.func);
            let fmt_int_sp_ptr = builder.ins().global_value(types::I64, fmt_int_sp_gv);
            let fmt_str_sp_gv = module.declare_data_in_func(fmt_str_sp_id, builder.func);
            let fmt_str_sp_ptr = builder.ins().global_value(types::I64, fmt_str_sp_gv);
            let fmt_float_dec_sp_gv =
                module.declare_data_in_func(fmt_float_dec_sp_id, builder.func);
            let fmt_float_dec_sp_ptr = builder.ins().global_value(types::I64, fmt_float_dec_sp_gv);

            let printf_ref = module.declare_func_in_func(printf_func, builder.func);
            let strlen_ref = module.declare_func_in_func(strlen_func, builder.func);
            let malloc_ref = module.declare_func_in_func(malloc_func, builder.func);
            let memcpy_ref = module.declare_func_in_func(memcpy_func, builder.func);
            let strcmp_ref = module.declare_func_in_func(strcmp_func, builder.func);
            let exit_ref = module.declare_func_in_func(exit_func, builder.func);
            let pow_ref = module.declare_func_in_func(pow_func, builder.func);
            let floor_ref = module.declare_func_in_func(floor_func, builder.func);
            let ceil_ref = module.declare_func_in_func(ceil_func, builder.func);
            let round_ref = module.declare_func_in_func(round_func, builder.func);
            let free_ref = module.declare_func_in_func(free_func, builder.func);
            let free_ref = module.declare_func_in_func(free_func, builder.func);
            let fmt_div0_gv = module.declare_data_in_func(str_div0_id, builder.func);
            let fmt_div0_ptr = builder.ins().global_value(types::I64, fmt_div0_gv);
            let fmt_ovf_gv = module.declare_data_in_func(str_ovf_id, builder.func);
            let fmt_ovf_ptr = builder.ins().global_value(types::I64, fmt_ovf_gv);
            let fmt_str_raw_gv = module.declare_data_in_func(fmt_str_raw_id, builder.func);
            let fmt_str_raw_ptr = builder.ins().global_value(types::I64, fmt_str_raw_gv);
            let fmt_lb_gv = module.declare_data_in_func(fmt_lb_id, builder.func);
            let fmt_lb_ptr = builder.ins().global_value(types::I64, fmt_lb_gv);
            let fmt_rb_gv = module.declare_data_in_func(fmt_rb_id, builder.func);
            let fmt_rb_ptr = builder.ins().global_value(types::I64, fmt_rb_gv);
            let fmt_rb_sp_gv = module.declare_data_in_func(fmt_rb_sp_id, builder.func);
            let fmt_rb_sp_ptr = builder.ins().global_value(types::I64, fmt_rb_sp_gv);
            let fmt_comma_gv = module.declare_data_in_func(fmt_comma_id, builder.func);
            let fmt_comma_ptr = builder.ins().global_value(types::I64, fmt_comma_gv);
            let fmt_empty_gv = module.declare_data_in_func(fmt_empty_id, builder.func);
            let fmt_empty_ptr = builder.ins().global_value(types::I64, fmt_empty_gv);
            let fmt_int_elem_gv = module.declare_data_in_func(fmt_int_elem_id, builder.func);
            let fmt_int_elem_ptr = builder.ins().global_value(types::I64, fmt_int_elem_gv);
            let fmt_str_elem_gv = module.declare_data_in_func(fmt_str_elem_id, builder.func);
            let fmt_str_elem_ptr = builder.ins().global_value(types::I64, fmt_str_elem_gv);
            let fmt_float_dec_elem_gv =
                module.declare_data_in_func(fmt_float_dec_elem_id, builder.func);
            let fmt_float_dec_elem_ptr = builder
                .ins()
                .global_value(types::I64, fmt_float_dec_elem_gv);
            let fmt_oob_gv = module.declare_data_in_func(fmt_oob_id, builder.func);
            let fmt_oob_ptr = builder.ins().global_value(types::I64, fmt_oob_gv);
            let fmt_lparen_gv = module.declare_data_in_func(fmt_lparen_id, builder.func);
            let fmt_lparen_ptr = builder.ins().global_value(types::I64, fmt_lparen_gv);
            let fmt_rparen_gv = module.declare_data_in_func(fmt_rparen_id, builder.func);
            let fmt_rparen_ptr = builder.ins().global_value(types::I64, fmt_rparen_gv);
            let fmt_rparen_sp_gv = module.declare_data_in_func(fmt_rparen_sp_id, builder.func);
            let fmt_rparen_sp_ptr = builder.ins().global_value(types::I64, fmt_rparen_sp_gv);
            let fmt_map_open_gv = module.declare_data_in_func(fmt_map_open_id, builder.func);
            let fmt_map_open_ptr = builder.ins().global_value(types::I64, fmt_map_open_gv);
            let fmt_map_err_gv = module.declare_data_in_func(fmt_map_err_id, builder.func);
            let fmt_map_err_ptr = builder.ins().global_value(types::I64, fmt_map_err_gv);
            let fmt_div0_msg_gv = module.declare_data_in_func(fmt_div0_msg_id, builder.func);
            let fmt_div0_msg_ptr = builder.ins().global_value(types::I64, fmt_div0_msg_gv);
            let fmt_ovf_msg_gv = module.declare_data_in_func(fmt_ovf_msg_id, builder.func);
            let fmt_ovf_msg_ptr = builder.ins().global_value(types::I64, fmt_ovf_msg_gv);
            let fmt_oob_msg_gv = module.declare_data_in_func(fmt_oob_msg_id, builder.func);
            let fmt_oob_msg_ptr = builder.ins().global_value(types::I64, fmt_oob_msg_gv);
            let fmt_map_err_msg_gv = module.declare_data_in_func(fmt_map_err_msg_id, builder.func);
            let fmt_map_err_msg_ptr = builder.ins().global_value(types::I64, fmt_map_err_msg_gv);

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
                globals_gv,
                sp_var,
                printf_ref,
                strlen_ref,
                malloc_ref,
                memcpy_ref,
                strcmp_ref,
                exit_ref,
                pow_ref,
                floor_ref,
                ceil_ref,
                round_ref,
                free_ref,
                fmt_int_ptr,
                fmt_str_ptr,
                fmt_float_int_ptr,
                fmt_float_dec_ptr,
                fmt_nan_ptr,
                fmt_inf_ptr,
                fmt_neg_inf_ptr,
                fmt_true_ptr,
                fmt_false_ptr,
                fmt_int_sp_ptr,
                fmt_str_sp_ptr,
                fmt_float_dec_sp_ptr,
                fmt_div0_ptr,
                fmt_ovf_ptr,
                fmt_str_raw_ptr,
                fmt_lb_ptr,
                fmt_rb_ptr,
                fmt_rb_sp_ptr,
                fmt_comma_ptr,
                fmt_empty_ptr,
                fmt_int_elem_ptr,
                fmt_str_elem_ptr,
                fmt_float_dec_elem_ptr,
                fmt_oob_ptr,
                fmt_lparen_ptr,
                fmt_rparen_ptr,
                fmt_rparen_sp_ptr,
                fmt_map_open_ptr,
                fmt_map_err_ptr,
                fmt_div0_msg_ptr,
                fmt_ovf_msg_ptr,
                fmt_oob_msg_ptr,
                fmt_map_err_msg_ptr,
                &f.param_types,
                &program.structs,
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
