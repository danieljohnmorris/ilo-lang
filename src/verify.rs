use std::collections::HashMap;

use crate::ast::*;

/// Verifier's internal type representation.
/// Adds `Unknown` for cases where we can't infer — compatible with anything.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Number,
    Text,
    Bool,
    Nil,
    List(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    Named(String),
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Number => write!(f, "n"),
            Ty::Text => write!(f, "t"),
            Ty::Bool => write!(f, "b"),
            Ty::Nil => write!(f, "_"),
            Ty::List(inner) => write!(f, "L {inner}"),
            Ty::Result(ok, err) => write!(f, "R {ok} {err}"),
            Ty::Named(name) => write!(f, "{name}"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifyError {
    pub function: String,
    pub message: String,
    pub hint: Option<String>,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "verify: {} in '{}'", self.message, self.function)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}

struct FuncSig {
    params: Vec<(String, Ty)>,
    return_type: Ty,
}

struct TypeDef {
    fields: Vec<(String, Ty)>,
}

struct VerifyContext {
    functions: HashMap<String, FuncSig>,
    types: HashMap<String, TypeDef>,
    errors: Vec<VerifyError>,
}

type Scope = Vec<HashMap<String, Ty>>;

fn scope_lookup(scope: &Scope, name: &str) -> Option<Ty> {
    for frame in scope.iter().rev() {
        if let Some(ty) = frame.get(name) {
            return Some(ty.clone());
        }
    }
    None
}

fn scope_insert(scope: &mut Scope, name: String, ty: Ty) {
    if let Some(frame) = scope.last_mut() {
        frame.insert(name, ty);
    }
}

fn convert_type(ast_ty: &Type) -> Ty {
    match ast_ty {
        Type::Number => Ty::Number,
        Type::Text => Ty::Text,
        Type::Bool => Ty::Bool,
        Type::Nil => Ty::Nil,
        Type::List(inner) => Ty::List(Box::new(convert_type(inner))),
        Type::Result(ok, err) => Ty::Result(Box::new(convert_type(ok)), Box::new(convert_type(err))),
        Type::Named(name) => Ty::Named(name.clone()),
    }
}

/// Two types are compatible if either is Unknown, or they're structurally equal.
fn compatible(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        (Ty::Number, Ty::Number) => true,
        (Ty::Text, Ty::Text) => true,
        (Ty::Bool, Ty::Bool) => true,
        (Ty::Nil, Ty::Nil) => true,
        (Ty::List(a), Ty::List(b)) => compatible(a, b),
        (Ty::Result(ao, ae), Ty::Result(bo, be)) => compatible(ao, bo) && compatible(ae, be),
        (Ty::Named(a), Ty::Named(b)) => a == b,
        _ => false,
    }
}

fn closest_match<'a>(name: &str, candidates: impl Iterator<Item = &'a String>) -> Option<String> {
    let mut best: Option<(String, usize)> = None;
    for candidate in candidates {
        let dist = levenshtein(name, candidate);
        if dist <= 3 && best.as_ref().is_none_or(|(_, d)| dist < *d) {
            best = Some((candidate.clone(), dist));
        }
    }
    best.map(|(s, _)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) { row[0] = i; }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) { *val = j; }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

const BUILTINS: &[(&str, &[&str], &str)] = &[
    // (name, param_types, return_type_desc)
    // We use special strings to describe signatures
    ("len", &["list_or_text"], "n"),
    ("str", &["n"], "t"),
    ("num", &["t"], "R n t"),
    ("abs", &["n"], "n"),
    ("flr", &["n"], "n"),
    ("cel", &["n"], "n"),
    ("min", &["n", "n"], "n"),
    ("max", &["n", "n"], "n"),
];

fn builtin_arity(name: &str) -> Option<usize> {
    BUILTINS.iter().find(|(n, _, _)| *n == name).map(|(_, params, _)| params.len())
}

fn is_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|(n, _, _)| *n == name)
}

fn builtin_check_args(name: &str, arg_types: &[Ty], func_ctx: &str) -> (Ty, Vec<VerifyError>) {
    let mut errors = Vec::new();
    match name {
        "len" => {
            if let Some(arg) = arg_types.first() {
                match arg {
                    Ty::List(_) | Ty::Text | Ty::Unknown => {}
                    other => errors.push(VerifyError {
                        function: func_ctx.to_string(),
                        message: format!("'len' expects a list or text, got {other}"),
                        hint: None,
                    }),
                }
            }
            (Ty::Number, errors)
        }
        "str" => {
            if let Some(arg) = arg_types.first()
                && !compatible(arg, &Ty::Number)
            {
                errors.push(VerifyError {
                    function: func_ctx.to_string(),
                    message: format!("'str' expects n, got {arg}"),
                    hint: None,
                });
            }
            (Ty::Text, errors)
        }
        "num" => {
            if let Some(arg) = arg_types.first()
                && !compatible(arg, &Ty::Text)
            {
                errors.push(VerifyError {
                    function: func_ctx.to_string(),
                    message: format!("'num' expects t, got {arg}"),
                    hint: None,
                });
            }
            (Ty::Result(Box::new(Ty::Number), Box::new(Ty::Text)), errors)
        }
        "abs" | "flr" | "cel" => {
            if let Some(arg) = arg_types.first()
                && !compatible(arg, &Ty::Number)
            {
                errors.push(VerifyError {
                    function: func_ctx.to_string(),
                    message: format!("'{name}' expects n, got {arg}"),
                    hint: None,
                });
            }
            (Ty::Number, errors)
        }
        "min" | "max" => {
            for (i, arg) in arg_types.iter().enumerate() {
                if !compatible(arg, &Ty::Number) {
                    errors.push(VerifyError {
                        function: func_ctx.to_string(),
                        message: format!("'{name}' arg {} expects n, got {arg}", i + 1),
                        hint: None,
                    });
                }
            }
            (Ty::Number, errors)
        }
        _ => (Ty::Unknown, errors),
    }
}

impl VerifyContext {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            types: HashMap::new(),
            errors: Vec::new(),
        }
    }

    fn err(&mut self, function: &str, message: String, hint: Option<String>) {
        self.errors.push(VerifyError {
            function: function.to_string(),
            message,
            hint,
        });
    }

    /// Phase 1: collect all declarations, check for duplicates and undefined Named types.
    fn collect_declarations(&mut self, program: &Program) {
        // First pass: collect type names
        for decl in &program.declarations {
            if let Decl::TypeDef { name, fields, .. } = decl {
                if self.types.contains_key(name) {
                    self.err("<global>", format!("duplicate type definition '{name}'"), None);
                } else {
                    let fields: Vec<(String, Ty)> = fields
                        .iter()
                        .map(|p| (p.name.clone(), convert_type(&p.ty)))
                        .collect();
                    self.types.insert(name.clone(), TypeDef { fields });
                }
            }
        }

        // Second pass: collect functions and tools, validate Named types in signatures
        for decl in &program.declarations {
            match decl {
                Decl::Function { name, params, return_type, .. } => {
                    if self.functions.contains_key(name) {
                        self.err("<global>", format!("duplicate function definition '{name}'"), None);
                        continue;
                    }
                    let params: Vec<(String, Ty)> = params
                        .iter()
                        .map(|p| (p.name.clone(), convert_type(&p.ty)))
                        .collect();
                    let ret = convert_type(return_type);
                    self.validate_named_types_in_sig(name, &params, &ret);
                    self.functions.insert(name.clone(), FuncSig { params, return_type: ret });
                }
                Decl::Tool { name, params, return_type, .. } => {
                    if self.functions.contains_key(name) {
                        self.err("<global>", format!("duplicate definition '{name}' (tool conflicts with function)"), None);
                        continue;
                    }
                    let params: Vec<(String, Ty)> = params
                        .iter()
                        .map(|p| (p.name.clone(), convert_type(&p.ty)))
                        .collect();
                    let ret = convert_type(return_type);
                    self.validate_named_types_in_sig(name, &params, &ret);
                    self.functions.insert(name.clone(), FuncSig { params, return_type: ret });
                }
                Decl::TypeDef { .. } => {} // already handled
            }
        }

        // Validate Named types in type def fields
        for decl in &program.declarations {
            if let Decl::TypeDef { name, fields, .. } = decl {
                for field in fields {
                    self.validate_named_type_recursive(&convert_type(&field.ty), name);
                }
            }
        }
    }

    fn validate_named_types_in_sig(&mut self, func_name: &str, params: &[(String, Ty)], ret: &Ty) {
        for (_, ty) in params {
            self.validate_named_type_recursive(ty, func_name);
        }
        self.validate_named_type_recursive(ret, func_name);
    }

    fn validate_named_type_recursive(&mut self, ty: &Ty, ctx: &str) {
        match ty {
            Ty::Named(name) => {
                if !self.types.contains_key(name) {
                    let hint = closest_match(name, self.types.keys())
                        .map(|s| format!("did you mean '{s}'?"));
                    self.err(ctx, format!("undefined type '{name}'"), hint);
                }
            }
            Ty::List(inner) => self.validate_named_type_recursive(inner, ctx),
            Ty::Result(ok, err) => {
                self.validate_named_type_recursive(ok, ctx);
                self.validate_named_type_recursive(err, ctx);
            }
            _ => {}
        }
    }

    /// Phase 2: verify all function bodies.
    fn verify_bodies(&mut self, program: &Program) {
        for decl in &program.declarations {
            if let Decl::Function { name, params, return_type, body, .. } = decl {
                let mut scope: Scope = vec![HashMap::new()];
                for p in params {
                    scope_insert(&mut scope, p.name.clone(), convert_type(&p.ty));
                }

                let body_ty = self.verify_body(name, &mut scope, body);
                let expected = convert_type(return_type);
                if !compatible(&body_ty, &expected) {
                    self.err(
                        name,
                        format!("return type mismatch: expected {expected}, got {body_ty}"),
                        None,
                    );
                }
            }
        }
    }

    fn verify_body(&mut self, func: &str, scope: &mut Scope, stmts: &[Stmt]) -> Ty {
        let mut last_ty = Ty::Nil;
        for stmt in stmts {
            last_ty = self.verify_stmt(func, scope, stmt);
        }
        last_ty
    }

    fn verify_stmt(&mut self, func: &str, scope: &mut Scope, stmt: &Stmt) -> Ty {
        match stmt {
            Stmt::Let { name, value } => {
                let ty = self.infer_expr(func, scope, value);
                scope_insert(scope, name.clone(), ty);
                Ty::Nil
            }
            Stmt::Guard { condition, body, .. } => {
                let _ = self.infer_expr(func, scope, condition);
                scope.push(HashMap::new());
                let body_ty = self.verify_body(func, scope, body);
                scope.pop();
                // Guard returns its body type if it fires, but we can't know statically.
                // The "fallthrough" type is whatever comes next, so return body_ty
                // as a possibility but don't enforce it as the only path.
                body_ty
            }
            Stmt::Match { subject, arms } => {
                let subject_ty = match subject {
                    Some(expr) => self.infer_expr(func, scope, expr),
                    None => Ty::Nil,
                };
                let mut arm_ty = Ty::Unknown;
                for arm in arms {
                    scope.push(HashMap::new());
                    self.bind_pattern(func, scope, &arm.pattern, &subject_ty);
                    let body_ty = self.verify_body(func, scope, &arm.body);
                    if arm_ty == Ty::Unknown {
                        arm_ty = body_ty;
                    }
                    scope.pop();
                }
                self.check_match_exhaustiveness(func, &subject_ty, arms);
                arm_ty
            }
            Stmt::ForEach { binding, collection, body } => {
                let coll_ty = self.infer_expr(func, scope, collection);
                let elem_ty = match &coll_ty {
                    Ty::List(inner) => *inner.clone(),
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.err(func, format!("foreach expects a list, got {other}"), None);
                        Ty::Unknown
                    }
                };
                scope.push(HashMap::new());
                scope_insert(scope, binding.clone(), elem_ty);
                let body_ty = self.verify_body(func, scope, body);
                scope.pop();
                body_ty
            }
            Stmt::Expr(expr) => self.infer_expr(func, scope, expr),
        }
    }

    fn bind_pattern(&mut self, _func: &str, scope: &mut Scope, pattern: &Pattern, subject_ty: &Ty) {
        match pattern {
            Pattern::Ok(name) => {
                if name != "_" {
                    let ty = match subject_ty {
                        Ty::Result(ok, _) => *ok.clone(),
                        Ty::Unknown => Ty::Unknown,
                        _ => Ty::Unknown,
                    };
                    scope_insert(scope, name.clone(), ty);
                }
            }
            Pattern::Err(name) => {
                if name != "_" {
                    let ty = match subject_ty {
                        Ty::Result(_, err) => *err.clone(),
                        Ty::Unknown => Ty::Unknown,
                        _ => Ty::Unknown,
                    };
                    scope_insert(scope, name.clone(), ty);
                }
            }
            Pattern::Literal(_) | Pattern::Wildcard => {}
        }
    }

    fn infer_expr(&mut self, func: &str, scope: &mut Scope, expr: &Expr) -> Ty {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Number(_) => Ty::Number,
                Literal::Text(_) => Ty::Text,
                Literal::Bool(_) => Ty::Bool,
            },

            Expr::Ref(name) => {
                if let Some(ty) = scope_lookup(scope, name) {
                    ty
                } else {
                    let candidates: Vec<String> = scope.iter()
                        .flat_map(|frame| frame.keys().cloned())
                        .collect();
                    let hint = closest_match(name, candidates.iter())
                        .map(|s| format!("did you mean '{s}'?"));
                    self.err(func, format!("undefined variable '{name}'"), hint);
                    Ty::Unknown
                }
            }

            Expr::Call { function: callee, args } => {
                // Infer all arg types first
                let arg_types: Vec<Ty> = args.iter().map(|a| self.infer_expr(func, scope, a)).collect();

                if is_builtin(callee) {
                    // Check arity
                    let expected_arity = builtin_arity(callee).unwrap();
                    if args.len() != expected_arity {
                        self.err(
                            func,
                            format!("arity mismatch: '{callee}' expects {expected_arity} args, got {}", args.len()),
                            None,
                        );
                        return Ty::Unknown;
                    }
                    let (ret_ty, errors) = builtin_check_args(callee, &arg_types, func);
                    self.errors.extend(errors);
                    ret_ty
                } else if let Some(sig) = self.functions.get(callee) {
                    let sig_params = sig.params.clone();
                    let sig_ret = sig.return_type.clone();

                    if args.len() != sig_params.len() {
                        self.err(
                            func,
                            format!(
                                "arity mismatch: '{callee}' expects {} args, got {}",
                                sig_params.len(),
                                args.len()
                            ),
                            None,
                        );
                        return sig_ret;
                    }

                    for (i, ((param_name, param_ty), arg_ty)) in sig_params.iter().zip(arg_types.iter()).enumerate() {
                        if !compatible(param_ty, arg_ty) {
                            self.err(
                                func,
                                format!(
                                    "type mismatch: param '{}' of '{}' expects {}, got {}",
                                    param_name, callee, param_ty, arg_ty
                                ),
                                None,
                            );
                        }
                        let _ = i;
                    }

                    sig_ret
                } else {
                    let mut candidates: Vec<String> = self.functions.keys().cloned().collect();
                    for (n, _, _) in BUILTINS {
                        candidates.push(n.to_string());
                    }
                    let hint = closest_match(callee, candidates.iter())
                        .map(|s| format!("did you mean '{s}'?"));
                    self.err(
                        func,
                        format!("undefined function '{callee}' (called with {} args)", args.len()),
                        hint,
                    );
                    Ty::Unknown
                }
            }

            Expr::BinOp { op, left, right } => {
                let lt = self.infer_expr(func, scope, left);
                let rt = self.infer_expr(func, scope, right);
                self.check_binop(func, op, &lt, &rt)
            }

            Expr::UnaryOp { op, operand } => {
                let t = self.infer_expr(func, scope, operand);
                match op {
                    UnaryOp::Negate => {
                        if !compatible(&t, &Ty::Number) {
                            self.err(func, format!("negate expects n, got {t}"), None);
                        }
                        Ty::Number
                    }
                    UnaryOp::Not => {
                        // Not works on anything (truthiness)
                        Ty::Bool
                    }
                }
            }

            Expr::Ok(inner) => {
                let t = self.infer_expr(func, scope, inner);
                Ty::Result(Box::new(t), Box::new(Ty::Unknown))
            }

            Expr::Err(inner) => {
                let t = self.infer_expr(func, scope, inner);
                Ty::Result(Box::new(Ty::Unknown), Box::new(t))
            }

            Expr::List(items) => {
                if items.is_empty() {
                    Ty::List(Box::new(Ty::Unknown))
                } else {
                    let first_ty = self.infer_expr(func, scope, &items[0]);
                    // Infer remaining items but don't enforce homogeneity strictly
                    for item in &items[1..] {
                        let _ = self.infer_expr(func, scope, item);
                    }
                    Ty::List(Box::new(first_ty))
                }
            }

            Expr::Record { type_name, fields } => {
                if let Some(type_def) = self.types.get(type_name) {
                    let def_fields = type_def.fields.clone();
                    let provided: HashMap<&str, &Expr> = fields.iter().map(|(n, e)| (n.as_str(), e)).collect();

                    // Check for missing fields
                    for (fname, _) in &def_fields {
                        if !provided.contains_key(fname.as_str()) {
                            self.err(
                                func,
                                format!("missing field '{fname}' in record '{type_name}'"),
                                None,
                            );
                        }
                    }

                    // Check for extra fields
                    let def_field_names: Vec<&str> = def_fields.iter().map(|(n, _)| n.as_str()).collect();
                    for (fname, _) in fields {
                        if !def_field_names.contains(&fname.as_str()) {
                            self.err(
                                func,
                                format!("unknown field '{fname}' in record '{type_name}'"),
                                None,
                            );
                        }
                    }

                    // Check field types
                    for (fname, fty) in &def_fields {
                        if let Some(expr) = provided.get(fname.as_str()) {
                            let actual = self.infer_expr(func, scope, expr);
                            if !compatible(fty, &actual) {
                                self.err(
                                    func,
                                    format!("field '{fname}' of '{type_name}' expects {fty}, got {actual}"),
                                    None,
                                );
                            }
                        }
                    }

                    Ty::Named(type_name.clone())
                } else {
                    let hint = closest_match(type_name, self.types.keys())
                        .map(|s| format!("did you mean '{s}'?"));
                    self.err(func, format!("undefined type '{type_name}'"), hint);
                    Ty::Unknown
                }
            }

            Expr::Field { object, field } => {
                let obj_ty = self.infer_expr(func, scope, object);
                match &obj_ty {
                    Ty::Named(type_name) => {
                        if let Some(type_def) = self.types.get(type_name) {
                            if let Some((_, fty)) = type_def.fields.iter().find(|(n, _)| n == field) {
                                fty.clone()
                            } else {
                                self.err(
                                    func,
                                    format!("no field '{field}' on type '{type_name}'"),
                                    None,
                                );
                                Ty::Unknown
                            }
                        } else {
                            Ty::Unknown
                        }
                    }
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.err(func, format!("field access on non-record type {other}"), None);
                        Ty::Unknown
                    }
                }
            }

            Expr::Index { object, .. } => {
                let obj_ty = self.infer_expr(func, scope, object);
                match &obj_ty {
                    Ty::List(inner) => *inner.clone(),
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.err(func, format!("index access on non-list type {other}"), None);
                        Ty::Unknown
                    }
                }
            }

            Expr::Match { subject, arms } => {
                let subject_ty = match subject {
                    Some(expr) => self.infer_expr(func, scope, expr),
                    None => Ty::Nil,
                };
                let mut result_ty = Ty::Unknown;
                for arm in arms {
                    scope.push(HashMap::new());
                    self.bind_pattern(func, scope, &arm.pattern, &subject_ty);
                    let body_ty = self.verify_body(func, scope, &arm.body);
                    if result_ty == Ty::Unknown {
                        result_ty = body_ty;
                    }
                    scope.pop();
                }
                self.check_match_exhaustiveness(func, &subject_ty, arms);
                result_ty
            }

            Expr::With { object, updates } => {
                let obj_ty = self.infer_expr(func, scope, object);
                match &obj_ty {
                    Ty::Named(type_name) => {
                        if let Some(type_def) = self.types.get(type_name) {
                            let def_fields = type_def.fields.clone();
                            for (fname, expr) in updates {
                                if let Some((_, fty)) = def_fields.iter().find(|(n, _)| n == fname) {
                                    let actual = self.infer_expr(func, scope, expr);
                                    if !compatible(fty, &actual) {
                                        self.err(
                                            func,
                                            format!("'with' field '{fname}' of '{type_name}' expects {fty}, got {actual}"),
                                            None,
                                        );
                                    }
                                } else {
                                    self.err(
                                        func,
                                        format!("unknown field '{fname}' in 'with' on '{type_name}'"),
                                        None,
                                    );
                                }
                            }
                        }
                        obj_ty
                    }
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.err(func, format!("'with' on non-record type {other}"), None);
                        Ty::Unknown
                    }
                }
            }
        }
    }

    fn check_binop(&mut self, func: &str, op: &BinOp, lt: &Ty, rt: &Ty) -> Ty {
        match op {
            BinOp::Add => {
                // Number+Number, Text+Text, List+List
                match (lt, rt) {
                    (Ty::Number, Ty::Number) => Ty::Number,
                    (Ty::Text, Ty::Text) => Ty::Text,
                    (Ty::List(a), Ty::List(_)) => Ty::List(a.clone()),
                    (Ty::Unknown, _) | (_, Ty::Unknown) => Ty::Unknown,
                    _ => {
                        self.err(func, format!("'+' expects matching n, t, or L types, got {lt} and {rt}"), None);
                        Ty::Unknown
                    }
                }
            }
            BinOp::Subtract | BinOp::Multiply | BinOp::Divide => {
                if !compatible(lt, &Ty::Number) || !compatible(rt, &Ty::Number) {
                    let sym = match op { BinOp::Subtract => "-", BinOp::Multiply => "*", _ => "/" };
                    self.err(func, format!("'{sym}' expects n and n, got {lt} and {rt}"), None);
                }
                Ty::Number
            }
            BinOp::GreaterThan | BinOp::LessThan | BinOp::GreaterOrEqual | BinOp::LessOrEqual => {
                match (lt, rt) {
                    (Ty::Number, Ty::Number) | (Ty::Text, Ty::Text) => {}
                    (Ty::Unknown, _) | (_, Ty::Unknown) => {}
                    _ => {
                        self.err(func, format!("comparison expects matching n or t, got {lt} and {rt}"), None);
                    }
                }
                Ty::Bool
            }
            BinOp::Equals | BinOp::NotEquals => Ty::Bool,
            BinOp::And | BinOp::Or => Ty::Bool,
            BinOp::Append => {
                // List(T) += T → List(T)
                match lt {
                    Ty::List(inner) => {
                        if !compatible(inner, rt) {
                            self.err(func, format!("'+=' list element type {inner} doesn't match appended {rt}"), None);
                        }
                        lt.clone()
                    }
                    Ty::Unknown => Ty::Unknown,
                    _ => {
                        self.err(func, format!("'+=' expects a list on the left, got {lt}"), None);
                        Ty::Unknown
                    }
                }
            }
        }
    }

    fn check_match_exhaustiveness(&mut self, func: &str, subject_ty: &Ty, arms: &[MatchArm]) {
        let has_wildcard = arms.iter().any(|a| matches!(a.pattern, Pattern::Wildcard));
        if has_wildcard {
            return;
        }

        match subject_ty {
            Ty::Result(_, _) => {
                let has_ok = arms.iter().any(|a| matches!(a.pattern, Pattern::Ok(_)));
                let has_err = arms.iter().any(|a| matches!(a.pattern, Pattern::Err(_)));
                if !has_ok || !has_err {
                    let missing: Vec<&str> = [
                        if !has_ok { Some("~") } else { None },
                        if !has_err { Some("^") } else { None },
                    ].into_iter().flatten().collect();
                    self.err(
                        func,
                        format!("non-exhaustive match on Result: missing {}", missing.join(", ")),
                        Some("add a wildcard arm '_:' or cover all cases".to_string()),
                    );
                }
            }
            Ty::Bool => {
                let has_true = arms.iter().any(|a| matches!(&a.pattern, Pattern::Literal(Literal::Bool(true))));
                let has_false = arms.iter().any(|a| matches!(&a.pattern, Pattern::Literal(Literal::Bool(false))));
                if !has_true || !has_false {
                    let missing: Vec<&str> = [
                        if !has_true { Some("true") } else { None },
                        if !has_false { Some("false") } else { None },
                    ].into_iter().flatten().collect();
                    self.err(
                        func,
                        format!("non-exhaustive match on Bool: missing {}", missing.join(", ")),
                        Some("add a wildcard arm '_:' or cover all cases".to_string()),
                    );
                }
            }
            // For other types (Number, Text, Named, etc.) we can't enumerate
            // all possible values, so warn if there's no wildcard.
            // Nil arises from subjectless match (?{...}) where the actual type
            // is the implicit last result — we can't check exhaustiveness here.
            Ty::Unknown | Ty::Nil => {}
            _ => {
                self.err(
                    func,
                    "non-exhaustive match: no wildcard arm".to_string(),
                    Some("add a wildcard arm '_:' to handle remaining cases".to_string()),
                );
            }
        }
    }
}

/// Run static verification on a parsed program.
/// Returns Ok(()) if valid, Err(errors) if problems found.
pub fn verify(program: &Program) -> Result<(), Vec<VerifyError>> {
    let mut ctx = VerifyContext::new();

    // Phase 1: collect declarations
    ctx.collect_declarations(program);

    // Phase 2: verify function bodies
    ctx.verify_bodies(program);

    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ctx.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_verify(code: &str) -> Result<(), Vec<VerifyError>> {
        let tokens = crate::lexer::lex(code).expect("lex failed");
        let token_spans: Vec<(crate::lexer::Token, crate::ast::Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, crate::ast::Span { start: r.start, end: r.end }))
            .collect();
        let program = crate::parser::parse(token_spans).expect("parse failed");
        verify(&program)
    }

    #[test]
    fn valid_simple_function() {
        assert!(parse_and_verify("f x:n>n;*x 2").is_ok());
    }

    #[test]
    fn valid_multi_param() {
        assert!(parse_and_verify("tot p:n q:n r:n>n;s=*p q;t=*s r;+s t").is_ok());
    }

    #[test]
    fn valid_bool_function() {
        assert!(parse_and_verify("f x:b>b;!x").is_ok());
    }

    #[test]
    fn valid_text_function() {
        assert!(parse_and_verify("f x:t>t;x").is_ok());
    }

    #[test]
    fn undefined_variable() {
        let result = parse_and_verify("f x:n>n;*y 2");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("undefined variable 'y'")));
    }

    #[test]
    fn undefined_function() {
        let result = parse_and_verify("f x:n>n;foo x");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("undefined function 'foo'")));
    }

    #[test]
    fn arity_mismatch() {
        let result = parse_and_verify("g a:n b:n>n;+a b f x:n>n;g x");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("arity mismatch")));
    }

    #[test]
    fn type_mismatch_param() {
        let result = parse_and_verify("g x:n>n;*x 2 f x:t>n;g x");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("type mismatch")));
    }

    #[test]
    fn multiply_on_text() {
        let result = parse_and_verify("f x:t>n;*x 2");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("'*' expects n and n")));
    }

    #[test]
    fn valid_let_binding() {
        assert!(parse_and_verify("f x:n>n;y=*x 2;+y 1").is_ok());
    }

    #[test]
    fn valid_guard() {
        assert!(parse_and_verify("f x:n>t;>x 10{\"big\"};\"small\"").is_ok());
    }

    #[test]
    fn valid_list() {
        assert!(parse_and_verify("f x:n>L n;[x, *x 2, *x 3]").is_ok());
    }

    #[test]
    fn valid_builtins() {
        assert!(parse_and_verify("f x:n>t;str x").is_ok());
        assert!(parse_and_verify("f x:t>n;len x").is_ok());
        assert!(parse_and_verify("f x:n>n;abs x").is_ok());
    }

    #[test]
    fn builtin_arity_mismatch() {
        let result = parse_and_verify("f x:n>n;min x");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("arity mismatch") && e.message.contains("min")));
    }

    #[test]
    fn compatible_types() {
        assert!(compatible(&Ty::Number, &Ty::Number));
        assert!(compatible(&Ty::Unknown, &Ty::Number));
        assert!(compatible(&Ty::Number, &Ty::Unknown));
        assert!(!compatible(&Ty::Number, &Ty::Text));
        assert!(compatible(
            &Ty::List(Box::new(Ty::Number)),
            &Ty::List(Box::new(Ty::Number))
        ));
        assert!(!compatible(
            &Ty::List(Box::new(Ty::Number)),
            &Ty::List(Box::new(Ty::Text))
        ));
    }

    #[test]
    fn valid_ok_err() {
        assert!(parse_and_verify("f x:n>R n t;~x").is_ok());
        assert!(parse_and_verify("f x:t>R n t;^x").is_ok());
    }

    #[test]
    fn valid_match() {
        assert!(parse_and_verify("f x:R n t>n;?x{^e:0;~v:v;_:1}").is_ok());
    }

    #[test]
    fn valid_foreach() {
        assert!(parse_and_verify("f xs:L n>n;s=0;@x xs{s=+s x};s").is_ok());
    }

    #[test]
    fn foreach_on_non_list() {
        let result = parse_and_verify("f x:n>n;@i x{i};0");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("foreach expects a list")));
    }

    #[test]
    fn duplicate_function() {
        // Two functions both named "dup" — second starts a new decl after first body
        let result = parse_and_verify("dup x:n>n;*x 2 dup x:n>n;+x 1");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("duplicate function")));
    }

    #[test]
    fn valid_nested_prefix() {
        assert!(parse_and_verify("f a:n b:n c:n>n;+*a b c").is_ok());
    }

    #[test]
    fn valid_multi_function_calls() {
        // Two functions: dbl doubles, then apply calls dbl
        assert!(parse_and_verify("dbl x:n>n;*x 2 apply x:n>n;dbl x").is_ok());
    }

    #[test]
    fn return_type_mismatch() {
        let result = parse_and_verify("f x:n>t;*x 2");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("return type mismatch")));
    }

    #[test]
    fn valid_negated_guard() {
        assert!(parse_and_verify("f x:b>t;!x{\"yes\"};\"no\"").is_ok());
    }

    #[test]
    fn index_on_non_list() {
        let result = parse_and_verify("f x:n>n;x.0");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("index access on non-list")));
    }

    #[test]
    fn did_you_mean_hint() {
        let result = parse_and_verify("calc x:n>n;*x 2 f x:n>n;calx x");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let err = errors.iter().find(|e| e.message.contains("undefined function 'calx'")).unwrap();
        assert!(err.hint.as_ref().is_some_and(|h| h.contains("did you mean 'calc'?")));
    }

    // --- Match exhaustiveness tests ---

    #[test]
    fn exhaustive_result_match_with_both_arms() {
        // ~v and ^e covers Result fully
        assert!(parse_and_verify("f x:R n t>n;?x{~v:v;^e:0}").is_ok());
    }

    #[test]
    fn exhaustive_result_match_with_wildcard() {
        // wildcard covers everything
        assert!(parse_and_verify("f x:R n t>n;?x{~v:v;_:0}").is_ok());
    }

    #[test]
    fn non_exhaustive_result_missing_err() {
        let result = parse_and_verify("f x:R n t>n;?x{~v:v}");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("non-exhaustive") && e.message.contains("^")));
    }

    #[test]
    fn non_exhaustive_result_missing_ok() {
        let result = parse_and_verify("f x:R n t>n;?x{^e:0}");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("non-exhaustive") && e.message.contains("~")));
    }

    #[test]
    fn exhaustive_bool_match() {
        assert!(parse_and_verify("f x:b>n;?x{true:1;false:0}").is_ok());
    }

    #[test]
    fn non_exhaustive_bool_missing_false() {
        let result = parse_and_verify("f x:b>n;?x{true:1}");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("non-exhaustive") && e.message.contains("false")));
    }

    #[test]
    fn non_exhaustive_number_no_wildcard() {
        let result = parse_and_verify("f x:n>t;?x{1:\"one\";2:\"two\"}");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("non-exhaustive") && e.message.contains("no wildcard")));
    }

    #[test]
    fn exhaustive_number_with_wildcard() {
        assert!(parse_and_verify("f x:n>t;?x{1:\"one\";_:\"other\"}").is_ok());
    }

    #[test]
    fn subjectless_match_no_false_positive() {
        // Subjectless match ?{...} — subject_ty is Nil, should not trigger exhaustiveness error
        assert!(parse_and_verify("f x:R n t>n;?x{~v:v;^e:0}").is_ok());
    }
}
