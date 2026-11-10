//! Cranelift backend for generating native machine code.
//! See blueprint §5.2: native compilation must ultimately be supported.
//!
//! Current status: generates a valid object file with a dummy `main()`
//! that returns 0. Full IR-to-Cranelift translation is the next milestone.
use cranelift::prelude::*;
use cranelift_codegen::ir::{AbiParam, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::compiler::ir::IrProgram;

pub fn compile(_program: &IrProgram) -> Result<Vec<u8>, String> {
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

    // Signature: fn main() -> i32
    let mut sig = module.make_signature();
    sig.returns.push(AbiParam::new(types::I32));

    // Create context and builder
    let mut ctx = module.make_context();
    ctx.func.signature = sig.clone();
    let mut builder_ctx = FunctionBuilderContext::new();

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Dummy: return 0
        let zero = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.finalize();
    }

    let main_id = module
        .declare_function("main", Linkage::Export, &sig)
        .map_err(|e| e.to_string())?;
    module
        .define_function(main_id, &mut ctx)
        .map_err(|e| e.to_string())?;

    let obj = module.finish();
    obj.emit().map_err(|e| e.to_string())
}
