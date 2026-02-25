//! LLVM JIT backend for numeric-only functions.
//!
//! Uses the `inkwell` crate (safe Rust wrapper for LLVM C API).
//! LLVM brings the heaviest optimization pipeline (same passes as clang -O2).

use super::*;
use inkwell::context::Context;
use inkwell::OptimizationLevel;
use inkwell::targets::{InitializationConfig, Target};

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

/// Compiled LLVM function.
pub(crate) struct JitFunction {
    _context: Context,
    func_ptr: *const u8,
    param_count: usize,
}

unsafe impl Send for JitFunction {}

/// Compile a chunk into native code via LLVM.
pub(crate) fn compile(chunk: &Chunk, nan_consts: &[NanVal]) -> Option<JitFunction> {
    if !is_jit_eligible(chunk) { return None; }

    Target::initialize_native(&InitializationConfig::default()).ok()?;

    let context = Context::create();
    let module = context.create_module("jit");
    let builder = context.create_builder();

    // Build function type: (f64, f64, ...) -> f64
    let f64_type = context.f64_type();
    let param_types: Vec<_> = (0..chunk.param_count).map(|_| f64_type.into()).collect();
    let fn_type = f64_type.fn_type(&param_types, false);
    let function = module.add_function("jit_func", fn_type, None);

    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);

    // Map VM registers to LLVM values
    let reg_count = chunk.reg_count.max(chunk.param_count) as usize;
    let mut regs: Vec<inkwell::values::FloatValue> = Vec::with_capacity(reg_count);

    // Initialize from params
    for i in 0..chunk.param_count as usize {
        regs.push(function.get_nth_param(i as u32).unwrap().into_float_value());
    }
    // Initialize remaining to 0.0
    for _ in chunk.param_count as usize..reg_count {
        regs.push(f64_type.const_float(0.0));
    }

    // Translate bytecode
    for &inst in &chunk.code {
        let op = (inst >> 24) as u8;
        let a = ((inst >> 16) & 0xFF) as usize;
        let b = ((inst >> 8) & 0xFF) as usize;
        let c = (inst & 0xFF) as usize;

        match op {
            OP_ADD_NN => {
                let result = builder.build_float_add(regs[b], regs[c], "add").ok()?;
                regs[a] = result;
            }
            OP_SUB_NN => {
                let result = builder.build_float_sub(regs[b], regs[c], "sub").ok()?;
                regs[a] = result;
            }
            OP_MUL_NN => {
                let result = builder.build_float_mul(regs[b], regs[c], "mul").ok()?;
                regs[a] = result;
            }
            OP_DIV_NN => {
                let result = builder.build_float_div(regs[b], regs[c], "div").ok()?;
                regs[a] = result;
            }
            OP_ADDK_N => {
                let kv = nan_consts.get(c)?.as_number();
                let kval = f64_type.const_float(kv);
                let result = builder.build_float_add(regs[b], kval, "addk").ok()?;
                regs[a] = result;
            }
            OP_SUBK_N => {
                let kv = nan_consts.get(c)?.as_number();
                let kval = f64_type.const_float(kv);
                let result = builder.build_float_sub(regs[b], kval, "subk").ok()?;
                regs[a] = result;
            }
            OP_MULK_N => {
                let kv = nan_consts.get(c)?.as_number();
                let kval = f64_type.const_float(kv);
                let result = builder.build_float_mul(regs[b], kval, "mulk").ok()?;
                regs[a] = result;
            }
            OP_DIVK_N => {
                let kv = nan_consts.get(c)?.as_number();
                let kval = f64_type.const_float(kv);
                let result = builder.build_float_div(regs[b], kval, "divk").ok()?;
                regs[a] = result;
            }
            OP_LOADK => {
                let bx = (inst & 0xFFFF) as usize;
                let val = match &chunk.constants[bx] {
                    Value::Number(n) => *n,
                    _ => return None,
                };
                regs[a] = f64_type.const_float(val);
            }
            OP_MOVE => {
                if a != b {
                    regs[a] = regs[b];
                }
            }
            OP_NEG => {
                let result = builder.build_float_neg(regs[b], "neg").ok()?;
                regs[a] = result;
            }
            OP_RET => {
                builder.build_return(Some(&regs[a])).ok()?;
            }
            _ => return None,
        }
    }

    // Create execution engine with O2 optimization
    let engine = module.create_jit_execution_engine(OptimizationLevel::Aggressive).ok()?;
    let func_ptr = engine.get_function_address("jit_func").ok()? as *const u8;

    // We need to keep the context alive — but execution engine owns the module.
    // SAFETY: The function pointer remains valid as long as context + engine live.
    // We leak the engine to keep the code alive (it's a one-shot JIT).
    std::mem::forget(engine);

    Some(JitFunction {
        _context: context,
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
