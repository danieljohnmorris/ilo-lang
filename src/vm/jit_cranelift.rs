//! Cranelift JIT backend for numeric-only functions.
//!
//! Translates VM bytecode to Cranelift IR, which handles register allocation
//! and instruction selection. Works on both ARM64 and x86_64.

use super::*;
use cranelift_codegen::ir::{AbiParam, InstBuilder};
use cranelift_codegen::ir::types::F64;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Module};

/// Check if a chunk uses only numeric-safe opcodes.
pub(crate) fn is_jit_eligible(chunk: &Chunk) -> bool {
    for &inst in &chunk.code {
        let op = (inst >> 24) as u8;
        match op {
            OP_ADD_NN | OP_SUB_NN | OP_MUL_NN | OP_DIV_NN |
            OP_ADDK_N | OP_SUBK_N | OP_MULK_N | OP_DIVK_N |
            OP_MOVE | OP_NEG | OP_RET => {}
            OP_LOADK => {
                let bx = (inst & 0xFFFF) as usize;
                if bx >= chunk.constants.len() { return false; }
                if !matches!(chunk.constants[bx], Value::Number(_)) { return false; }
            }
            _ => return false,
        }
    }
    true
}

/// Compiled Cranelift function that can be called repeatedly.
pub(crate) struct JitFunction {
    _module: JITModule,
    func_ptr: *const u8,
    param_count: usize,
}

// The function pointer is safe to call from any thread (it's immutable code).
unsafe impl Send for JitFunction {}

/// Compile a chunk into native code via Cranelift.
pub(crate) fn compile(chunk: &Chunk, nan_consts: &[NanVal]) -> Option<JitFunction> {
    if !is_jit_eligible(chunk) { return None; }

    let mut flag_builder = settings::builder();
    flag_builder.set("opt_level", "speed").ok()?;
    let isa_builder = cranelift_native::builder().ok()?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder)).ok()?;

    let builder = JITBuilder::with_isa(isa, default_libcall_names());
    let mut module = JITModule::new(builder);

    // Build function signature: (f64, f64, ...) -> f64
    let mut sig = module.make_signature();
    for _ in 0..chunk.param_count {
        sig.params.push(AbiParam::new(F64));
    }
    sig.returns.push(AbiParam::new(F64));

    let func_id = module.declare_function("jit_func", cranelift_module::Linkage::Local, &sig).ok()?;

    let mut ctx = Context::new();
    ctx.func.signature = sig;

    let mut fn_builder_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

    // Declare variables for all VM registers
    let reg_count = chunk.reg_count.max(chunk.param_count) as usize;
    let mut vars: Vec<Variable> = Vec::with_capacity(reg_count);
    for i in 0..reg_count {
        let var = Variable::from_u32(i as u32);
        builder.declare_var(var, F64);
        vars.push(var);
    }

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // Initialize params
    for (i, var) in vars.iter().enumerate().take(chunk.param_count as usize) {
        let val = builder.block_params(entry_block)[i];
        builder.def_var(*var, val);
    }

    // Initialize non-param registers to 0.0
    for var in vars.iter().take(reg_count).skip(chunk.param_count as usize) {
        let zero = builder.ins().f64const(0.0);
        builder.def_var(*var, zero);
    }

    // Translate bytecode
    for &inst in &chunk.code {
        let op = (inst >> 24) as u8;
        let a = ((inst >> 16) & 0xFF) as usize;
        let b = ((inst >> 8) & 0xFF) as usize;
        let c = (inst & 0xFF) as usize;

        match op {
            OP_ADD_NN => {
                let bv = builder.use_var(vars[b]);
                let cv = builder.use_var(vars[c]);
                let result = builder.ins().fadd(bv, cv);
                builder.def_var(vars[a], result);
            }
            OP_SUB_NN => {
                let bv = builder.use_var(vars[b]);
                let cv = builder.use_var(vars[c]);
                let result = builder.ins().fsub(bv, cv);
                builder.def_var(vars[a], result);
            }
            OP_MUL_NN => {
                let bv = builder.use_var(vars[b]);
                let cv = builder.use_var(vars[c]);
                let result = builder.ins().fmul(bv, cv);
                builder.def_var(vars[a], result);
            }
            OP_DIV_NN => {
                let bv = builder.use_var(vars[b]);
                let cv = builder.use_var(vars[c]);
                let result = builder.ins().fdiv(bv, cv);
                builder.def_var(vars[a], result);
            }
            OP_ADDK_N => {
                let bv = builder.use_var(vars[b]);
                let kv = nan_consts.get(c)?.as_number();
                let kval = builder.ins().f64const(kv);
                let result = builder.ins().fadd(bv, kval);
                builder.def_var(vars[a], result);
            }
            OP_SUBK_N => {
                let bv = builder.use_var(vars[b]);
                let kv = nan_consts.get(c)?.as_number();
                let kval = builder.ins().f64const(kv);
                let result = builder.ins().fsub(bv, kval);
                builder.def_var(vars[a], result);
            }
            OP_MULK_N => {
                let bv = builder.use_var(vars[b]);
                let kv = nan_consts.get(c)?.as_number();
                let kval = builder.ins().f64const(kv);
                let result = builder.ins().fmul(bv, kval);
                builder.def_var(vars[a], result);
            }
            OP_DIVK_N => {
                let bv = builder.use_var(vars[b]);
                let kv = nan_consts.get(c)?.as_number();
                let kval = builder.ins().f64const(kv);
                let result = builder.ins().fdiv(bv, kval);
                builder.def_var(vars[a], result);
            }
            OP_LOADK => {
                let bx = (inst & 0xFFFF) as usize;
                let val = match &chunk.constants[bx] {
                    Value::Number(n) => *n,
                    _ => return None,
                };
                let kval = builder.ins().f64const(val);
                builder.def_var(vars[a], kval);
            }
            OP_MOVE => {
                if a != b {
                    let bv = builder.use_var(vars[b]);
                    builder.def_var(vars[a], bv);
                }
            }
            OP_NEG => {
                let bv = builder.use_var(vars[b]);
                let result = builder.ins().fneg(bv);
                builder.def_var(vars[a], result);
            }
            OP_RET => {
                let av = builder.use_var(vars[a]);
                builder.ins().return_(&[av]);
            }
            _ => return None,
        }
    }

    builder.finalize();

    module.define_function(func_id, &mut ctx).ok()?;
    module.finalize_definitions().ok()?;

    let func_ptr = module.get_finalized_function(func_id);

    Some(JitFunction {
        _module: module,
        func_ptr,
        param_count: chunk.param_count as usize,
    })
}

/// Call a compiled function.
pub(crate) fn call(func: &JitFunction, args: &[f64]) -> Option<f64> {
    if args.len() != func.param_count { return None; }
    Some(match args.len() {
        0 => {
            let f: extern "C" fn() -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f()
        }
        1 => {
            let f: extern "C" fn(f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0])
        }
        2 => {
            let f: extern "C" fn(f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1])
        }
        3 => {
            let f: extern "C" fn(f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2])
        }
        4 => {
            let f: extern "C" fn(f64, f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2], args[3])
        }
        5 => {
            let f: extern "C" fn(f64, f64, f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2], args[3], args[4])
        }
        6 => {
            let f: extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2], args[3], args[4], args[5])
        }
        7 => {
            let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2], args[3], args[4], args[5], args[6])
        }
        8 => {
            let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = unsafe { std::mem::transmute(func.func_ptr) };
            f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7])
        }
        _ => return None,
    })
}

/// Compile and call in one shot.
pub(crate) fn compile_and_call(chunk: &Chunk, nan_consts: &[NanVal], args: &[f64]) -> Option<f64> {
    let func = compile(chunk, nan_consts)?;
    call(&func, args)
}
