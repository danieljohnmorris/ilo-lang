use std::collections::HashMap;
use std::rc::Rc;
use crate::ast::*;
use crate::interpreter::Value;

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("no functions defined")]
    NoFunctionsDefined,
    #[error("undefined function: {name}")]
    UndefinedFunction { name: String },
    #[error("division by zero")]
    DivisionByZero,
    #[error("no field '{field}' on record")]
    FieldNotFound { field: String },
    #[error("unknown opcode: {op}")]
    UnknownOpcode { op: u8 },
    #[error("{0}")]
    Type(&'static str),
}

type VmResult<T> = Result<T, VmError>;

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("undefined variable: {name}")]
    UndefinedVariable { name: String },
    #[error("undefined function: {name}")]
    UndefinedFunction { name: String },
}


#[cfg(target_arch = "aarch64")]
pub(crate) mod jit_arm64;
#[cfg(feature = "cranelift")]
pub(crate) mod jit_cranelift;
#[cfg(feature = "llvm")]
pub(crate) mod jit_llvm;

// ── Register-based opcodes (32-bit packed instructions) ─────────────
//
// ABC mode:  [OP:8 | A:8 | B:8 | C:8]
// ABx mode:  [OP:8 | A:8 | Bx:16]  (Bx unsigned or signed)

// ABC mode — 3 registers
pub(crate) const OP_ADD: u8 = 0;
pub(crate) const OP_SUB: u8 = 1;
pub(crate) const OP_MUL: u8 = 2;
pub(crate) const OP_DIV: u8 = 3;
pub(crate) const OP_EQ: u8 = 4;
pub(crate) const OP_NE: u8 = 5;
pub(crate) const OP_GT: u8 = 6;
pub(crate) const OP_LT: u8 = 7;
pub(crate) const OP_GE: u8 = 8;
pub(crate) const OP_LE: u8 = 9;
pub(crate) const OP_MOVE: u8 = 10;
pub(crate) const OP_NOT: u8 = 11;
pub(crate) const OP_NEG: u8 = 12;
pub(crate) const OP_WRAPOK: u8 = 13;
pub(crate) const OP_WRAPERR: u8 = 14;
pub(crate) const OP_ISOK: u8 = 15;
pub(crate) const OP_ISERR: u8 = 16;
pub(crate) const OP_UNWRAP: u8 = 17;
pub(crate) const OP_RECFLD: u8 = 18;
pub(crate) const OP_LISTGET: u8 = 19;

// ABC mode — type-specialized (both operands known numeric, no type check)
pub(crate) const OP_ADD_NN: u8 = 29;
pub(crate) const OP_SUB_NN: u8 = 30;
pub(crate) const OP_MUL_NN: u8 = 31;
pub(crate) const OP_DIV_NN: u8 = 32;

// ABC mode — superinstructions: register op constant (C = constant pool index)
// These fuse LOADK + arithmetic into one dispatch, both operands known numeric
pub(crate) const OP_ADDK_N: u8 = 33;  // R[A] = R[B] + K[C]
pub(crate) const OP_SUBK_N: u8 = 34;  // R[A] = R[B] - K[C]
pub(crate) const OP_MULK_N: u8 = 35;  // R[A] = R[B] * K[C]
pub(crate) const OP_DIVK_N: u8 = 36;  // R[A] = R[B] / K[C]

// ABx mode — register + 16-bit operand
pub(crate) const OP_LOADK: u8 = 20;
pub(crate) const OP_JMP: u8 = 21;
pub(crate) const OP_JMPF: u8 = 22;
pub(crate) const OP_JMPT: u8 = 23;
pub(crate) const OP_CALL: u8 = 24;
pub(crate) const OP_RET: u8 = 25;
pub(crate) const OP_RECNEW: u8 = 26;
pub(crate) const OP_RECWITH: u8 = 27;
pub(crate) const OP_LISTNEW: u8 = 28;

// ── Instruction encoding ────────────────────────────────────────────

#[inline(always)]
fn encode_abc(op: u8, a: u8, b: u8, c: u8) -> u32 {
    (op as u32) << 24 | (a as u32) << 16 | (b as u32) << 8 | c as u32
}

#[inline(always)]
fn encode_abx(op: u8, a: u8, bx: u16) -> u32 {
    (op as u32) << 24 | (a as u32) << 16 | bx as u32
}

// ── Chunk ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<u32>,
    pub constants: Vec<Value>,
    pub param_count: u8,
    pub reg_count: u8,
}

impl Chunk {
    fn new(param_count: u8) -> Self {
        Chunk { code: Vec::new(), constants: Vec::new(), param_count, reg_count: param_count }
    }

    fn add_const(&mut self, val: Value) -> u16 {
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

    fn add_const_raw(&mut self, val: Value) -> u16 {
        let idx = self.constants.len() as u16;
        self.constants.push(val);
        idx
    }

    fn emit(&mut self, inst: u32) -> usize {
        let idx = self.code.len();
        self.code.push(inst);
        idx
    }

    fn patch_jump(&mut self, jump_pos: usize) {
        let target = self.code.len();
        let offset = (target as i32 - jump_pos as i32 - 1) as i16;
        let inst = self.code[jump_pos];
        self.code[jump_pos] = (inst & 0xFFFF0000) | (offset as u16 as u32);
    }
}

// ── Compiled program ─────────────────────────────────────────────────

pub struct CompiledProgram {
    pub chunks: Vec<Chunk>,
    pub func_names: Vec<String>,
    pub(crate) nan_constants: Vec<Vec<NanVal>>,
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

// ── Register Compiler ────────────────────────────────────────────────

struct RegCompiler {
    chunks: Vec<Chunk>,
    func_names: Vec<String>,
    current: Chunk,
    locals: Vec<(String, u8)>,
    next_reg: u8,
    max_reg: u8,
    reg_is_num: [bool; 256],  // track which registers are known numeric
    errors: Vec<CompileError>,
}

impl RegCompiler {
    fn new() -> Self {
        RegCompiler {
            chunks: Vec::new(),
            func_names: Vec::new(),
            current: Chunk::new(0),
            locals: Vec::new(),
            next_reg: 0,
            max_reg: 0,
            reg_is_num: [false; 256],
            errors: Vec::new(),
        }
    }

    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_reg;
        self.next_reg += 1;
        if self.next_reg > self.max_reg {
            self.max_reg = self.next_reg;
        }
        self.reg_is_num[r as usize] = false;
        r
    }


    fn resolve_local(&self, name: &str) -> Option<u8> {
        self.locals.iter().rev().find(|(n, _)| n == name).map(|(_, r)| *r)
    }

    fn add_local(&mut self, name: &str, reg: u8) {
        self.locals.push((name.to_string(), reg));
    }

    fn emit_abc(&mut self, op: u8, a: u8, b: u8, c: u8) -> usize {
        self.current.emit(encode_abc(op, a, b, c))
    }

    fn emit_abx(&mut self, op: u8, a: u8, bx: u16) -> usize {
        self.current.emit(encode_abx(op, a, bx))
    }

    fn emit_jmpf(&mut self, reg: u8) -> usize {
        self.emit_abx(OP_JMPF, reg, 0)
    }

    fn emit_jmpt(&mut self, reg: u8) -> usize {
        self.emit_abx(OP_JMPT, reg, 0)
    }

    fn emit_jmp_placeholder(&mut self) -> usize {
        self.emit_abx(OP_JMP, 0, 0)
    }

    fn emit_jump_to(&mut self, target: usize) {
        let pos = self.current.code.len();
        let offset = (target as i32 - pos as i32 - 1) as i16;
        self.emit_abx(OP_JMP, 0, offset as u16);
    }

    fn compile_program(mut self, program: &Program) -> Result<CompiledProgram, CompileError> {
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
                self.current = Chunk::new(params.len() as u8);
                self.locals.clear();
                self.next_reg = params.len() as u8;
                self.max_reg = self.next_reg;

                self.reg_is_num = [false; 256];
                for (i, p) in params.iter().enumerate() {
                    self.add_local(&p.name, i as u8);
                    if p.ty == Type::Number {
                        self.reg_is_num[i] = true;
                    }
                }

                let result = self.compile_body(body);

                let ret_reg = result.unwrap_or_else(|| {
                    let r = self.alloc_reg();
                    let ki = self.current.add_const(Value::Nil);
                    self.emit_abx(OP_LOADK, r, ki);
                    r
                });

                // Only emit RET if last instruction isn't already RET
                let last_is_ret = self.current.code.last()
                    .map(|inst| (inst >> 24) as u8 == OP_RET)
                    .unwrap_or(false);
                if !last_is_ret {
                    self.emit_abx(OP_RET, ret_reg, 0);
                }

                self.current.reg_count = self.max_reg;
                self.chunks.push(self.current.clone());
            } else {
                self.chunks.push(Chunk::new(0));
            }
        }

        if let Some(e) = self.errors.into_iter().next() {
            return Err(e);
        }
        Ok(CompiledProgram { chunks: self.chunks, func_names: self.func_names, nan_constants: Vec::new() })
    }

    fn compile_body(&mut self, stmts: &[Stmt]) -> Option<u8> {
        let saved_locals = self.locals.len();
        let mut result = None;
        for stmt in stmts {
            result = self.compile_stmt(stmt);
        }
        self.locals.truncate(saved_locals);
        result
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Option<u8> {
        match stmt {
            Stmt::Let { name, value } => {
                let reg = self.compile_expr(value);
                self.add_local(name, reg);
                None
            }

            Stmt::Guard { condition, negated, body } => {
                let saved_next = self.next_reg;
                let cond_reg = self.compile_expr(condition);
                let jump = if *negated {
                    self.emit_jmpt(cond_reg)
                } else {
                    self.emit_jmpf(cond_reg)
                };
                let body_result = self.compile_body(body);
                let ret_reg = body_result.unwrap_or_else(|| {
                    let r = self.alloc_reg();
                    let ki = self.current.add_const(Value::Nil);
                    self.emit_abx(OP_LOADK, r, ki);
                    r
                });
                self.emit_abx(OP_RET, ret_reg, 0);
                self.current.patch_jump(jump);
                self.next_reg = saved_next;
                None
            }

            Stmt::Match { subject, arms } => {
                let sub_reg = match subject {
                    Some(e) => self.compile_expr(e),
                    None => {
                        let r = self.alloc_reg();
                        let ki = self.current.add_const(Value::Nil);
                        self.emit_abx(OP_LOADK, r, ki);
                        r
                    }
                };
                let result_reg = self.alloc_reg();
                self.compile_match_arms(sub_reg, result_reg, arms);
                Some(result_reg)
            }

            Stmt::ForEach { binding, collection, body } => {
                let coll_reg = self.compile_expr(collection);
                self.add_local("__fe_coll", coll_reg);

                let idx_reg = self.alloc_reg();
                let zero_ki = self.current.add_const(Value::Number(0.0));
                self.emit_abx(OP_LOADK, idx_reg, zero_ki);
                self.add_local("__fe_idx", idx_reg);

                let last_reg = self.alloc_reg();
                let nil_ki = self.current.add_const(Value::Nil);
                self.emit_abx(OP_LOADK, last_reg, nil_ki);
                self.add_local("__fe_last", last_reg);

                let bind_reg = self.alloc_reg();
                self.emit_abx(OP_LOADK, bind_reg, nil_ki);
                self.add_local(binding, bind_reg);

                let one_reg = self.alloc_reg();
                let one_ki = self.current.add_const(Value::Number(1.0));
                self.emit_abx(OP_LOADK, one_reg, one_ki);

                // Loop top
                let loop_top = self.current.code.len();
                self.emit_abc(OP_LISTGET, bind_reg, coll_reg, idx_reg);
                let exit_jump = self.emit_jmp_placeholder();

                // Compile body
                let saved_locals = self.locals.len();
                let body_result = self.compile_body(body);
                self.locals.truncate(saved_locals);

                if let Some(br) = body_result {
                    if br != last_reg {
                        self.emit_abc(OP_MOVE, last_reg, br, 0);
                    }
                }

                // idx += 1
                self.emit_abc(OP_ADD, idx_reg, idx_reg, one_reg);

                // Jump back to loop top
                self.emit_jump_to(loop_top);

                // Exit
                self.current.patch_jump(exit_jump);

                Some(last_reg)
            }

            Stmt::Expr(expr) => {
                let reg = self.compile_expr(expr);
                Some(reg)
            }
        }
    }

    fn compile_match_arms(&mut self, sub_reg: u8, result_reg: u8, arms: &[MatchArm]) {
        let mut end_jumps = Vec::new();

        for arm in arms {
            let saved_next = self.next_reg;
            let saved_locals = self.locals.len();

            match &arm.pattern {
                Pattern::Wildcard => {
                    let body_result = self.compile_body(&arm.body);
                    if let Some(br) = body_result {
                        if br != result_reg {
                            self.emit_abc(OP_MOVE, result_reg, br, 0);
                        }
                    }
                    self.next_reg = saved_next;
                    self.locals.truncate(saved_locals);
                    for j in end_jumps {
                        self.current.patch_jump(j);
                    }
                    return;
                }

                Pattern::Ok(binding) => {
                    let test_reg = self.alloc_reg();
                    self.emit_abc(OP_ISOK, test_reg, sub_reg, 0);
                    let skip = self.emit_jmpf(test_reg);

                    if binding != "_" {
                        let bind_reg = self.alloc_reg();
                        self.emit_abc(OP_UNWRAP, bind_reg, sub_reg, 0);
                        self.add_local(binding, bind_reg);
                    }

                    let body_result = self.compile_body(&arm.body);
                    if let Some(br) = body_result {
                        if br != result_reg {
                            self.emit_abc(OP_MOVE, result_reg, br, 0);
                        }
                    }
                    end_jumps.push(self.emit_jmp_placeholder());
                    self.current.patch_jump(skip);
                }

                Pattern::Err(binding) => {
                    let test_reg = self.alloc_reg();
                    self.emit_abc(OP_ISERR, test_reg, sub_reg, 0);
                    let skip = self.emit_jmpf(test_reg);

                    if binding != "_" {
                        let bind_reg = self.alloc_reg();
                        self.emit_abc(OP_UNWRAP, bind_reg, sub_reg, 0);
                        self.add_local(binding, bind_reg);
                    }

                    let body_result = self.compile_body(&arm.body);
                    if let Some(br) = body_result {
                        if br != result_reg {
                            self.emit_abc(OP_MOVE, result_reg, br, 0);
                        }
                    }
                    end_jumps.push(self.emit_jmp_placeholder());
                    self.current.patch_jump(skip);
                }

                Pattern::Literal(lit) => {
                    let val = match lit {
                        Literal::Number(n) => Value::Number(*n),
                        Literal::Text(s) => Value::Text(s.clone()),
                        Literal::Bool(b) => Value::Bool(*b),
                    };
                    let const_reg = self.alloc_reg();
                    let ki = self.current.add_const(val);
                    self.emit_abx(OP_LOADK, const_reg, ki);
                    let eq_reg = self.alloc_reg();
                    self.emit_abc(OP_EQ, eq_reg, sub_reg, const_reg);
                    let skip = self.emit_jmpf(eq_reg);

                    let body_result = self.compile_body(&arm.body);
                    if let Some(br) = body_result {
                        if br != result_reg {
                            self.emit_abc(OP_MOVE, result_reg, br, 0);
                        }
                    }
                    end_jumps.push(self.emit_jmp_placeholder());
                    self.current.patch_jump(skip);
                }
            }

            self.next_reg = saved_next;
            self.locals.truncate(saved_locals);
        }

        // No wildcard matched: default to nil
        let nil_ki = self.current.add_const(Value::Nil);
        self.emit_abx(OP_LOADK, result_reg, nil_ki);

        for j in end_jumps {
            self.current.patch_jump(j);
        }
    }

    /// Try to evaluate an expression at compile time. Returns Some(Value) if fully constant.
    fn try_const_fold(expr: &Expr) -> Option<Value> {
        match expr {
            Expr::Literal(lit) => Some(match lit {
                Literal::Number(n) => Value::Number(*n),
                Literal::Text(s) => Value::Text(s.clone()),
                Literal::Bool(b) => Value::Bool(*b),
            }),
            Expr::BinOp { op, left, right } => {
                let lv = Self::try_const_fold(left)?;
                let rv = Self::try_const_fold(right)?;
                match (&lv, &rv) {
                    (Value::Number(a), Value::Number(b)) => Some(match op {
                        BinOp::Add => Value::Number(a + b),
                        BinOp::Subtract => Value::Number(a - b),
                        BinOp::Multiply => Value::Number(a * b),
                        BinOp::Divide if *b != 0.0 => Value::Number(a / b),
                        BinOp::Equals => Value::Bool((a - b).abs() < f64::EPSILON),
                        BinOp::NotEquals => Value::Bool((a - b).abs() >= f64::EPSILON),
                        BinOp::GreaterThan => Value::Bool(a > b),
                        BinOp::LessThan => Value::Bool(a < b),
                        BinOp::GreaterOrEqual => Value::Bool(a >= b),
                        BinOp::LessOrEqual => Value::Bool(a <= b),
                        _ => return None,
                    }),
                    (Value::Text(a), Value::Text(b)) => match op {
                        BinOp::Add => Some(Value::Text(format!("{}{}", a, b))),
                        _ => None,
                    },
                    (Value::Bool(a), Value::Bool(b)) => match op {
                        BinOp::Equals => Some(Value::Bool(a == b)),
                        BinOp::NotEquals => Some(Value::Bool(a != b)),
                        _ => None,
                    },
                    _ => None,
                }
            }
            Expr::UnaryOp { op, operand } => {
                let v = Self::try_const_fold(operand)?;
                match (&v, op) {
                    (Value::Number(n), UnaryOp::Negate) => Some(Value::Number(-n)),
                    (Value::Bool(b), UnaryOp::Not) => Some(Value::Bool(!b)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> u8 {
        // Try constant folding for BinOp/UnaryOp expressions
        if matches!(expr, Expr::BinOp { .. } | Expr::UnaryOp { .. }) {
            if let Some(ref val) = Self::try_const_fold(expr) {
                let is_num = matches!(val, Value::Number(_));
                let reg = self.alloc_reg();
                let ki = self.current.add_const(val.clone());
                self.emit_abx(OP_LOADK, reg, ki);
                if is_num { self.reg_is_num[reg as usize] = true; }
                return reg;
            }
        }

        match expr {
            Expr::Literal(lit) => {
                let is_num = matches!(lit, Literal::Number(_));
                let val = match lit {
                    Literal::Number(n) => Value::Number(*n),
                    Literal::Text(s) => Value::Text(s.clone()),
                    Literal::Bool(b) => Value::Bool(*b),
                };
                let reg = self.alloc_reg();
                let ki = self.current.add_const(val);
                self.emit_abx(OP_LOADK, reg, ki);
                if is_num { self.reg_is_num[reg as usize] = true; }
                reg
            }

            Expr::Ref(name) => {
                if let Some(reg) = self.resolve_local(name) {
                    reg // FREE — no instruction needed!
                } else {
                    self.errors.push(CompileError::UndefinedVariable { name: name.clone() });
                    0 // dummy register; compile continues to surface more errors
                }
            }

            Expr::Field { object, field } => {
                let obj_reg = self.compile_expr(object);
                let ra = self.alloc_reg();
                let ki = self.current.add_const(Value::Text(field.clone()));
                self.emit_abc(OP_RECFLD, ra, obj_reg, ki as u8);
                ra
            }

            Expr::Call { function, args } => {
                let arg_regs: Vec<u8> = args.iter().map(|a| self.compile_expr(a)).collect();
                let func_idx = self.func_names.iter().position(|n| n == function)
                    .unwrap_or_else(|| {
                        self.errors.push(CompileError::UndefinedFunction { name: function.clone() });
                        0 // dummy index; compile continues to surface more errors
                    });

                let a = self.alloc_reg(); // result register
                // Reserve slots for args
                let args_base = self.next_reg;
                self.next_reg += args.len() as u8;
                if self.next_reg > self.max_reg {
                    self.max_reg = self.next_reg;
                }

                for (i, &arg_reg) in arg_regs.iter().enumerate() {
                    let target = args_base + i as u8;
                    if arg_reg != target {
                        self.emit_abc(OP_MOVE, target, arg_reg, 0);
                    }
                }

                let bx = ((func_idx as u16) << 8) | args.len() as u16;
                self.emit_abx(OP_CALL, a, bx);

                // After call, only the result register is live
                self.next_reg = a + 1;
                a
            }

            Expr::BinOp { op, left, right } => {
                // Try superinstructions: register op constant (right is number literal)
                let is_arith = matches!(op, BinOp::Add | BinOp::Subtract | BinOp::Multiply | BinOp::Divide);
                if is_arith {
                    if let Expr::Literal(Literal::Number(n)) = right.as_ref() {
                        let rb = self.compile_expr(left);
                        if self.reg_is_num[rb as usize] {
                            let ki = self.current.add_const(Value::Number(*n));
                            if ki <= 255 {
                                let ra = self.alloc_reg();
                                let opcode = match op {
                                    BinOp::Add => OP_ADDK_N,
                                    BinOp::Subtract => OP_SUBK_N,
                                    BinOp::Multiply => OP_MULK_N,
                                    BinOp::Divide => OP_DIVK_N,
                                    _ => unreachable!(),
                                };
                                self.emit_abc(opcode, ra, rb, ki as u8);
                                self.reg_is_num[ra as usize] = true;
                                return ra;
                            }
                        }
                    }
                    // Also handle constant on left (e.g., 2 * x → MULK x, 2)
                    // Only for commutative ops (Add, Multiply)
                    if matches!(op, BinOp::Add | BinOp::Multiply) {
                        if let Expr::Literal(Literal::Number(n)) = left.as_ref() {
                            let rc = self.compile_expr(right);
                            if self.reg_is_num[rc as usize] {
                                let ki = self.current.add_const(Value::Number(*n));
                                if ki <= 255 {
                                    let ra = self.alloc_reg();
                                    let opcode = match op {
                                        BinOp::Add => OP_ADDK_N,
                                        BinOp::Multiply => OP_MULK_N,
                                        _ => unreachable!(),
                                    };
                                    self.emit_abc(opcode, ra, rc, ki as u8);
                                    self.reg_is_num[ra as usize] = true;
                                    return ra;
                                }
                            }
                        }
                    }
                }

                let rb = self.compile_expr(left);
                let rc = self.compile_expr(right);
                let both_num = self.reg_is_num[rb as usize] && self.reg_is_num[rc as usize];

                // Use type-specialized opcodes when both operands are known numeric
                let (opcode, result_is_num) = match op {
                    BinOp::Add if both_num => (OP_ADD_NN, true),
                    BinOp::Subtract if both_num => (OP_SUB_NN, true),
                    BinOp::Multiply if both_num => (OP_MUL_NN, true),
                    BinOp::Divide if both_num => (OP_DIV_NN, true),
                    BinOp::Add => (OP_ADD, false),
                    BinOp::Subtract => (OP_SUB, false),
                    BinOp::Multiply => (OP_MUL, false),
                    BinOp::Divide => (OP_DIV, false),
                    BinOp::Equals => (OP_EQ, false),
                    BinOp::NotEquals => (OP_NE, false),
                    BinOp::GreaterThan => (OP_GT, false),
                    BinOp::LessThan => (OP_LT, false),
                    BinOp::GreaterOrEqual => (OP_GE, false),
                    BinOp::LessOrEqual => (OP_LE, false),
                };
                let ra = self.alloc_reg();
                self.emit_abc(opcode, ra, rb, rc);
                if result_is_num { self.reg_is_num[ra as usize] = true; }
                ra
            }

            Expr::UnaryOp { op, operand } => {
                let rb = self.compile_expr(operand);
                let ra = self.alloc_reg();
                let opcode = match op {
                    UnaryOp::Not => OP_NOT,
                    UnaryOp::Negate => OP_NEG,
                };
                self.emit_abc(opcode, ra, rb, 0);
                if *op == UnaryOp::Negate && self.reg_is_num[rb as usize] {
                    self.reg_is_num[ra as usize] = true;
                }
                ra
            }

            Expr::Ok(inner) => {
                let rb = self.compile_expr(inner);
                let ra = self.alloc_reg();
                self.emit_abc(OP_WRAPOK, ra, rb, 0);
                ra
            }

            Expr::Err(inner) => {
                let rb = self.compile_expr(inner);
                let ra = self.alloc_reg();
                self.emit_abc(OP_WRAPERR, ra, rb, 0);
                ra
            }

            Expr::List(items) => {
                let item_regs: Vec<u8> = items.iter().map(|item| self.compile_expr(item)).collect();

                let a = self.alloc_reg(); // result register
                // Reserve slots for items
                let items_base = self.next_reg;
                self.next_reg += items.len() as u8;
                if self.next_reg > self.max_reg {
                    self.max_reg = self.next_reg;
                }

                for (i, &item_reg) in item_regs.iter().enumerate() {
                    let target = items_base + i as u8;
                    if item_reg != target {
                        self.emit_abc(OP_MOVE, target, item_reg, 0);
                    }
                }

                self.emit_abx(OP_LISTNEW, a, items.len() as u16);
                a
            }

            Expr::Record { type_name, fields } => {
                let field_regs: Vec<u8> = fields.iter()
                    .map(|(_, val_expr)| self.compile_expr(val_expr))
                    .collect();

                let desc = Value::List(vec![
                    Value::Text(type_name.clone()),
                    Value::List(fields.iter().map(|(n, _)| Value::Text(n.clone())).collect()),
                ]);
                let desc_idx = self.current.add_const_raw(desc);

                let a = self.alloc_reg(); // result register
                let fields_base = self.next_reg;
                self.next_reg += fields.len() as u8;
                if self.next_reg > self.max_reg {
                    self.max_reg = self.next_reg;
                }

                for (i, &field_reg) in field_regs.iter().enumerate() {
                    let target = fields_base + i as u8;
                    if field_reg != target {
                        self.emit_abc(OP_MOVE, target, field_reg, 0);
                    }
                }

                let bx = ((desc_idx as u16) << 8) | fields.len() as u16;
                self.emit_abx(OP_RECNEW, a, bx);
                a
            }

            Expr::Match { subject, arms } => {
                let sub_reg = match subject {
                    Some(e) => self.compile_expr(e),
                    None => {
                        let r = self.alloc_reg();
                        let ki = self.current.add_const(Value::Nil);
                        self.emit_abx(OP_LOADK, r, ki);
                        r
                    }
                };
                let result_reg = self.alloc_reg();
                self.compile_match_arms(sub_reg, result_reg, arms);
                result_reg
            }

            Expr::With { object, updates } => {
                let obj_reg = self.compile_expr(object);
                let update_regs: Vec<u8> = updates.iter()
                    .map(|(_, val_expr)| self.compile_expr(val_expr))
                    .collect();

                let names = Value::List(
                    updates.iter().map(|(n, _)| Value::Text(n.clone())).collect()
                );
                let names_idx = self.current.add_const_raw(names);

                let a = self.alloc_reg(); // result register
                let updates_base = self.next_reg;
                self.next_reg += updates.len() as u8;
                if self.next_reg > self.max_reg {
                    self.max_reg = self.next_reg;
                }

                // Move object into result slot
                if obj_reg != a {
                    self.emit_abc(OP_MOVE, a, obj_reg, 0);
                }

                // Move update values into consecutive slots
                for (i, &val_reg) in update_regs.iter().enumerate() {
                    let target = updates_base + i as u8;
                    if val_reg != target {
                        self.emit_abc(OP_MOVE, target, val_reg, 0);
                    }
                }

                let bx = ((names_idx as u16) << 8) | updates.len() as u16;
                self.emit_abx(OP_RECWITH, a, bx);
                a
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
pub(crate) struct NanVal(u64);

impl NanVal {
    #[inline]
    pub(crate) fn number(n: f64) -> Self {
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
    pub(crate) fn is_number(self) -> bool {
        (self.0 & QNAN) != QNAN
    }

    #[inline]
    pub(crate) fn as_number(self) -> f64 {
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

    /// # Safety
    /// Caller must ensure `self` was created via one of the `heap_*` constructors
    /// (i.e. `is_heap()` returns true) and that the underlying `Rc<HeapObj>` is
    /// still alive — i.e. the strong count has not reached zero. The returned
    /// reference borrows the heap allocation; its lifetime is bounded by the
    /// caller's knowledge of the RC lifetime, not by `'a`. Callers must not
    /// hold the reference across any operation that could decrement the RC to zero.
    #[inline]
    unsafe fn as_heap_ref<'a>(self) -> &'a HeapObj {
        let ptr = (self.0 & PTR_MASK) as *const HeapObj;
        // SAFETY: pointer was produced by Rc::into_raw in a heap_* constructor.
        // Caller guarantees is_heap() and the Rc is still live.
        unsafe { &*ptr }
    }

    #[inline(always)]
    fn clone_rc(self) {
        if self.is_heap() {
            let ptr = (self.0 & PTR_MASK) as *const HeapObj;
            // SAFETY: is_heap() guarantees this pointer was produced by Rc::into_raw
            // and the RC count is at least 1 (we hold a NanVal that represents it).
            unsafe { Rc::increment_strong_count(ptr); }
        }
    }

    #[inline(always)]
    fn drop_rc(self) {
        if self.is_heap() {
            let ptr = (self.0 & PTR_MASK) as *const HeapObj;
            // SAFETY: is_heap() guarantees this pointer was produced by Rc::into_raw.
            // Decrementing mirrors every clone_rc call; the VM is responsible for
            // pairing increments and decrements correctly.
            unsafe { Rc::decrement_strong_count(ptr); }
        }
    }

    pub(crate) fn from_value(val: &Value) -> Self {
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

    pub(crate) fn to_value(self) -> Value {
        if self.is_number() {
            return Value::Number(self.as_number());
        }
        match self.0 {
            TAG_NIL => Value::Nil,
            TAG_TRUE => Value::Bool(true),
            TAG_FALSE => Value::Bool(false),
            _ => unsafe {
                // SAFETY: Not a number, nil, true, or false — must be a heap-tagged
                // pointer. The NanVal was created by a heap_* constructor so the
                // Rc is still live (we own this NanVal value).
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

pub fn compile(program: &Program) -> Result<CompiledProgram, CompileError> {
    let mut prog = RegCompiler::new().compile_program(program)?;
    prog.nan_constants = prog.chunks.iter()
        .map(|chunk| chunk.constants.iter().map(|v| NanVal::from_value(v)).collect())
        .collect();
    Ok(prog)
}

pub fn run(compiled: &CompiledProgram, func_name: Option<&str>, args: Vec<Value>) -> VmResult<Value> {
    let target = match func_name {
        Some(name) => name.to_string(),
        None => compiled.func_names.first().ok_or(VmError::NoFunctionsDefined)?.clone(),
    };
    let func_idx = compiled.func_index(&target)
        .ok_or_else(|| VmError::UndefinedFunction { name: target })?;
    VM::new(compiled).call(func_idx, args)
}

#[cfg(test)]
pub fn compile_and_run(program: &Program, func_name: Option<&str>, args: Vec<Value>) -> Result<Value, Box<dyn std::error::Error>> {
    let compiled = compile(program)?;
    Ok(run(&compiled, func_name, args)?)
}

/// Reusable VM handle — avoids re-allocating stack/frames per call.
pub struct VmState<'a> {
    vm: VM<'a>,
}

impl<'a> VmState<'a> {
    pub fn new(compiled: &'a CompiledProgram) -> Self {
        VmState { vm: VM::new(compiled) }
    }

    pub fn call(&mut self, func_name: &str, args: Vec<Value>) -> VmResult<Value> {
        for v in self.vm.stack.drain(..) {
            v.drop_rc();
        }
        self.vm.frames.clear();

        let func_idx = self.vm.program.func_index(func_name)
            .ok_or_else(|| VmError::UndefinedFunction { name: func_name.to_string() })?;
        let nan_args: Vec<NanVal> = args.iter().map(|v| NanVal::from_value(v)).collect();
        self.vm.setup_call(func_idx, nan_args, 0);
        self.vm.execute()
    }
}

struct CallFrame {
    chunk_idx: u16,
    ip: usize,
    stack_base: usize,
    result_reg: u8,
}

struct VM<'a> {
    program: &'a CompiledProgram,
    stack: Vec<NanVal>,
    frames: Vec<CallFrame>,
}

impl<'a> Drop for VM<'a> {
    fn drop(&mut self) {
        for v in &self.stack {
            v.drop_rc();
        }
    }
}

impl<'a> VM<'a> {
    fn new(program: &'a CompiledProgram) -> Self {
        VM { program, stack: Vec::with_capacity(256), frames: Vec::with_capacity(64) }
    }

    fn setup_call(&mut self, func_idx: u16, args: Vec<NanVal>, result_reg: u8) {
        let chunk = &self.program.chunks[func_idx as usize];
        let stack_base = self.stack.len();

        for arg in args {
            self.stack.push(arg);
        }

        // Pre-allocate register slots
        while self.stack.len() < stack_base + chunk.reg_count as usize {
            self.stack.push(NanVal::nil());
        }

        self.frames.push(CallFrame {
            chunk_idx: func_idx,
            ip: 0,
            stack_base,
            result_reg,
        });
    }

    fn call(&mut self, func_idx: u16, args: Vec<Value>) -> VmResult<Value> {
        let nan_args: Vec<NanVal> = args.iter().map(|v| NanVal::from_value(v)).collect();
        self.setup_call(func_idx, nan_args, 0);
        self.execute()
    }

    fn execute(&mut self) -> VmResult<Value> {
        // SAFETY: execute() is only called from call() after setup_call() has pushed
        // a frame, so frames is non-empty.
        let frame = unsafe { self.frames.last().unwrap_unchecked() };
        let mut ci = frame.chunk_idx as usize;
        let mut ip = frame.ip;
        let mut base = frame.stack_base;

        loop {
            // SAFETY: ci is always set from frame.chunk_idx, which is a valid index
            // assigned by the compiler (func_idx < chunks.len()). nan_constants has
            // the same length as chunks (built together in compile()).
            let code = unsafe { &self.program.chunks.get_unchecked(ci).code };
            let nan_consts = unsafe { self.program.nan_constants.get_unchecked(ci) };

            if ip >= code.len() {
                // Safety: should not happen with explicit RET, but handle gracefully
                let result = NanVal::nil();
                for i in base..self.stack.len() {
                    self.stack[i].drop_rc();
                }
                self.stack.truncate(base);
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(result.to_value());
                }
                // SAFETY: we just checked !self.frames.is_empty().
                let f = unsafe { self.frames.last().unwrap_unchecked() };
                let target = f.stack_base + self.frames.last().map(|f| f.result_reg).unwrap_or(0) as usize;
                ci = f.chunk_idx as usize;
                ip = f.ip;
                base = f.stack_base;
                if target < self.stack.len() {
                    self.stack[target].drop_rc();
                    self.stack[target] = result;
                }
                continue;
            }

            // SAFETY: ip < code.len() was verified by the bounds check above.
            let inst = unsafe { *code.get_unchecked(ip) };
            ip += 1;
            let op = (inst >> 24) as u8;

            // Macro for register access in hot paths.
            // SAFETY invariant for reg!/reg_set!: the compiler assigns each
            // function a reg_count and stack slots are pre-allocated in setup_call.
            // Register indices in instructions are always < reg_count, so
            // base + reg_idx < stack.len() is guaranteed by construction.
            macro_rules! reg {
                ($idx:expr) => {
                    // SAFETY: $idx = base + encoded register, within pre-allocated slots.
                    unsafe { *self.stack.get_unchecked($idx) }
                }
            }
            macro_rules! reg_set {
                ($idx:expr, $val:expr) => {
                    // SAFETY: same bounds as reg!; using as_mut_ptr().add() to avoid
                    // aliasing a mutable reference to the stack while it may be read.
                    unsafe {
                        let slot = self.stack.as_mut_ptr().add($idx);
                        (*slot).drop_rc();
                        *slot = $val;
                    }
                }
            }

            match op {
                OP_ADD => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::number(bv.as_number() + cv.as_number()));
                    } else if bv.is_string() && cv.is_string() {
                        let result = unsafe {
                            // SAFETY: is_string() confirmed both are heap-tagged string
                            // pointers with live RC counts (loaded from valid registers).
                            let sb = match bv.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
                            let sc = match cv.as_heap_ref() { HeapObj::Str(s) => s, _ => unreachable!() };
                            NanVal::heap_string(format!("{}{}", sb, sc))
                        };
                        reg_set!(a, result);
                    } else {
                        return Err(VmError::Type("cannot add non-matching types"));
                    }
                }
                OP_SUB => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::number(bv.as_number() - cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot subtract non-numbers"));
                    }
                }
                OP_MUL => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::number(bv.as_number() * cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot multiply non-numbers"));
                    }
                }
                OP_DIV => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        let dv = cv.as_number();
                        if dv == 0.0 {
                            return Err(VmError::DivisionByZero);
                        }
                        reg_set!(a, NanVal::number(bv.as_number() / dv));
                    } else {
                        return Err(VmError::Type("cannot divide non-numbers"));
                    }
                }
                OP_EQ => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let eq = nanval_equal(reg!(b), reg!(c));
                    reg_set!(a, NanVal::boolean(eq));
                }
                OP_NE => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let eq = nanval_equal(reg!(b), reg!(c));
                    reg_set!(a, NanVal::boolean(!eq));
                }
                OP_GT => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::boolean(bv.as_number() > cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot compare > non-numbers"));
                    }
                }
                OP_LT => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::boolean(bv.as_number() < cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot compare < non-numbers"));
                    }
                }
                OP_GE => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::boolean(bv.as_number() >= cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot compare >= non-numbers"));
                    }
                }
                OP_LE => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let bv = reg!(b);
                    let cv = reg!(c);
                    if bv.is_number() && cv.is_number() {
                        reg_set!(a, NanVal::boolean(bv.as_number() <= cv.as_number()));
                    } else {
                        return Err(VmError::Type("cannot compare <= non-numbers"));
                    }
                }
                OP_MOVE => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let v = reg!(b);
                    if !v.is_number() { v.clone_rc(); }
                    reg_set!(a, v);
                }
                OP_NOT => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let t = nanval_truthy(reg!(b));
                    reg_set!(a, NanVal::boolean(!t));
                }
                OP_NEG => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let v = reg!(b);
                    if v.is_number() {
                        reg_set!(a, NanVal::number(-v.as_number()));
                    } else {
                        return Err(VmError::Type("cannot negate non-number"));
                    }
                }
                OP_WRAPOK => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let v = reg!(b);
                    if !v.is_number() { v.clone_rc(); }
                    reg_set!(a, NanVal::heap_ok(v));
                }
                OP_WRAPERR => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let v = reg!(b);
                    if !v.is_number() { v.clone_rc(); }
                    reg_set!(a, NanVal::heap_err(v));
                }
                OP_ISOK => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let is_ok = (reg!(b).0 & TAG_MASK) == TAG_OK;
                    reg_set!(a, NanVal::boolean(is_ok));
                }
                OP_ISERR => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let is_err = (reg!(b).0 & TAG_MASK) == TAG_ERR;
                    reg_set!(a, NanVal::boolean(is_err));
                }
                OP_UNWRAP => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let v = reg!(b);
                    let inner = unsafe {
                        // SAFETY: v comes from a valid register. The Ok/Err check below
                        // returns Err if the tag is wrong, so as_heap_ref is only reached
                        // when the value is a heap-allocated Ok or Err wrapper.
                        match v.as_heap_ref() {
                            HeapObj::OkVal(inner) | HeapObj::ErrVal(inner) => {
                                inner.clone_rc();
                                *inner
                            }
                            _ => return Err(VmError::Type("unwrap on non-Ok/Err")),
                        }
                    };
                    reg_set!(a, inner);
                }
                OP_RECFLD => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize;
                    // SAFETY: ci is a valid chunk index (same invariant as loop header).
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let field_name = match &chunk.constants[c] {
                        Value::Text(s) => s.as_str(),
                        _ => return Err(VmError::Type("RecordField expects string constant")),
                    };
                    let record = reg!(b);
                    let field_val = unsafe {
                        // SAFETY: record comes from a valid register; the non-record
                        // case returns Err before any pointer dereference.
                        match record.as_heap_ref() {
                            HeapObj::Record { fields, .. } => {
                                match fields.get(field_name) {
                                    Some(&val) => {
                                        val.clone_rc();
                                        val
                                    }
                                    None => return Err(VmError::FieldNotFound { field: field_name.to_string() }),
                                }
                            }
                            _ => return Err(VmError::Type("field access on non-record")),
                        }
                    };
                    reg_set!(a, field_val);
                }
                OP_LISTGET => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    let list = reg!(b);
                    let idx_val = reg!(c);
                    if idx_val.is_number() {
                        unsafe {
                            // SAFETY: list comes from a valid register; non-list case
                            // returns Err before any pointer dereference.
                            match list.as_heap_ref() {
                                HeapObj::List(items) => {
                                    let i = idx_val.as_number() as usize;
                                    if i < items.len() {
                                        let item = items[i];
                                        item.clone_rc();
                                        reg_set!(a, item);
                                        ip += 1; // skip the following JMP (stay in loop)
                                    }
                                    // else: fall through to JMP exit
                                }
                                _ => return Err(VmError::Type("foreach requires a list")),
                            }
                        }
                    } else {
                        return Err(VmError::Type("list index must be a number"));
                    }
                }
                OP_LOADK => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let bx = (inst & 0xFFFF) as usize;
                    // SAFETY: bx is the constant pool index encoded in the instruction;
                    // the compiler only emits indices < constants.len().
                    let v = unsafe { *nan_consts.get_unchecked(bx) };
                    if !v.is_number() { v.clone_rc(); }
                    reg_set!(a, v);
                }
                OP_JMP => {
                    let sbx = (inst & 0xFFFF) as i16;
                    ip = (ip as isize + sbx as isize) as usize;
                }
                OP_JMPF => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let sbx = (inst & 0xFFFF) as i16;
                    if !nanval_truthy(reg!(a)) {
                        ip = (ip as isize + sbx as isize) as usize;
                    }
                }
                OP_JMPT => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let sbx = (inst & 0xFFFF) as i16;
                    if nanval_truthy(reg!(a)) {
                        ip = (ip as isize + sbx as isize) as usize;
                    }
                }
                OP_CALL => {
                    let a = ((inst >> 16) & 0xFF) as u8;
                    let bx = (inst & 0xFFFF) as usize;
                    let func_idx = (bx >> 8) as u16;
                    let n_args = bx & 0xFF;

                    // SAFETY: frames is non-empty while execute() is running.
                    unsafe { self.frames.last_mut().unwrap_unchecked() }.ip = ip;

                    let mut args = Vec::with_capacity(n_args);
                    for i in 0..n_args {
                        let v = reg!(base + a as usize + 1 + i);
                        if !v.is_number() { v.clone_rc(); }
                        args.push(v);
                    }

                    self.setup_call(func_idx, args, a);

                    // SAFETY: setup_call just pushed a new frame above.
                    let f = unsafe { self.frames.last().unwrap_unchecked() };
                    ci = f.chunk_idx as usize;
                    ip = f.ip;
                    base = f.stack_base;
                }
                OP_RET => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let result = reg!(a);
                    if !result.is_number() { result.clone_rc(); }

                    // SAFETY: frames is non-empty while execute() is running.
                    let result_reg = unsafe { self.frames.last().unwrap_unchecked() }.result_reg;

                    for i in base..self.stack.len() {
                        // SAFETY: i is in range base..self.stack.len() by loop bounds.
                        unsafe { self.stack.get_unchecked(i) }.drop_rc();
                    }
                    self.stack.truncate(base);
                    self.frames.pop();

                    if self.frames.is_empty() {
                        let val = result.to_value();
                        result.drop_rc();
                        return Ok(val);
                    }

                    // SAFETY: we just checked !self.frames.is_empty().
                    let f = unsafe { self.frames.last().unwrap_unchecked() };
                    ci = f.chunk_idx as usize;
                    ip = f.ip;
                    base = f.stack_base;

                    // Store result in caller's register
                    reg_set!(base + result_reg as usize, result);
                }
                OP_RECNEW => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let bx = (inst & 0xFFFF) as usize;
                    let desc_idx = bx >> 8;
                    let n_fields = bx & 0xFF;

                    // SAFETY: ci is a valid chunk index (same invariant as loop header).
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let desc = chunk.constants[desc_idx].clone();
                    let (type_name, field_names) = unpack_record_desc(desc)?;

                    let mut fields = HashMap::new();
                    for (i, name) in field_names.into_iter().enumerate() {
                        let v = reg!(a + 1 + i);
                        v.clone_rc();
                        fields.insert(name, v);
                    }

                    reg_set!(a, NanVal::heap_record(type_name, fields));
                    let _ = n_fields;
                }
                OP_RECWITH => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let bx = (inst & 0xFFFF) as usize;
                    let names_idx = bx >> 8;
                    let n_updates = bx & 0xFF;

                    // SAFETY: ci is a valid chunk index (same invariant as loop header).
                    let chunk = unsafe { self.program.chunks.get_unchecked(ci) };
                    let field_names = unpack_string_list(&chunk.constants[names_idx])?;

                    let old_record = reg!(a);
                    let new_record = unsafe {
                        // SAFETY: old_record comes from a valid register; the non-record
                        // case returns Err before any pointer dereference.
                        match old_record.as_heap_ref() {
                            HeapObj::Record { type_name, fields } => {
                                let mut new_fields = HashMap::new();
                                for (k, v) in fields {
                                    v.clone_rc();
                                    new_fields.insert(k.clone(), *v);
                                }
                                for (i, name) in field_names.into_iter().enumerate() {
                                    let val = reg!(a + 1 + i);
                                    val.clone_rc();
                                    if let Some(old_val) = new_fields.insert(name, val) {
                                        old_val.drop_rc();
                                    }
                                }
                                NanVal::heap_record(type_name.clone(), new_fields)
                            }
                            _ => return Err(VmError::Type("'with' requires a record")),
                        }
                    };
                    reg_set!(a, new_record);
                    let _ = n_updates;
                }
                OP_LISTNEW => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let n = (inst & 0xFFFF) as usize;
                    let mut items = Vec::with_capacity(n);
                    for i in 0..n {
                        let v = reg!(a + 1 + i);
                        v.clone_rc();
                        items.push(v);
                    }
                    reg_set!(a, NanVal::heap_list(items));
                }
                OP_ADDK_N => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize;
                    // SAFETY: c is a constant pool index emitted by the compiler (< nan_consts.len()).
                    // a = base + reg, within pre-allocated stack slots.
                    let kv = unsafe { *nan_consts.get_unchecked(c) };
                    let result = NanVal::number(reg!(b).as_number() + kv.as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_SUBK_N => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize;
                    // SAFETY: same as OP_ADDK_N.
                    let kv = unsafe { *nan_consts.get_unchecked(c) };
                    let result = NanVal::number(reg!(b).as_number() - kv.as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_MULK_N => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize;
                    // SAFETY: same as OP_ADDK_N.
                    let kv = unsafe { *nan_consts.get_unchecked(c) };
                    let result = NanVal::number(reg!(b).as_number() * kv.as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_DIVK_N => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize;
                    // SAFETY: same as OP_ADDK_N.
                    let kv = unsafe { *nan_consts.get_unchecked(c) };
                    let dv = kv.as_number();
                    if dv == 0.0 {
                        return Err(VmError::DivisionByZero);
                    }
                    let result = NanVal::number(reg!(b).as_number() / dv);
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_ADD_NN => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    // SAFETY: a, b, c are all base + register offsets within pre-allocated stack slots.
                    let result = NanVal::number(reg!(b).as_number() + reg!(c).as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_SUB_NN => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    // SAFETY: a, b, c are base + register offsets within pre-allocated stack slots.
                    let result = NanVal::number(reg!(b).as_number() - reg!(c).as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_MUL_NN => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    // SAFETY: same as OP_SUB_NN.
                    let result = NanVal::number(reg!(b).as_number() * reg!(c).as_number());
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                OP_DIV_NN => {
                    let a = ((inst >> 16) & 0xFF) as usize + base;
                    let b = ((inst >> 8) & 0xFF) as usize + base;
                    let c = (inst & 0xFF) as usize + base;
                    // SAFETY: same as OP_SUB_NN.
                    let dv = reg!(c).as_number();
                    if dv == 0.0 {
                        return Err(VmError::DivisionByZero);
                    }
                    let result = NanVal::number(reg!(b).as_number() / dv);
                    unsafe { *self.stack.as_mut_ptr().add(a) = result; }
                }
                _ => return Err(VmError::UnknownOpcode { op }),
            }
        }
    }
}

fn nanval_equal(a: NanVal, b: NanVal) -> bool {
    if a.is_number() && b.is_number() {
        (a.as_number() - b.as_number()).abs() < f64::EPSILON
    } else if a.0 == b.0 {
        true
    } else if a.is_string() && b.is_string() {
        unsafe {
            // SAFETY: is_string() confirmed both are live heap-allocated string Rc pointers.
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

fn unpack_record_desc(desc: Value) -> VmResult<(String, Vec<String>)> {
    match desc {
        Value::List(items) if items.len() == 2 => {
            let tn = match &items[0] {
                Value::Text(s) => s.clone(),
                _ => return Err(VmError::Type("invalid record descriptor")),
            };
            let fns = unpack_string_list(&items[1])?;
            Ok((tn, fns))
        }
        _ => Err(VmError::Type("invalid record descriptor")),
    }
}

fn unpack_string_list(val: &Value) -> VmResult<Vec<String>> {
    match val {
        Value::List(items) => {
            items.iter().map(|v| match v {
                Value::Text(s) => Ok(s.clone()),
                _ => Err(VmError::Type("expected string in list")),
            }).collect()
        }
        _ => Err(VmError::Type("expected list")),
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
        let source = std::fs::read_to_string("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo").unwrap();
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
