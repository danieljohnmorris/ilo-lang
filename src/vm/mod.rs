use std::collections::HashMap;
use std::rc::Rc;
use crate::ast::*;
use crate::interpreter::Value;

// ── Opcodes ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Op {
    LoadConst(u16),
    LoadLocal(u16),
    StoreLocal(u16),
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Gt,
    Lt,
    Ge,
    Le,
    Not,
    Negate,
    WrapOk,
    WrapErr,
    JumpIfFalse(u16),
    JumpIfTrue(u16),
    Jump(u16),
    Call(u16, u8),
    Return,
    /// Create record: (descriptor_const_idx, n_field_values_on_stack)
    /// Descriptor const is List [type_name_str, List [field_name_strs...]]
    RecordNew(u16, u8),
    /// Field access: (field_name_const_idx) — pops record, pushes field value
    RecordField(u16),
    /// Record with: (field_names_const_idx, n_updates)
    /// Stack: [record, val1, val2, ...] → [updated_record]
    RecordWith(u16, u8),
    Pop,
    Dup,
    IsOk,
    IsErr,
    UnwrapOkErr,
    /// Create list from N items on stack
    ListNew(u16),
    /// Stack: [list, index] → [item] or jump to target if out of bounds
    ListGetOrEnd(u16),
}

// ── Chunk ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub constants: Vec<Value>,
    pub param_count: u8,
}

impl Chunk {
    fn new(param_count: u8) -> Self {
        Chunk { code: Vec::new(), constants: Vec::new(), param_count }
    }

    fn add_const(&mut self, val: Value) -> u16 {
        // Reuse existing constant if identical (simple types only)
        for (i, c) in self.constants.iter().enumerate() {
            match (c, &val) {
                (Value::Number(a), Value::Number(b)) if (a - b).abs() < f64::EPSILON => return i as u16,
                (Value::Text(a), Value::Text(b)) if a == b => return i as u16,
                (Value::Bool(a), Value::Bool(b)) if a == b => return i as u16,
                (Value::Nil, Value::Nil) => return i as u16,
                _ => {}
            }
        }
        let idx = self.constants.len() as u16;
        self.constants.push(val);
        idx
    }

    fn emit(&mut self, op: Op) -> usize {
        let idx = self.code.len();
        self.code.push(op);
        idx
    }

    fn patch_jump(&mut self, idx: usize) {
        let target = self.code.len() as u16;
        match &mut self.code[idx] {
            Op::JumpIfFalse(a) | Op::JumpIfTrue(a) | Op::Jump(a) | Op::ListGetOrEnd(a) => *a = target,
            _ => panic!("tried to patch non-jump at {}", idx),
        }
    }
}

// ── Compiled program ─────────────────────────────────────────────────

pub struct CompiledProgram {
    pub chunks: Vec<Chunk>,
    pub func_names: Vec<String>,
    /// Pre-computed NaN-boxed constants, parallel to chunks.
    /// Stored here (not in Chunk) to avoid Chunk Clone/Drop complications.
    nan_constants: Vec<Vec<NanVal>>,
}

impl CompiledProgram {
    fn func_index(&self, name: &str) -> Option<u16> {
        self.func_names.iter().position(|n| n == name).map(|i| i as u16)
    }
}

impl Drop for CompiledProgram {
    fn drop(&mut self) {
        for chunk_consts in &self.nan_constants {
            for v in chunk_consts {
                v.drop_rc();
            }
        }
    }
}

// ── Compiler ─────────────────────────────────────────────────────────

struct Compiler {
    chunks: Vec<Chunk>,
    func_names: Vec<String>,
    current_chunk: Chunk,
    locals: Vec<String>,
}

impl Compiler {
    fn new() -> Self {
        Compiler {
            chunks: Vec::new(),
            func_names: Vec::new(),
            current_chunk: Chunk::new(0),
            locals: Vec::new(),
        }
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        self.locals.iter().rposition(|n| n == name).map(|i| i as u16)
    }

    fn add_local(&mut self, name: &str) -> u16 {
        let idx = self.locals.len() as u16;
        self.locals.push(name.to_string());
        idx
    }

    fn compile_program(mut self, program: &Program) -> CompiledProgram {
        // Register function names (so Call can resolve during compilation)
        for decl in &program.declarations {
            match decl {
                Decl::Function { name, .. } | Decl::Tool { name, .. } => {
                    self.func_names.push(name.clone());
                }
                Decl::TypeDef { .. } => {}
            }
        }

        for decl in &program.declarations {
            if let Decl::Function { params, body, .. } = decl {
                self.current_chunk = Chunk::new(params.len() as u8);
                self.locals.clear();

                for p in params {
                    self.add_local(&p.name);
                }

                self.compile_body(body, true);

                if !matches!(self.current_chunk.code.last(), Some(Op::Return)) {
                    self.current_chunk.emit(Op::Return);
                }

                self.chunks.push(self.current_chunk.clone());
            } else {
                // Placeholder chunk for tools/typedefs
                self.chunks.push(Chunk::new(0));
            }
        }

        CompiledProgram { chunks: self.chunks, func_names: self.func_names, nan_constants: Vec::new() }
    }

    fn compile_body(&mut self, stmts: &[Stmt], is_func_body: bool) {
        let saved = self.locals.len();
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == stmts.len() - 1;
            self.compile_stmt(stmt, is_last, is_func_body);
        }
        self.locals.truncate(saved);
    }

    fn compile_stmt(&mut self, stmt: &Stmt, is_last: bool, is_func_body: bool) {
        match stmt {
            Stmt::Let { name, value } => {
                self.compile_expr(value);
                let slot = self.add_local(name);
                self.current_chunk.emit(Op::StoreLocal(slot));
            }

            Stmt::Guard { condition, negated, body } => {
                self.compile_expr(condition);
                let jump = if *negated {
                    self.current_chunk.emit(Op::JumpIfTrue(0))
                } else {
                    self.current_chunk.emit(Op::JumpIfFalse(0))
                };
                self.compile_body(body, false);
                self.current_chunk.emit(Op::Return);
                self.current_chunk.patch_jump(jump);
            }

            Stmt::Match { subject, arms } => {
                match subject {
                    Some(e) => self.compile_expr(e),
                    None => {
                        let idx = self.current_chunk.add_const(Value::Nil);
                        self.current_chunk.emit(Op::LoadConst(idx));
                    }
                }
                self.compile_match_arms(arms, is_last && is_func_body);
            }

            Stmt::ForEach { binding, collection, body } => {
                // Compile collection, store it
                self.compile_expr(collection);
                let coll_slot = self.add_local("__fe_coll");
                self.current_chunk.emit(Op::StoreLocal(coll_slot));

                // Index = 0
                let zero = self.current_chunk.add_const(Value::Number(0.0));
                self.current_chunk.emit(Op::LoadConst(zero));
                let idx_slot = self.add_local("__fe_idx");
                self.current_chunk.emit(Op::StoreLocal(idx_slot));

                // last = nil
                let nil = self.current_chunk.add_const(Value::Nil);
                self.current_chunk.emit(Op::LoadConst(nil));
                let last_slot = self.add_local("__fe_last");
                self.current_chunk.emit(Op::StoreLocal(last_slot));

                // binding = nil (placeholder)
                self.current_chunk.emit(Op::LoadConst(nil));
                let bind_slot = self.add_local(binding);
                self.current_chunk.emit(Op::StoreLocal(bind_slot));

                // Loop top
                let loop_top = self.current_chunk.code.len();
                self.current_chunk.emit(Op::LoadLocal(coll_slot));
                self.current_chunk.emit(Op::LoadLocal(idx_slot));
                let exit = self.current_chunk.emit(Op::ListGetOrEnd(0));

                // Store item into binding
                self.current_chunk.emit(Op::StoreLocal(bind_slot));

                // Compile body
                let saved = self.locals.len();
                for (si, s) in body.iter().enumerate() {
                    let sl = si == body.len() - 1;
                    self.compile_stmt(s, sl, false);
                }
                self.locals.truncate(saved);

                // Store body result as last
                self.current_chunk.emit(Op::StoreLocal(last_slot));

                // idx += 1
                self.current_chunk.emit(Op::LoadLocal(idx_slot));
                let one = self.current_chunk.add_const(Value::Number(1.0));
                self.current_chunk.emit(Op::LoadConst(one));
                self.current_chunk.emit(Op::Add);
                self.current_chunk.emit(Op::StoreLocal(idx_slot));

                self.current_chunk.emit(Op::Jump(loop_top as u16));
                self.current_chunk.patch_jump(exit);

                // Push last value as result
                self.current_chunk.emit(Op::LoadLocal(last_slot));
            }

            Stmt::Expr(expr) => {
                self.compile_expr(expr);
                // Result stays on stack (last expr becomes return value)
            }
        }
    }

    fn compile_match_arms(&mut self, arms: &[MatchArm], should_return: bool) {
        // Subject is on top of stack
        let mut end_jumps = Vec::new();

        for arm in arms {
            self.current_chunk.emit(Op::Dup); // dup subject for pattern test

            match &arm.pattern {
                Pattern::Wildcard => {
                    self.current_chunk.emit(Op::Pop); // pop dup
                    self.current_chunk.emit(Op::Pop); // pop original subject
                    self.compile_body(&arm.body, false);
                    if should_return {
                        self.current_chunk.emit(Op::Return);
                    }
                    // Patch all prior end_jumps to land here
                    for j in end_jumps {
                        self.current_chunk.patch_jump(j);
                    }
                    return; // wildcard is terminal
                }

                Pattern::Ok(binding) => {
                    // Dup is on stack. IsOk pops it, pushes bool.
                    self.current_chunk.emit(Op::IsOk);
                    let skip = self.current_chunk.emit(Op::JumpIfFalse(0));

                    // Matched: subject still on stack. Unwrap it.
                    self.current_chunk.emit(Op::Dup);
                    self.current_chunk.emit(Op::UnwrapOkErr);
                    if binding != "_" {
                        let slot = self.add_local(binding);
                        self.current_chunk.emit(Op::StoreLocal(slot));
                    } else {
                        self.current_chunk.emit(Op::Pop);
                    }
                    self.current_chunk.emit(Op::Pop); // pop subject

                    self.compile_body(&arm.body, false);
                    if should_return {
                        self.current_chunk.emit(Op::Return);
                    } else {
                        end_jumps.push(self.current_chunk.emit(Op::Jump(0)));
                    }
                    self.current_chunk.patch_jump(skip);
                }

                Pattern::Err(binding) => {
                    self.current_chunk.emit(Op::IsErr);
                    let skip = self.current_chunk.emit(Op::JumpIfFalse(0));

                    self.current_chunk.emit(Op::Dup);
                    self.current_chunk.emit(Op::UnwrapOkErr);
                    if binding != "_" {
                        let slot = self.add_local(binding);
                        self.current_chunk.emit(Op::StoreLocal(slot));
                    } else {
                        self.current_chunk.emit(Op::Pop);
                    }
                    self.current_chunk.emit(Op::Pop);

                    self.compile_body(&arm.body, false);
                    if should_return {
                        self.current_chunk.emit(Op::Return);
                    } else {
                        end_jumps.push(self.current_chunk.emit(Op::Jump(0)));
                    }
                    self.current_chunk.patch_jump(skip);
                }

                Pattern::Literal(lit) => {
                    let val = match lit {
                        Literal::Number(n) => Value::Number(*n),
                        Literal::Text(s) => Value::Text(s.clone()),
                        Literal::Bool(b) => Value::Bool(*b),
                    };
                    let idx = self.current_chunk.add_const(val);
                    self.current_chunk.emit(Op::LoadConst(idx));
                    self.current_chunk.emit(Op::Eq);
                    let skip = self.current_chunk.emit(Op::JumpIfFalse(0));

                    self.current_chunk.emit(Op::Pop); // pop subject
                    self.compile_body(&arm.body, false);
                    if should_return {
                        self.current_chunk.emit(Op::Return);
                    } else {
                        end_jumps.push(self.current_chunk.emit(Op::Jump(0)));
                    }
                    self.current_chunk.patch_jump(skip);
                }
            }
        }

        // No arm matched: pop subject, push nil
        self.current_chunk.emit(Op::Pop);
        let nil = self.current_chunk.add_const(Value::Nil);
        self.current_chunk.emit(Op::LoadConst(nil));

        for j in end_jumps {
            self.current_chunk.patch_jump(j);
        }
    }

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(lit) => {
                let val = match lit {
                    Literal::Number(n) => Value::Number(*n),
                    Literal::Text(s) => Value::Text(s.clone()),
                    Literal::Bool(b) => Value::Bool(*b),
                };
                let idx = self.current_chunk.add_const(val);
                self.current_chunk.emit(Op::LoadConst(idx));
            }

            Expr::Ref(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    self.current_chunk.emit(Op::LoadLocal(slot));
                } else {
                    panic!("undefined variable in compiler: {}", name);
                }
            }

            Expr::Field { object, field } => {
                self.compile_expr(object);
                let idx = self.current_chunk.add_const(Value::Text(field.clone()));
                self.current_chunk.emit(Op::RecordField(idx));
            }

            Expr::Call { function, args } => {
                for arg in args {
                    self.compile_expr(arg);
                }
                let func_idx = self.func_names.iter().position(|n| n == function)
                    .unwrap_or_else(|| panic!("undefined function in compiler: {}", function));
                self.current_chunk.emit(Op::Call(func_idx as u16, args.len() as u8));
            }

            Expr::BinOp { op, left, right } => {
                self.compile_expr(left);
                self.compile_expr(right);
                match op {
                    BinOp::Add => { self.current_chunk.emit(Op::Add); }
                    BinOp::Subtract => { self.current_chunk.emit(Op::Sub); }
                    BinOp::Multiply => { self.current_chunk.emit(Op::Mul); }
                    BinOp::Divide => { self.current_chunk.emit(Op::Div); }
                    BinOp::Equals => { self.current_chunk.emit(Op::Eq); }
                    BinOp::NotEquals => { self.current_chunk.emit(Op::NotEq); }
                    BinOp::GreaterThan => { self.current_chunk.emit(Op::Gt); }
                    BinOp::LessThan => { self.current_chunk.emit(Op::Lt); }
                    BinOp::GreaterOrEqual => { self.current_chunk.emit(Op::Ge); }
                    BinOp::LessOrEqual => { self.current_chunk.emit(Op::Le); }
                };
            }

            Expr::UnaryOp { op, operand } => {
                self.compile_expr(operand);
                match op {
                    UnaryOp::Not => { self.current_chunk.emit(Op::Not); }
                    UnaryOp::Negate => { self.current_chunk.emit(Op::Negate); }
                };
            }

            Expr::Ok(inner) => {
                self.compile_expr(inner);
                self.current_chunk.emit(Op::WrapOk);
            }

            Expr::Err(inner) => {
                self.compile_expr(inner);
                self.current_chunk.emit(Op::WrapErr);
            }

            Expr::List(items) => {
                for item in items {
                    self.compile_expr(item);
                }
                self.current_chunk.emit(Op::ListNew(items.len() as u16));
            }

            Expr::Record { type_name, fields } => {
                // Push field values onto stack
                for (_, val_expr) in fields {
                    self.compile_expr(val_expr);
                }
                // Build descriptor: [type_name, [field_names...]]
                let desc = Value::List(vec![
                    Value::Text(type_name.clone()),
                    Value::List(fields.iter().map(|(n, _)| Value::Text(n.clone())).collect()),
                ]);
                let desc_idx = self.current_chunk.constants.len() as u16;
                self.current_chunk.constants.push(desc); // don't dedup complex values
                self.current_chunk.emit(Op::RecordNew(desc_idx, fields.len() as u8));
            }

            Expr::Match { subject, arms } => {
                match subject {
                    Some(e) => self.compile_expr(e),
                    None => {
                        let idx = self.current_chunk.add_const(Value::Nil);
                        self.current_chunk.emit(Op::LoadConst(idx));
                    }
                }
                self.compile_match_arms(arms, false);
            }

            Expr::With { object, updates } => {
                self.compile_expr(object);
                // Push update values
                for (_, val_expr) in updates {
                    self.compile_expr(val_expr);
                }
                // Field names descriptor
                let names = Value::List(
                    updates.iter().map(|(n, _)| Value::Text(n.clone())).collect()
                );
                let names_idx = self.current_chunk.constants.len() as u16;
                self.current_chunk.constants.push(names);
                self.current_chunk.emit(Op::RecordWith(names_idx, updates.len() as u8));
            }
        }
    }
}

// ── NaN-boxed value ──────────────────────────────────────────────────
//
// IEEE 754 quiet NaN has 51 unused payload bits. We use them to encode
// all ilo value types in a single Copy u64, making the VM stack
// Vec<u64>-equivalent with zero-cost number operations.

const QNAN: u64       = 0x7FFC_0000_0000_0000;
const TAG_NIL: u64    = QNAN;
const TAG_TRUE: u64   = QNAN | 1;
const TAG_FALSE: u64  = QNAN | 2;
const TAG_STRING: u64 = 0x7FFD_0000_0000_0000;
const TAG_LIST: u64   = 0x7FFE_0000_0000_0000;
const TAG_RECORD: u64 = 0x7FFF_0000_0000_0000;
const TAG_OK: u64     = 0xFFFC_0000_0000_0000;
const TAG_ERR: u64    = 0xFFFD_0000_0000_0000;
const PTR_MASK: u64   = 0x0000_FFFF_FFFF_FFFF;
const TAG_MASK: u64   = 0xFFFF_0000_0000_0000;

enum HeapObj {
    Str(String),
    List(Vec<NanVal>),
    Record { type_name: String, fields: HashMap<String, NanVal> },
    OkVal(NanVal),
    ErrVal(NanVal),
}

impl Drop for HeapObj {
    fn drop(&mut self) {
        match self {
            HeapObj::Str(_) => {}
            HeapObj::List(items) => {
                for item in items {
                    item.drop_rc();
                }
            }
            HeapObj::Record { fields, .. } => {
                for val in fields.values() {
                    val.drop_rc();
                }
            }
            HeapObj::OkVal(inner) | HeapObj::ErrVal(inner) => {
                inner.drop_rc();
            }
        }
    }
}

#[derive(Clone, Copy)]
struct NanVal(u64);

impl NanVal {
    #[inline]
    fn number(n: f64) -> Self {
        if n.is_nan() {
            NanVal(0x7FF8_0000_0000_0000) // canonical NaN, outside our tag space
        } else {
            NanVal(n.to_bits())
        }
    }

    #[inline]
    fn nil() -> Self { NanVal(TAG_NIL) }

    #[inline]
    fn boolean(b: bool) -> Self {
        NanVal(if b { TAG_TRUE } else { TAG_FALSE })
    }

    fn heap_string(s: String) -> Self {
        let rc = Rc::new(HeapObj::Str(s));
        let ptr = Rc::into_raw(rc) as u64;
        NanVal(TAG_STRING | (ptr & PTR_MASK))
    }

    fn heap_list(items: Vec<NanVal>) -> Self {
        let rc = Rc::new(HeapObj::List(items));
        let ptr = Rc::into_raw(rc) as u64;
        NanVal(TAG_LIST | (ptr & PTR_MASK))
    }

    fn heap_record(type_name: String, fields: HashMap<String, NanVal>) -> Self {
        let rc = Rc::new(HeapObj::Record { type_name, fields });
        let ptr = Rc::into_raw(rc) as u64;
        NanVal(TAG_RECORD | (ptr & PTR_MASK))
    }

    fn heap_ok(inner: NanVal) -> Self {
        let rc = Rc::new(HeapObj::OkVal(inner));
        let ptr = Rc::into_raw(rc) as u64;
        NanVal(TAG_OK | (ptr & PTR_MASK))
    }

    fn heap_err(inner: NanVal) -> Self {
        let rc = Rc::new(HeapObj::ErrVal(inner));
        let ptr = Rc::into_raw(rc) as u64;
        NanVal(TAG_ERR | (ptr & PTR_MASK))
    }

    #[inline]
    fn is_number(self) -> bool {
        (self.0 & QNAN) != QNAN
    }

    #[inline]
    fn as_number(self) -> f64 {
        f64::from_bits(self.0)
    }

    #[inline]
    fn is_heap(self) -> bool {
        (self.0 & QNAN) == QNAN && self.0 != TAG_NIL && self.0 != TAG_TRUE && self.0 != TAG_FALSE
    }

    #[inline]
    fn is_string(self) -> bool {
        (self.0 & TAG_MASK) == TAG_STRING
    }

    #[inline]
    unsafe fn as_heap_ref<'a>(self) -> &'a HeapObj {
        let ptr = (self.0 & PTR_MASK) as *const HeapObj;
        unsafe { &*ptr }
    }

    #[inline]
    fn clone_rc(self) {
        if self.is_heap() {
            let ptr = (self.0 & PTR_MASK) as *const HeapObj;
            unsafe { Rc::increment_strong_count(ptr); }
        }
    }

    #[inline]
    fn drop_rc(self) {
        if self.is_heap() {
            let ptr = (self.0 & PTR_MASK) as *const HeapObj;
            unsafe { Rc::decrement_strong_count(ptr); }
        }
    }

    fn from_value(val: &Value) -> Self {
        match val {
            Value::Number(n) => NanVal::number(*n),
            Value::Bool(b) => NanVal::boolean(*b),
            Value::Nil => NanVal::nil(),
            Value::Text(s) => NanVal::heap_string(s.clone()),
            Value::List(items) => {
                NanVal::heap_list(items.iter().map(|v| NanVal::from_value(v)).collect())
            }
            Value::Record { type_name, fields } => {
                NanVal::heap_record(
                    type_name.clone(),
                    fields.iter().map(|(k, v)| (k.clone(), NanVal::from_value(v))).collect(),
                )
            }
            Value::Ok(inner) => NanVal::heap_ok(NanVal::from_value(inner)),
            Value::Err(inner) => NanVal::heap_err(NanVal::from_value(inner)),
        }
    }

    fn to_value(self) -> Value {
        if self.is_number() {
            return Value::Number(self.as_number());
        }
        match self.0 {
            TAG_NIL => Value::Nil,
            TAG_TRUE => Value::Bool(true),
            TAG_FALSE => Value::Bool(false),
            _ => unsafe {
                match self.as_heap_ref() {
                    HeapObj::Str(s) => Value::Text(s.clone()),
                    HeapObj::List(items) => {
                        Value::List(items.iter().map(|v| v.to_value()).collect())
                    }
                    HeapObj::Record { type_name, fields } => Value::Record {
                        type_name: type_name.clone(),
                        fields: fields.iter().map(|(k, v)| (k.clone(), v.to_value())).collect(),
                    },
                    HeapObj::OkVal(inner) => Value::Ok(Box::new(inner.to_value())),
                    HeapObj::ErrVal(inner) => Value::Err(Box::new(inner.to_value())),
                }
            }
        }
    }
}

// ── VM ───────────────────────────────────────────────────────────────

pub fn compile(program: &Program) -> CompiledProgram {
    let mut prog = Compiler::new().compile_program(program);
    prog.nan_constants = prog.chunks.iter()
        .map(|chunk| chunk.constants.iter().map(|v| NanVal::from_value(v)).collect())
        .collect();
    prog
}

pub fn run(compiled: &CompiledProgram, func_name: Option<&str>, args: Vec<Value>) -> Result<Value, String> {
    let target = match func_name {
        Some(name) => name.to_string(),
        None => compiled.func_names.first().ok_or("no functions defined")?.clone(),
    };
    let func_idx = compiled.func_index(&target)
        .ok_or_else(|| format!("undefined function: {}", target))?;
    VM::new(compiled).call(func_idx, args)
}

#[cfg(test)]
pub fn compile_and_run(program: &Program, func_name: Option<&str>, args: Vec<Value>) -> Result<Value, String> {
    let compiled = compile(program);
    run(&compiled, func_name, args)
}

/// Reusable VM handle — avoids re-allocating stack/frames per call.
pub struct VmState<'a> {
    vm: VM<'a>,
}

impl<'a> VmState<'a> {
    pub fn new(compiled: &'a CompiledProgram) -> Self {
        VmState { vm: VM::new(compiled) }
    }

    pub fn call(&mut self, func_name: &str, args: Vec<Value>) -> Result<Value, String> {
        // Drop any leaked NanVals from a prior failed call
        for v in self.vm.stack.drain(..) {
            v.drop_rc();
        }
        self.vm.frames.clear();

        let func_idx = self.vm.program.func_index(func_name)
            .ok_or_else(|| format!("undefined function: {}", func_name))?;
        let nan_args: Vec<NanVal> = args.iter().map(|v| NanVal::from_value(v)).collect();
        self.vm.setup_call(func_idx, nan_args);
        self.vm.execute()
    }
}

struct CallFrame {
    chunk_idx: u16,
    ip: usize,
    stack_base: usize,
}

struct VM<'a> {
    program: &'a CompiledProgram,
    stack: Vec<NanVal>,
    frames: Vec<CallFrame>,
}

impl<'a> VM<'a> {
    fn new(program: &'a CompiledProgram) -> Self {
        VM { program, stack: Vec::with_capacity(256), frames: Vec::with_capacity(64) }
    }

    fn max_local_slot(chunk: &Chunk) -> usize {
        let mut max = chunk.param_count as usize;
        for op in &chunk.code {
            match op {
                Op::StoreLocal(s) | Op::LoadLocal(s) => {
                    let s = *s as usize + 1;
                    if s > max { max = s; }
                }
                _ => {}
            }
        }
        max
    }

    fn setup_call(&mut self, func_idx: u16, args: Vec<NanVal>) {
        let chunk = &self.program.chunks[func_idx as usize];
        let stack_base = self.stack.len();

        for arg in args {
            self.stack.push(arg);
        }

        let max_locals = Self::max_local_slot(chunk);
        while self.stack.len() < stack_base + max_locals {
            self.stack.push(NanVal::nil());
        }

        self.frames.push(CallFrame { chunk_idx: func_idx, ip: 0, stack_base });
    }

    fn call(&mut self, func_idx: u16, args: Vec<Value>) -> Result<Value, String> {
        let nan_args: Vec<NanVal> = args.iter().map(|v| NanVal::from_value(v)).collect();
        self.setup_call(func_idx, nan_args);
        self.execute()
    }

    fn pop(&mut self) -> NanVal {
        self.stack.pop().expect("stack underflow")
    }

    fn execute(&mut self) -> Result<Value, String> {
        // Cache frame state in locals to avoid frames.last() per opcode
        let frame = unsafe { self.frames.last().unwrap_unchecked() };
        let mut ci = frame.chunk_idx as usize;
        let mut ip = frame.ip;
        let mut base = frame.stack_base;

        loop {
            let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
            let nan_consts = unsafe { self.program.nan_constants.get_unchecked(ci) };

            if ip >= chunk.code.len() {
                // Implicit return at end of chunk
                let result = if self.stack.len() > base + Self::max_local_slot(chunk) {
                    self.stack.pop().unwrap()
                } else {
                    NanVal::nil()
                };
                for i in base..self.stack.len() {
                    self.stack[i].drop_rc();
                }
                self.stack.truncate(base);
                self.frames.pop();
                if self.frames.is_empty() {
                    let val = result.to_value();
                    result.drop_rc();
                    return Ok(val);
                }
                self.stack.push(result);
                let f = unsafe { self.frames.last().unwrap_unchecked() };
                ci = f.chunk_idx as usize;
                ip = f.ip;
                base = f.stack_base;
                continue;
            }

            let op = unsafe { chunk.code.get_unchecked(ip) }.clone();
            ip += 1;

            match op {
                Op::LoadConst(idx) => {
                    let v = unsafe { *nan_consts.get_unchecked(idx as usize) };
                    v.clone_rc();
                    self.stack.push(v);
                }
                Op::LoadLocal(slot) => {
                    let v = unsafe { *self.stack.get_unchecked(base + slot as usize) };
                    v.clone_rc();
                    self.stack.push(v);
                }
                Op::StoreLocal(slot) => {
                    let val = self.pop();
                    let abs = base + slot as usize;
                    while self.stack.len() <= abs {
                        self.stack.push(NanVal::nil());
                    }
                    self.stack[abs].drop_rc();
                    self.stack[abs] = val;
                }
                Op::Add => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::number(a.as_number() + b.as_number()));
                    } else if a.is_string() && b.is_string() {
                        let result = unsafe {
                            let sa = match a.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
                            let sb = match b.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
                            NanVal::heap_string(format!("{}{}", sa, sb))
                        };
                        a.drop_rc();
                        b.drop_rc();
                        self.stack.push(result);
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot add non-matching types".to_string());
                    }
                }
                Op::Sub => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::number(a.as_number() - b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot subtract non-numbers".to_string());
                    }
                }
                Op::Mul => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::number(a.as_number() * b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot multiply non-numbers".to_string());
                    }
                }
                Op::Div => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        let bv = b.as_number();
                        if bv == 0.0 {
                            return Err("division by zero".to_string());
                        }
                        self.stack.push(NanVal::number(a.as_number() / bv));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot divide non-numbers".to_string());
                    }
                }
                Op::Eq => {
                    let b = self.pop();
                    let a = self.pop();
                    let eq = nanval_equal(a, b);
                    a.drop_rc();
                    b.drop_rc();
                    self.stack.push(NanVal::boolean(eq));
                }
                Op::NotEq => {
                    let b = self.pop();
                    let a = self.pop();
                    let eq = nanval_equal(a, b);
                    a.drop_rc();
                    b.drop_rc();
                    self.stack.push(NanVal::boolean(!eq));
                }
                Op::Gt => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::boolean(a.as_number() > b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot compare > non-numbers".to_string());
                    }
                }
                Op::Lt => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::boolean(a.as_number() < b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot compare < non-numbers".to_string());
                    }
                }
                Op::Ge => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::boolean(a.as_number() >= b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot compare >= non-numbers".to_string());
                    }
                }
                Op::Le => {
                    let b = self.pop();
                    let a = self.pop();
                    if a.is_number() && b.is_number() {
                        self.stack.push(NanVal::boolean(a.as_number() <= b.as_number()));
                    } else {
                        a.drop_rc();
                        b.drop_rc();
                        return Err("cannot compare <= non-numbers".to_string());
                    }
                }
                Op::Not => {
                    let v = self.pop();
                    let t = nanval_truthy(v);
                    v.drop_rc();
                    self.stack.push(NanVal::boolean(!t));
                }
                Op::Negate => {
                    let v = self.pop();
                    if v.is_number() {
                        self.stack.push(NanVal::number(-v.as_number()));
                    } else {
                        v.drop_rc();
                        return Err("cannot negate non-number".to_string());
                    }
                }
                Op::WrapOk => {
                    let v = self.pop();
                    self.stack.push(NanVal::heap_ok(v));
                }
                Op::WrapErr => {
                    let v = self.pop();
                    self.stack.push(NanVal::heap_err(v));
                }
                Op::JumpIfFalse(target) => {
                    let v = self.pop();
                    let t = nanval_truthy(v);
                    v.drop_rc();
                    if !t {
                        ip = target as usize;
                    }
                }
                Op::JumpIfTrue(target) => {
                    let v = self.pop();
                    let t = nanval_truthy(v);
                    v.drop_rc();
                    if t {
                        ip = target as usize;
                    }
                }
                Op::Jump(target) => {
                    ip = target as usize;
                }
                Op::Call(func_idx, n_args) => {
                    // Save ip back to current frame before switching
                    unsafe { self.frames.last_mut().unwrap_unchecked() }.ip = ip;
                    let n = n_args as usize;
                    let args_start = self.stack.len() - n;
                    let args: Vec<NanVal> = self.stack.drain(args_start..).collect();
                    self.setup_call(func_idx, args);
                    // Load new frame state
                    let f = unsafe { self.frames.last().unwrap_unchecked() };
                    ci = f.chunk_idx as usize;
                    ip = f.ip;
                    base = f.stack_base;
                }
                Op::Return => {
                    let result = self.pop();
                    for i in base..self.stack.len() {
                        self.stack[i].drop_rc();
                    }
                    self.stack.truncate(base);
                    self.frames.pop();
                    if self.frames.is_empty() {
                        let val = result.to_value();
                        result.drop_rc();
                        return Ok(val);
                    }
                    self.stack.push(result);
                    // Restore parent frame state
                    let f = unsafe { self.frames.last().unwrap_unchecked() };
                    ci = f.chunk_idx as usize;
                    ip = f.ip;
                    base = f.stack_base;
                }
                Op::RecordNew(desc_idx, n_fields) => {
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let desc = chunk.constants[desc_idx as usize].clone();
                    let (type_name, field_names) = unpack_record_desc(desc)?;
                    let n = n_fields as usize;
                    let start = self.stack.len() - n;
                    let vals: Vec<NanVal> = self.stack.drain(start..).collect();
                    let mut fields = HashMap::new();
                    for (name, val) in field_names.into_iter().zip(vals) {
                        fields.insert(name, val);
                    }
                    self.stack.push(NanVal::heap_record(type_name, fields));
                }
                Op::RecordField(field_idx) => {
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let field_name = match &chunk.constants[field_idx as usize] {
                        Value::Text(s) => s.as_str(),
                        _ => return Err("RecordField expects string constant".to_string()),
                    };
                    let record = self.pop();
                    let field_val = unsafe {
                        match record.as_heap_ref() {
                            HeapObj::Record { fields, .. } => {
                                match fields.get(field_name) {
                                    Some(&val) => {
                                        val.clone_rc();
                                        val
                                    }
                                    None => return Err(format!("no field '{}' on record", field_name)),
                                }
                            }
                            _ => return Err("field access on non-record".to_string()),
                        }
                    };
                    record.drop_rc();
                    self.stack.push(field_val);
                }
                Op::RecordWith(names_idx, n_updates) => {
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let field_names = unpack_string_list(&chunk.constants[names_idx as usize])?;
                    let n = n_updates as usize;
                    let start = self.stack.len() - n;
                    let update_vals: Vec<NanVal> = self.stack.drain(start..).collect();
                    let old_record = self.pop();
                    unsafe {
                        match old_record.as_heap_ref() {
                            HeapObj::Record { type_name, fields } => {
                                let mut new_fields = HashMap::new();
                                for (k, v) in fields {
                                    v.clone_rc();
                                    new_fields.insert(k.clone(), *v);
                                }
                                for (name, val) in field_names.into_iter().zip(update_vals) {
                                    if let Some(old_val) = new_fields.insert(name, val) {
                                        old_val.drop_rc();
                                    }
                                }
                                let new_record = NanVal::heap_record(type_name.clone(), new_fields);
                                old_record.drop_rc();
                                self.stack.push(new_record);
                            }
                            _ => return Err("'with' requires a record".to_string()),
                        }
                    }
                }
                Op::Pop => {
                    self.pop().drop_rc();
                }
                Op::Dup => {
                    let v = *self.stack.last().expect("stack underflow");
                    v.clone_rc();
                    self.stack.push(v);
                }
                Op::IsOk => {
                    let v = self.pop();
                    let is_ok = (v.0 & TAG_MASK) == TAG_OK;
                    v.drop_rc();
                    self.stack.push(NanVal::boolean(is_ok));
                }
                Op::IsErr => {
                    let v = self.pop();
                    let is_err = (v.0 & TAG_MASK) == TAG_ERR;
                    v.drop_rc();
                    self.stack.push(NanVal::boolean(is_err));
                }
                Op::UnwrapOkErr => {
                    let v = self.pop();
                    let inner = unsafe {
                        match v.as_heap_ref() {
                            HeapObj::OkVal(inner) | HeapObj::ErrVal(inner) => {
                                inner.clone_rc();
                                *inner
                            }
                            _ => return Err("unwrap on non-Ok/Err".to_string()),
                        }
                    };
                    v.drop_rc();
                    self.stack.push(inner);
                }
                Op::ListNew(n) => {
                    let start = self.stack.len() - n as usize;
                    let items: Vec<NanVal> = self.stack.drain(start..).collect();
                    self.stack.push(NanVal::heap_list(items));
                }
                Op::ListGetOrEnd(target) => {
                    let idx_val = self.pop();
                    let coll = self.pop();
                    unsafe {
                        match coll.as_heap_ref() {
                            HeapObj::List(items) if idx_val.is_number() => {
                                let i = idx_val.as_number() as usize;
                                if i < items.len() {
                                    let item = items[i];
                                    item.clone_rc();
                                    coll.drop_rc();
                                    self.stack.push(item);
                                } else {
                                    coll.drop_rc();
                                    ip = target as usize;
                                }
                            }
                            _ => {
                                coll.drop_rc();
                                return Err("foreach requires a list".to_string());
                            }
                        }
                    }
                }
            }
            // Write ip back (most opcodes just advance it; jumps/calls already set it)
            // We do this once at the end rather than in every branch
        }
    }
}

fn nanval_equal(a: NanVal, b: NanVal) -> bool {
    if a.is_number() && b.is_number() {
        (a.as_number() - b.as_number()).abs() < f64::EPSILON
    } else if a.0 == b.0 {
        // Identical bits: same nil, same bool, or same heap pointer
        true
    } else if a.is_string() && b.is_string() {
        unsafe {
            let sa = match a.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
            let sb = match b.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
            sa == sb
        }
    } else {
        false
    }
}

fn nanval_truthy(v: NanVal) -> bool {
    if v.is_number() {
        v.as_number() != 0.0
    } else {
        match v.0 {
            TAG_NIL | TAG_FALSE => false,
            TAG_TRUE => true,
            _ => unsafe {
                match v.as_heap_ref() {
                    HeapObj::Str(s) => !s.is_empty(),
                    HeapObj::List(l) => !l.is_empty(),
                    _ => true,
                }
            }
        }
    }
}

fn unpack_record_desc(desc: Value) -> Result<(String, Vec<String>), String> {
    match desc {
        Value::List(items) if items.len() == 2 => {
            let tn = match &items[0] {
                Value::Text(s) => s.clone(),
                _ => return Err("invalid record descriptor".to_string()),
            };
            let fns = unpack_string_list(&items[1])?;
            Ok((tn, fns))
        }
        _ => Err("invalid record descriptor".to_string()),
    }
}

fn unpack_string_list(val: &Value) -> Result<Vec<String>, String> {
    match val {
        Value::List(items) => {
            items.iter().map(|v| match v {
                Value::Text(s) => Ok(s.clone()),
                _ => Err("expected string in list".to_string()),
            }).collect()
        }
        _ => Err("expected list".to_string()),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn parse_program(source: &str) -> Program {
        let tokens: Vec<crate::lexer::Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        parser::parse(tokens).unwrap()
    }

    fn vm_run(source: &str, func: Option<&str>, args: Vec<Value>) -> Value {
        let prog = parse_program(source);
        compile_and_run(&prog, func, args).unwrap()
    }

    #[test]
    fn vm_tot() {
        let source = std::fs::read_to_string("examples/idea9-ultra-dense-short/01-simple-function.ilo").unwrap();
        let result = vm_run(
            &source,
            Some("tot"),
            vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)],
        );
        assert_eq!(result, Value::Number(6200.0));
    }

    #[test]
    fn vm_tot_different_args() {
        let source = "tot p:n q:n r:n>n;s=*p q;t=*s r;+s t";
        let result = vm_run(
            source,
            Some("tot"),
            vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)],
        );
        assert_eq!(result, Value::Number(30.0));
    }

    #[test]
    fn vm_cls_gold() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = vm_run(source, Some("cls"), vec![Value::Number(1000.0)]);
        assert_eq!(result, Value::Text("gold".to_string()));
    }

    #[test]
    fn vm_cls_silver() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = vm_run(source, Some("cls"), vec![Value::Number(500.0)]);
        assert_eq!(result, Value::Text("silver".to_string()));
    }

    #[test]
    fn vm_cls_bronze() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = vm_run(source, Some("cls"), vec![Value::Number(100.0)]);
        assert_eq!(result, Value::Text("bronze".to_string()));
    }

    #[test]
    fn vm_match_stmt() {
        let source = r#"f x:t>n;?x{"a":1;"b":2;_:0}"#;
        assert_eq!(
            vm_run(source, Some("f"), vec![Value::Text("a".to_string())]),
            Value::Number(1.0)
        );
        assert_eq!(
            vm_run(source, Some("f"), vec![Value::Text("b".to_string())]),
            Value::Number(2.0)
        );
        assert_eq!(
            vm_run(source, Some("f"), vec![Value::Text("z".to_string())]),
            Value::Number(0.0)
        );
    }

    #[test]
    fn vm_ok_err() {
        let source = "f x:n>R n t;~x";
        let result = vm_run(source, Some("f"), vec![Value::Number(42.0)]);
        assert_eq!(result, Value::Ok(Box::new(Value::Number(42.0))));
    }

    #[test]
    fn vm_err_constructor() {
        let source = r#"f x:n>R n t;!"bad""#;
        let result = vm_run(source, Some("f"), vec![Value::Number(0.0)]);
        assert_eq!(result, Value::Err(Box::new(Value::Text("bad".to_string()))));
    }

    #[test]
    fn vm_match_ok_err_patterns() {
        let source = r#"f x:R n t>n;?x{!e:0;~v:v}"#;
        let ok_result = vm_run(
            source,
            Some("f"),
            vec![Value::Ok(Box::new(Value::Number(42.0)))],
        );
        assert_eq!(ok_result, Value::Number(42.0));

        let err_result = vm_run(
            source,
            Some("f"),
            vec![Value::Err(Box::new(Value::Text("oops".to_string())))],
        );
        assert_eq!(err_result, Value::Number(0.0));
    }

    #[test]
    fn vm_negated_guard() {
        let source = r#"f x:b>t;!x{"nope"};"yes""#;
        assert_eq!(
            vm_run(source, Some("f"), vec![Value::Bool(false)]),
            Value::Text("nope".to_string())
        );
        assert_eq!(
            vm_run(source, Some("f"), vec![Value::Bool(true)]),
            Value::Text("yes".to_string())
        );
    }

    #[test]
    fn vm_record_and_field() {
        let source = "f x:n>n;r=point x:x y:10;r.y";
        let result = vm_run(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn vm_with_expr() {
        let source = "f>n;r=point x:1 y:2;r2=r with y:10;r2.y";
        let result = vm_run(source, Some("f"), vec![]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn vm_string_concat() {
        let source = r#"f a:t b:t>t;+a b"#;
        let result = vm_run(
            source,
            Some("f"),
            vec![Value::Text("hello ".to_string()), Value::Text("world".to_string())],
        );
        assert_eq!(result, Value::Text("hello world".to_string()));
    }

    #[test]
    fn vm_multi_function() {
        let source = "double x:n>n;*x 2\nf x:n>n;double x";
        let result = vm_run(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn vm_match_expr_in_let() {
        let source = r#"f x:t>n;y=?x{"a":1;"b":2;_:0};y"#;
        let result = vm_run(source, Some("f"), vec![Value::Text("b".to_string())]);
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn vm_default_first_function() {
        let source = "f>n;42";
        let result = vm_run(source, None, vec![]);
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn vm_division_by_zero() {
        let source = "f x:n>n;/x 0";
        let prog = parse_program(source);
        let result = compile_and_run(&prog, Some("f"), vec![Value::Number(10.0)]);
        assert!(result.is_err());
    }

    #[test]
    fn nanval_roundtrip() {
        // Number
        let v = Value::Number(42.5);
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);
        nv.drop_rc();

        // Negative zero
        let v = Value::Number(-0.0);
        let nv = NanVal::from_value(&v);
        assert!(nv.is_number());
        let rt = nv.to_value();
        match rt { Value::Number(n) => assert!(n.to_bits() == (-0.0f64).to_bits()), _ => panic!() }
        nv.drop_rc();

        // Infinity
        let v = Value::Number(f64::INFINITY);
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);
        nv.drop_rc();

        // Bool true
        let v = Value::Bool(true);
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);

        // Bool false
        let v = Value::Bool(false);
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);

        // Nil
        let v = Value::Nil;
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);

        // Text
        let v = Value::Text("hello".to_string());
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);
        nv.drop_rc();

        // Ok wrapping number
        let v = Value::Ok(Box::new(Value::Number(7.0)));
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);
        nv.drop_rc();

        // Err wrapping text
        let v = Value::Err(Box::new(Value::Text("bad".to_string())));
        let nv = NanVal::from_value(&v);
        assert_eq!(nv.to_value(), v);
        nv.drop_rc();
    }
}
