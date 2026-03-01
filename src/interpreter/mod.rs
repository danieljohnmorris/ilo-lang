use std::collections::HashMap;
use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    Nil,
    List(Vec<Value>),
    Record { type_name: String, fields: HashMap<String, Value> },
    Ok(Box<Value>),
    Err(Box<Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => {
                if *n == (*n as i64) as f64 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Text(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Record { type_name, fields } => {
                write!(f, "{} {{", type_name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Ok(v) => write!(f, "~{}", v),
            Value::Err(v) => write!(f, "^{}", v),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Runtime error: {message}")]
pub struct RuntimeError {
    pub code: &'static str,
    pub message: String,
    pub span: Option<crate::ast::Span>,
    pub call_stack: Vec<String>,
    /// When set, the `!` operator is propagating an Err value — not a real error.
    pub propagate_value: Option<Value>,
}

impl RuntimeError {
    fn new(code: &'static str, msg: impl Into<String>) -> Self {
        RuntimeError { code, message: msg.into(), span: None, call_stack: Vec::new(), propagate_value: None }
    }
}

type Result<T> = std::result::Result<T, RuntimeError>;

struct Env {
    scopes: Vec<HashMap<String, Value>>,
    functions: HashMap<String, Decl>,
    call_stack: Vec<String>,
}

impl Env {
    fn new() -> Self {
        Env {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            call_stack: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn set(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    fn get(&self, name: &str) -> Result<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Ok(val.clone());
            }
        }
        Err(RuntimeError::new("ILO-R001", format!("undefined variable: {}", name)))
    }

    fn function(&self, name: &str) -> Result<Decl> {
        self.functions.get(name).cloned().ok_or_else(|| {
            RuntimeError::new("ILO-R002", format!("undefined function: {}", name))
        })
    }
}

/// Signal that a body produced an early return
enum BodyResult {
    /// Normal completion, last value
    Value(Value),
    /// Early return from guard
    Return(Value),
}

pub fn run(program: &Program, func_name: Option<&str>, args: Vec<Value>) -> Result<Value> {
    let mut env = Env::new();

    // Register all functions and tools
    for decl in &program.declarations {
        match decl {
            Decl::Function { name, .. } | Decl::Tool { name, .. } => {
                env.functions.insert(name.clone(), decl.clone());
            }
            Decl::TypeDef { .. } | Decl::Error { .. } => {}
        }
    }

    // Find function to call
    let target = match func_name {
        Some(name) => name.to_string(),
        None => {
            // Find first function
            program.declarations.iter()
                .find_map(|d| match d {
                    Decl::Function { name, .. } => Some(name.clone()),
                    _ => None,
                })
                .ok_or_else(|| RuntimeError::new("ILO-R012", "no functions defined"))?
        }
    };

    call_function(&mut env, &target, args)
}

fn call_function(env: &mut Env, name: &str, args: Vec<Value>) -> Result<Value> {
    // Builtins
    if name == "len" {
        if args.len() != 1 {
            return Err(RuntimeError::new("ILO-R009", format!("len: expected 1 arg, got {}", args.len())));
        }
        return match &args[0] {
            Value::Text(s) => Ok(Value::Number(s.len() as f64)),
            Value::List(l) => Ok(Value::Number(l.len() as f64)),
            other => Err(RuntimeError::new("ILO-R009", format!("len requires string or list, got {:?}", other))),
        };
    }
    if name == "str" {
        if args.len() != 1 {
            return Err(RuntimeError::new("ILO-R009", format!("str: expected 1 arg, got {}", args.len())));
        }
        return match &args[0] {
            Value::Number(n) => {
                let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                };
                Ok(Value::Text(s))
            }
            other => Err(RuntimeError::new("ILO-R009", format!("str requires a number, got {:?}", other))),
        };
    }
    if name == "num" {
        if args.len() != 1 {
            return Err(RuntimeError::new("ILO-R009", format!("num: expected 1 arg, got {}", args.len())));
        }
        return match &args[0] {
            Value::Text(s) => match s.parse::<f64>() {
                Ok(n) => Ok(Value::Ok(Box::new(Value::Number(n)))),
                Err(_) => Ok(Value::Err(Box::new(Value::Text(s.clone())))),
            },
            other => Err(RuntimeError::new("ILO-R009", format!("num requires text, got {:?}", other))),
        };
    }
    if name == "abs" {
        if args.len() != 1 {
            return Err(RuntimeError::new("ILO-R009", format!("abs: expected 1 arg, got {}", args.len())));
        }
        return match &args[0] {
            Value::Number(n) => Ok(Value::Number(n.abs())),
            other => Err(RuntimeError::new("ILO-R009", format!("abs requires a number, got {:?}", other))),
        };
    }
    if (name == "min" || name == "max") && args.len() == 2 {
        return match (&args[0], &args[1]) {
            (Value::Number(a), Value::Number(b)) => {
                let result = if name == "min" { a.min(*b) } else { a.max(*b) };
                Ok(Value::Number(result))
            }
            _ => Err(RuntimeError::new("ILO-R009", format!("{} requires two numbers", name))),
        };
    }
    if (name == "flr" || name == "cel") && args.len() == 1 {
        return match &args[0] {
            Value::Number(n) => {
                let result = if name == "flr" { n.floor() } else { n.ceil() };
                Ok(Value::Number(result))
            }
            other => Err(RuntimeError::new("ILO-R009", format!("{} requires a number, got {:?}", name, other))),
        };
    }
    if name == "spl" && args.len() == 2 {
        return match (&args[0], &args[1]) {
            (Value::Text(s), Value::Text(sep)) => {
                let parts: Vec<Value> = s.split(sep.as_str()).map(|p| Value::Text(p.to_string())).collect();
                Ok(Value::List(parts))
            }
            _ => Err(RuntimeError::new("ILO-R009", "spl requires two text args".to_string())),
        };
    }
    if name == "get" && args.len() == 1 {
        return match &args[0] {
            Value::Text(url) => {
                #[cfg(feature = "http")]
                {
                    match minreq::get(url.as_str()).send() {
                        Ok(resp) => match resp.as_str() {
                            Ok(body) => Ok(Value::Ok(Box::new(Value::Text(body.to_string())))),
                            Err(e) => Ok(Value::Err(Box::new(Value::Text(format!("response is not valid UTF-8: {e}"))))),
                        },
                        Err(e) => Ok(Value::Err(Box::new(Value::Text(e.to_string())))),
                    }
                }
                #[cfg(not(feature = "http"))]
                {
                    let _ = url;
                    Ok(Value::Err(Box::new(Value::Text("http feature not enabled".to_string()))))
                }
            }
            other => Err(RuntimeError::new("ILO-R009", format!("get requires text, got {:?}", other))),
        };
    }

    let decl = env.function(name)?;
    match decl {
        Decl::Function { params, body, name: func_name, .. } => {
            if args.len() != params.len() {
                return Err(RuntimeError::new("ILO-R004", format!(
                    "{}: expected {} args, got {}", name, params.len(), args.len()
                )));
            }
            env.push_scope();
            for (param, arg) in params.iter().zip(args) {
                env.set(&param.name, arg);
            }
            env.call_stack.push(func_name.clone());
            let result = eval_body(env, &body);
            env.call_stack.pop();
            env.pop_scope();
            match result? {
                BodyResult::Value(v) | BodyResult::Return(v) => Ok(v),
            }
        }
        Decl::Tool { name, .. } => {
            let args_str: Vec<String> = args.iter().map(|a| format!("{}", a)).collect();
            eprintln!("tool call: {}({})", name, args_str.join(", "));
            Ok(Value::Ok(Box::new(Value::Nil)))
        }
        Decl::TypeDef { .. } => {
            Err(RuntimeError::new("ILO-R004", format!("{} is a type, not callable", name)))
        }
        Decl::Error { .. } => {
            Err(RuntimeError::new("ILO-R002", format!("{} failed to parse", name)))
        }
    }
}

fn eval_body(env: &mut Env, stmts: &[Spanned<Stmt>]) -> Result<BodyResult> {
    let mut last = Value::Nil;
    for (i, spanned) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;
        match eval_stmt(env, &spanned.node, is_last) {
            Ok(Some(BodyResult::Return(v))) => return Ok(BodyResult::Return(v)),
            Ok(Some(BodyResult::Value(v))) => last = v,
            Ok(None) => {}
            Err(mut e) => {
                // Auto-unwrap propagation: convert to early return
                if let Some(val) = e.propagate_value.take() {
                    return Ok(BodyResult::Return(val));
                }
                if e.span.is_none() { e.span = Some(spanned.span); }
                if e.call_stack.is_empty() {
                    e.call_stack = env.call_stack.clone();
                }
                return Err(e);
            }
        }
    }
    Ok(BodyResult::Value(last))
}

fn eval_stmt(env: &mut Env, stmt: &Stmt, is_last: bool) -> Result<Option<BodyResult>> {
    match stmt {
        Stmt::Let { name, value } => {
            let val = eval_expr(env, value)?;
            env.set(name, val);
            Ok(None)
        }
        Stmt::Guard { condition, negated, body } => {
            let cond = eval_expr(env, condition)?;
            let truth = is_truthy(&cond);
            let should_run = if *negated { !truth } else { truth };
            if should_run {
                env.push_scope();
                let result = eval_body(env, body);
                env.pop_scope();
                let v = match result? {
                    BodyResult::Value(v) | BodyResult::Return(v) => v,
                };
                Ok(Some(BodyResult::Return(v)))
            } else {
                Ok(None)
            }
        }
        Stmt::Match { subject, arms } => {
            let subj = match subject {
                Some(e) => eval_expr(env, e)?,
                None => Value::Nil,
            };
            for arm in arms {
                if let Some(bindings) = match_pattern(&arm.pattern, &subj) {
                    env.push_scope();
                    for (name, val) in bindings {
                        env.set(&name, val);
                    }
                    let result = eval_body(env, &arm.body);
                    env.pop_scope();
                    match result? {
                        BodyResult::Return(v) => return Ok(Some(BodyResult::Return(v))),
                        BodyResult::Value(v) => {
                            if is_last {
                                return Ok(Some(BodyResult::Return(v)));
                            }
                            return Ok(Some(BodyResult::Value(v)));
                        }
                    }
                }
            }
            Ok(None)
        }
        Stmt::ForEach { binding, collection, body } => {
            let coll = eval_expr(env, collection)?;
            match coll {
                Value::List(items) => {
                    let mut last = Value::Nil;
                    for item in items {
                        env.push_scope();
                        env.set(binding, item);
                        let result = eval_body(env, body);
                        env.pop_scope();
                        match result? {
                            BodyResult::Return(v) => {
                                return Ok(Some(BodyResult::Return(v)));
                            }
                            BodyResult::Value(v) => last = v,
                        }
                    }
                    Ok(Some(BodyResult::Value(last)))
                }
                _ => Err(RuntimeError::new("ILO-R007", "foreach requires a list")),
            }
        }
        Stmt::Expr(expr) => {
            let val = eval_expr(env, expr)?;
            Ok(Some(BodyResult::Value(val)))
        }
    }
}

fn eval_expr(env: &mut Env, expr: &Expr) -> Result<Value> {
    match expr {
        Expr::Literal(lit) => Ok(eval_literal(lit)),
        Expr::Ref(name) => env.get(name),
        Expr::Field { object, field } => {
            let obj = eval_expr(env, object)?;
            match obj {
                Value::Record { fields, .. } => {
                    fields.get(field).cloned().ok_or_else(|| {
                        RuntimeError::new("ILO-R005", format!("no field '{}' on record", field))
                    })
                }
                _ => Err(RuntimeError::new("ILO-R005", format!("cannot access field '{}' on non-record", field))),
            }
        }
        Expr::Index { object, index } => {
            let obj = eval_expr(env, object)?;
            match obj {
                Value::List(items) => {
                    items.get(*index).cloned().ok_or_else(|| {
                        RuntimeError::new("ILO-R006", format!("list index {} out of bounds (len {})", index, items.len()))
                    })
                }
                _ => Err(RuntimeError::new("ILO-R006", "index access on non-list")),
            }
        }
        Expr::Call { function, args, unwrap } => {
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(eval_expr(env, arg)?);
            }
            let result = call_function(env, function, arg_vals)?;
            if *unwrap {
                match result {
                    Value::Ok(v) => Ok(*v),
                    Value::Err(e) => Err(RuntimeError {
                        propagate_value: Some(Value::Err(e)),
                        ..RuntimeError::new("ILO-R014", "auto-unwrap propagating Err")
                    }),
                    other => Ok(other), // non-Result values pass through
                }
            } else {
                Ok(result)
            }
        }
        Expr::BinOp { op, left, right } => {
            // Short-circuit for logical ops
            if *op == BinOp::And {
                let l = eval_expr(env, left)?;
                return if !is_truthy(&l) { Ok(l) } else { eval_expr(env, right) };
            }
            if *op == BinOp::Or {
                let l = eval_expr(env, left)?;
                return if is_truthy(&l) { Ok(l) } else { eval_expr(env, right) };
            }
            let l = eval_expr(env, left)?;
            let r = eval_expr(env, right)?;
            eval_binop(op, &l, &r)
        }
        Expr::UnaryOp { op, operand } => {
            let val = eval_expr(env, operand)?;
            match op {
                UnaryOp::Not => Ok(Value::Bool(!is_truthy(&val))),
                UnaryOp::Negate => match val {
                    Value::Number(n) => Ok(Value::Number(-n)),
                    _ => Err(RuntimeError::new("ILO-R004", "cannot negate non-number")),
                },
            }
        }
        Expr::Ok(inner) => {
            let val = eval_expr(env, inner)?;
            Ok(Value::Ok(Box::new(val)))
        }
        Expr::Err(inner) => {
            let val = eval_expr(env, inner)?;
            Ok(Value::Err(Box::new(val)))
        }
        Expr::List(items) => {
            let mut vals = Vec::new();
            for item in items {
                vals.push(eval_expr(env, item)?);
            }
            Ok(Value::List(vals))
        }
        Expr::Record { type_name, fields } => {
            let mut field_map = HashMap::new();
            for (name, val_expr) in fields {
                field_map.insert(name.clone(), eval_expr(env, val_expr)?);
            }
            Ok(Value::Record {
                type_name: type_name.clone(),
                fields: field_map,
            })
        }
        Expr::Match { subject, arms } => {
            let subj = match subject {
                Some(e) => eval_expr(env, e)?,
                None => Value::Nil,
            };
            for arm in arms {
                if let Some(bindings) = match_pattern(&arm.pattern, &subj) {
                    env.push_scope();
                    for (name, val) in bindings {
                        env.set(&name, val);
                    }
                    let result = eval_body(env, &arm.body);
                    env.pop_scope();
                    return match result? {
                        BodyResult::Value(v) | BodyResult::Return(v) => Ok(v),
                    };
                }
            }
            Ok(Value::Nil)
        }
        Expr::With { object, updates } => {
            let obj = eval_expr(env, object)?;
            match obj {
                Value::Record { type_name, mut fields } => {
                    for (name, val_expr) in updates {
                        fields.insert(name.clone(), eval_expr(env, val_expr)?);
                    }
                    Ok(Value::Record { type_name, fields })
                }
                _ => Err(RuntimeError::new("ILO-R008", "'with' requires a record")),
            }
        }
    }
}

fn eval_literal(lit: &Literal) -> Value {
    match lit {
        Literal::Number(n) => Value::Number(*n),
        Literal::Text(s) => Value::Text(s.clone()),
        Literal::Bool(b) => Value::Bool(*b),
    }
}

fn eval_binop(op: &BinOp, left: &Value, right: &Value) -> Result<Value> {
    match (op, left, right) {
        // Numeric ops
        (BinOp::Add, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
        (BinOp::Subtract, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
        (BinOp::Multiply, Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
        (BinOp::Divide, Value::Number(a), Value::Number(b)) => {
            if *b == 0.0 {
                Err(RuntimeError::new("ILO-R003", "division by zero"))
            } else {
                Ok(Value::Number(a / b))
            }
        }
        // String concatenation with +
        (BinOp::Add, Value::Text(a), Value::Text(b)) => {
            let mut out = String::with_capacity(a.len() + b.len());
            out.push_str(a);
            out.push_str(b);
            Ok(Value::Text(out))
        }
        // List concatenation with +
        (BinOp::Add, Value::List(a), Value::List(b)) => {
            let mut out = Vec::with_capacity(a.len() + b.len());
            out.extend_from_slice(a);
            out.extend_from_slice(b);
            Ok(Value::List(out))
        }
        // Comparisons on numbers
        (BinOp::GreaterThan, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
        (BinOp::LessThan, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
        (BinOp::GreaterOrEqual, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
        (BinOp::LessOrEqual, Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
        // Comparisons on text (lexicographic)
        (BinOp::GreaterThan, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a > b)),
        (BinOp::LessThan, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a < b)),
        (BinOp::GreaterOrEqual, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a >= b)),
        (BinOp::LessOrEqual, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a <= b)),
        // List append
        (BinOp::Append, Value::List(items), val) => {
            let mut new_items = items.clone();
            new_items.push(val.clone());
            Ok(Value::List(new_items))
        }
        // Equality
        (BinOp::Equals, a, b) => Ok(Value::Bool(values_equal(a, b))),
        (BinOp::NotEquals, a, b) => Ok(Value::Bool(!values_equal(a, b))),
        _ => Err(RuntimeError::new("ILO-R004", format!(
            "unsupported operation: {:?} on {:?} and {:?}", op, left, right
        ))),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Nil, Value::Nil) => true,
        _ => false,
    }
}

fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Nil => false,
        Value::Number(n) => *n != 0.0,
        Value::Text(s) => !s.is_empty(),
        Value::List(l) => !l.is_empty(),
        _ => true,
    }
}

fn match_pattern(pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
    match pattern {
        Pattern::Wildcard => Some(vec![]),
        Pattern::Ok(binding) => {
            if let Value::Ok(inner) = value {
                let mut bindings = vec![];
                if binding != "_" {
                    bindings.push((binding.clone(), *inner.clone()));
                }
                Some(bindings)
            } else {
                None
            }
        }
        Pattern::Err(binding) => {
            if let Value::Err(inner) = value {
                let mut bindings = vec![];
                if binding != "_" {
                    bindings.push((binding.clone(), *inner.clone()));
                }
                Some(bindings)
            } else {
                None
            }
        }
        Pattern::Literal(lit) => {
            let expected = eval_literal(lit);
            if values_equal(&expected, value) {
                Some(vec![])
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn parse_program(source: &str) -> Program {
        let tokens = lexer::lex(source).unwrap();
        let token_spans: Vec<(crate::lexer::Token, crate::ast::Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, crate::ast::Span { start: r.start, end: r.end }))
            .collect();
        let (prog, errors) = parser::parse(token_spans);
        assert!(errors.is_empty(), "parse errors: {:?}", errors);
        prog
    }

    fn run_str(source: &str, func: Option<&str>, args: Vec<Value>) -> Value {
        let prog = parse_program(source);
        run(&prog, func, args).unwrap()
    }

    #[test]
    fn interpret_tot() {
        // tot p:n q:n r:n>n;s=*p q;t=*s r;+s t
        let source = std::fs::read_to_string("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo").unwrap();
        let result = run_str(
            &source,
            Some("tot"),
            vec![Value::Number(10.0), Value::Number(20.0), Value::Number(30.0)],
        );
        assert_eq!(result, Value::Number(6200.0));
    }

    #[test]
    fn interpret_tot_different_args() {
        let source = "tot p:n q:n r:n>n;s=*p q;t=*s r;+s t";
        let result = run_str(
            source,
            Some("tot"),
            vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)],
        );
        // s = 2*3 = 6, t = 6*4 = 24, s+t = 30
        assert_eq!(result, Value::Number(30.0));
    }

    #[test]
    fn interpret_cls_gold() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = run_str(source, Some("cls"), vec![Value::Number(1000.0)]);
        assert_eq!(result, Value::Text("gold".to_string()));
    }

    #[test]
    fn interpret_cls_silver() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = run_str(source, Some("cls"), vec![Value::Number(500.0)]);
        assert_eq!(result, Value::Text("silver".to_string()));
    }

    #[test]
    fn interpret_cls_bronze() {
        let source = r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#;
        let result = run_str(source, Some("cls"), vec![Value::Number(100.0)]);
        assert_eq!(result, Value::Text("bronze".to_string()));
    }

    #[test]
    fn interpret_match_stmt() {
        let source = r#"f x:t>n;?x{"a":1;"b":2;_:0}"#;
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Text("a".to_string())]),
            Value::Number(1.0)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Text("b".to_string())]),
            Value::Number(2.0)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Text("z".to_string())]),
            Value::Number(0.0)
        );
    }

    #[test]
    fn interpret_ok_err() {
        let source = "f x:n>R n t;~x";
        let result = run_str(source, Some("f"), vec![Value::Number(42.0)]);
        assert_eq!(result, Value::Ok(Box::new(Value::Number(42.0))));
    }

    #[test]
    fn interpret_err_constructor() {
        let source = r#"f x:n>R n t;^"bad""#;
        let result = run_str(source, Some("f"), vec![Value::Number(0.0)]);
        assert_eq!(result, Value::Err(Box::new(Value::Text("bad".to_string()))));
    }

    #[test]
    fn interpret_match_ok_err_patterns() {
        let source = r#"f x:R n t>n;?x{^e:0;~v:v}"#;
        let ok_result = run_str(
            source,
            Some("f"),
            vec![Value::Ok(Box::new(Value::Number(42.0)))],
        );
        assert_eq!(ok_result, Value::Number(42.0));

        let err_result = run_str(
            source,
            Some("f"),
            vec![Value::Err(Box::new(Value::Text("oops".to_string())))],
        );
        assert_eq!(err_result, Value::Number(0.0));
    }

    #[test]
    fn interpret_negated_guard() {
        let source = r#"f x:b>t;!x{"nope"};"yes""#;
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(false)]),
            Value::Text("nope".to_string())
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(true)]),
            Value::Text("yes".to_string())
        );
    }

    #[test]
    fn interpret_logical_not() {
        let source = "f x:b>b;!x";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(true)]),
            Value::Bool(false)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(false)]),
            Value::Bool(true)
        );
    }

    #[test]
    fn interpret_record_and_field() {
        let source = "f x:n>n;r=point x:x y:10;r.y";
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn interpret_with_expr() {
        let source = "f>n;r=point x:1 y:2;r2=r with y:10;r2.y";
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn interpret_string_concat() {
        let source = r#"f a:t b:t>t;+a b"#;
        let result = run_str(
            source,
            Some("f"),
            vec![Value::Text("hello ".to_string()), Value::Text("world".to_string())],
        );
        assert_eq!(result, Value::Text("hello world".to_string()));
    }

    #[test]
    fn interpret_string_comparison() {
        let gt = r#"f a:t b:t>b;>a b"#;
        assert_eq!(
            run_str(gt, Some("f"), vec![Value::Text("banana".into()), Value::Text("apple".into())]),
            Value::Bool(true)
        );
        assert_eq!(
            run_str(gt, Some("f"), vec![Value::Text("apple".into()), Value::Text("banana".into())]),
            Value::Bool(false)
        );

        let lt = r#"f a:t b:t>b;<a b"#;
        assert_eq!(
            run_str(lt, Some("f"), vec![Value::Text("apple".into()), Value::Text("banana".into())]),
            Value::Bool(true)
        );

        let ge = r#"f a:t b:t>b;>=a b"#;
        assert_eq!(
            run_str(ge, Some("f"), vec![Value::Text("apple".into()), Value::Text("apple".into())]),
            Value::Bool(true)
        );

        let le = r#"f a:t b:t>b;<=a b"#;
        assert_eq!(
            run_str(le, Some("f"), vec![Value::Text("zebra".into()), Value::Text("banana".into())]),
            Value::Bool(false)
        );
    }

    #[test]
    fn interpret_match_expr_in_let() {
        let source = r#"f x:t>n;y=?x{"a":1;"b":2;_:0};y"#;
        let result = run_str(source, Some("f"), vec![Value::Text("b".to_string())]);
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn interpret_default_first_function() {
        let source = "f>n;42";
        let result = run_str(source, None, vec![]);
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn interpret_division_by_zero() {
        let source = "f x:n>n;/x 0";
        let prog = parse_program(source);
        let result = run(&prog, Some("f"), vec![Value::Number(10.0)]);
        assert!(result.is_err());
    }

    #[test]
    fn interpret_logical_and() {
        let source = "f a:b b:b>b;&a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(true), Value::Bool(true)]),
            Value::Bool(true)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(true), Value::Bool(false)]),
            Value::Bool(false)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(false), Value::Bool(true)]),
            Value::Bool(false)
        );
    }

    #[test]
    fn interpret_logical_or() {
        let source = "f a:b b:b>b;|a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(false), Value::Bool(false)]),
            Value::Bool(false)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(true), Value::Bool(false)]),
            Value::Bool(true)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Bool(false), Value::Bool(true)]),
            Value::Bool(true)
        );
    }

    #[test]
    fn interpret_len_string() {
        let source = r#"f s:t>n;len s"#;
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Text("hello".to_string())]),
            Value::Number(5.0)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Text("".to_string())]),
            Value::Number(0.0)
        );
    }

    #[test]
    fn interpret_len_list() {
        let source = "f>n;xs=[1, 2, 3];len xs";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(3.0));
    }

    #[test]
    fn interpret_list_append() {
        let source = "f>L n;xs=[1, 2];+=xs 3";
        assert_eq!(
            run_str(source, Some("f"), vec![]),
            Value::List(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])
        );
    }

    #[test]
    fn interpret_list_append_empty() {
        let source = "f>L n;xs=[];+=xs 42";
        assert_eq!(
            run_str(source, Some("f"), vec![]),
            Value::List(vec![Value::Number(42.0)])
        );
    }

    #[test]
    fn interpret_list_concat() {
        let source = "f>L n;a=[1, 2];b=[3, 4];+a b";
        assert_eq!(
            run_str(source, Some("f"), vec![]),
            Value::List(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)])
        );
    }

    #[test]
    fn interpret_str_integer() {
        let source = "f>t;str 42";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Text("42".into()));
    }

    #[test]
    fn interpret_str_float() {
        let source = "f>t;str 3.14";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Text("3.14".into()));
    }

    #[test]
    fn interpret_num_ok() {
        let source = "f>R n t;num \"42\"";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Ok(Box::new(Value::Number(42.0))));
    }

    #[test]
    fn interpret_num_err() {
        let source = "f>R n t;num \"abc\"";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Err(Box::new(Value::Text("abc".into()))));
    }

    #[test]
    fn interpret_abs() {
        let source = "f>n;abs -7";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(7.0));
    }

    #[test]
    fn interpret_min() {
        let source = "f>n;min 3 7";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(3.0));
    }

    #[test]
    fn interpret_max() {
        let source = "f>n;max 3 7";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(7.0));
    }

    #[test]
    fn interpret_flr() {
        let source = "f>n;flr 3.7";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(3.0));
    }

    #[test]
    fn interpret_cel() {
        let source = "f>n;cel 3.2";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(4.0));
    }

    #[test]
    fn interpret_index_access() {
        let source = "f>n;xs=[10, 20, 30];xs.1";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Number(20.0));
    }

    #[test]
    fn interpret_index_access_string() {
        let source = "f>t;xs=[\"hello\", \"world\"];xs.0";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Text("hello".into()));
    }

    #[test]
    fn interpret_multi_function() {
        let source = "double x:n>n;*x 2\nf x:n>n;double x";
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn interpret_nested_multiply_add() {
        // +*a b c → (a * b) + c
        let source = "f a:n b:n c:n>n;+*a b c";
        let result = run_str(source, Some("f"), vec![Value::Number(2.0), Value::Number(3.0), Value::Number(4.0)]);
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn interpret_nested_compare() {
        // >=+x y 100 → (x + y) >= 100
        let source = "f x:n y:n>b;>=+x y 100";
        let result = run_str(source, Some("f"), vec![Value::Number(60.0), Value::Number(50.0)]);
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn interpret_not_as_and_operand() {
        // &!x y → (!x) & y
        let source = "f x:b y:b>b;&!x y";
        let result = run_str(source, Some("f"), vec![Value::Bool(false), Value::Bool(true)]);
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn interpret_negate_product() {
        // -*a b → -(a * b)
        let source = "f a:n b:n>n;-*a b";
        let result = run_str(source, Some("f"), vec![Value::Number(3.0), Value::Number(4.0)]);
        assert_eq!(result, Value::Number(-12.0));
    }

    // ── Helper for error tests ──────────────────────────────────────────

    fn run_str_err(source: &str, func: Option<&str>, args: Vec<Value>) -> String {
        let prog = parse_program(source);
        run(&prog, func, args).unwrap_err().to_string()
    }

    // ── Value::fmt Display tests ────────────────────────────────────────

    #[test]
    fn display_float() {
        assert_eq!(format!("{}", Value::Number(3.14)), "3.14");
    }

    #[test]
    fn display_integer_number() {
        assert_eq!(format!("{}", Value::Number(42.0)), "42");
    }

    #[test]
    fn display_text() {
        assert_eq!(format!("{}", Value::Text("hello".into())), "hello");
    }

    #[test]
    fn display_bool() {
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Bool(false)), "false");
    }

    #[test]
    fn display_nil() {
        assert_eq!(format!("{}", Value::Nil), "nil");
    }

    #[test]
    fn display_list() {
        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        assert_eq!(format!("{}", list), "[1, 2, 3]");
    }

    #[test]
    fn display_list_empty() {
        assert_eq!(format!("{}", Value::List(vec![])), "[]");
    }

    #[test]
    fn display_record() {
        let mut fields = HashMap::new();
        fields.insert("x".to_string(), Value::Number(1.0));
        let rec = Value::Record {
            type_name: "point".into(),
            fields,
        };
        assert_eq!(format!("{}", rec), "point {x: 1}");
    }

    #[test]
    fn display_record_multiple_fields() {
        let mut fields = HashMap::new();
        fields.insert("a".to_string(), Value::Number(1.0));
        fields.insert("b".to_string(), Value::Number(2.0));
        let rec = Value::Record {
            type_name: "pair".into(),
            fields,
        };
        let s = format!("{}", rec);
        assert!(s.starts_with("pair {"));
        assert!(s.contains("a: 1"));
        assert!(s.contains("b: 2"));
        assert!(s.ends_with("}"));
    }

    #[test]
    fn display_ok() {
        assert_eq!(
            format!("{}", Value::Ok(Box::new(Value::Number(42.0)))),
            "~42"
        );
    }

    #[test]
    fn display_err() {
        assert_eq!(
            format!("{}", Value::Err(Box::new(Value::Text("bad".into())))),
            "^bad"
        );
    }

    // ── Error path tests ────────────────────────────────────────────────

    #[test]
    fn err_undefined_variable() {
        let err = run_str_err("f>n;x", Some("f"), vec![]);
        assert!(err.contains("undefined variable"));
    }

    #[test]
    fn err_undefined_function() {
        let err = run_str_err("f>n;nope 1", Some("f"), vec![]);
        assert!(err.contains("undefined function"));
    }

    #[test]
    fn err_wrong_arity() {
        let err = run_str_err("f x:n>n;x", Some("f"), vec![]);
        assert!(err.contains("expected 1 args, got 0"));
    }

    #[test]
    fn err_len_wrong_arg_count() {
        let err = run_str_err("f>n;len 1 2", Some("f"), vec![]);
        assert!(err.contains("len: expected 1 arg"));
    }

    #[test]
    fn err_len_wrong_type() {
        let err = run_str_err("f x:n>n;len x", Some("f"), vec![Value::Number(1.0)]);
        assert!(err.contains("len requires string or list"));
    }

    #[test]
    fn err_str_wrong_arg_count() {
        let err = run_str_err("f>t;str 1 2", Some("f"), vec![]);
        assert!(err.contains("str: expected 1 arg"));
    }

    #[test]
    fn err_str_wrong_type() {
        let err = run_str_err(r#"f x:t>t;str x"#, Some("f"), vec![Value::Text("hi".into())]);
        assert!(err.contains("str requires a number"));
    }

    #[test]
    fn err_num_wrong_arg_count() {
        let err = run_str_err(r#"f>R n t;num "1" "2""#, Some("f"), vec![]);
        assert!(err.contains("num: expected 1 arg"));
    }

    #[test]
    fn err_num_wrong_type() {
        let err = run_str_err("f x:n>R n t;num x", Some("f"), vec![Value::Number(1.0)]);
        assert!(err.contains("num requires text"));
    }

    #[test]
    fn err_abs_wrong_arg_count() {
        let err = run_str_err("f>n;abs 1 2", Some("f"), vec![]);
        assert!(err.contains("abs: expected 1 arg"));
    }

    #[test]
    fn err_abs_wrong_type() {
        let err = run_str_err(r#"f x:t>n;abs x"#, Some("f"), vec![Value::Text("hi".into())]);
        assert!(err.contains("abs requires a number"));
    }

    #[test]
    fn err_min_non_number() {
        let err = run_str_err(
            r#"f a:t b:t>n;min a b"#,
            Some("f"),
            vec![Value::Text("a".into()), Value::Text("b".into())],
        );
        assert!(err.contains("min requires two numbers"));
    }

    #[test]
    fn err_max_non_number() {
        let err = run_str_err(
            r#"f a:t b:t>n;max a b"#,
            Some("f"),
            vec![Value::Text("a".into()), Value::Text("b".into())],
        );
        assert!(err.contains("max requires two numbers"));
    }

    #[test]
    fn err_flr_non_number() {
        let err = run_str_err(r#"f x:t>n;flr x"#, Some("f"), vec![Value::Text("a".into())]);
        assert!(err.contains("flr requires a number"));
    }

    #[test]
    fn err_cel_non_number() {
        let err = run_str_err(r#"f x:t>n;cel x"#, Some("f"), vec![Value::Text("a".into())]);
        assert!(err.contains("cel requires a number"));
    }

    #[test]
    fn err_field_not_found_on_record() {
        let err = run_str_err("f>n;r=point x:1 y:2;r.z", Some("f"), vec![]);
        assert!(err.contains("no field 'z' on record"));
    }

    #[test]
    fn err_field_access_on_non_record() {
        let err = run_str_err("f x:n>n;x.y", Some("f"), vec![Value::Number(1.0)]);
        assert!(err.contains("cannot access field"));
    }

    #[test]
    fn err_index_out_of_bounds() {
        let err = run_str_err("f>n;xs=[1, 2];xs.5", Some("f"), vec![]);
        assert!(err.contains("out of bounds"));
    }

    #[test]
    fn err_index_on_non_list() {
        let err = run_str_err("f x:n>n;x.0", Some("f"), vec![Value::Number(1.0)]);
        // x.0 is an index access; on a number it should error
        assert!(
            err.contains("index access on non-list") || err.contains("cannot access field"),
            "got: {}", err
        );
    }

    #[test]
    fn err_negate_non_number() {
        let err = run_str_err(r#"f>n;-"hello""#, Some("f"), vec![]);
        assert!(err.contains("cannot negate non-number"));
    }

    #[test]
    fn err_with_on_non_record() {
        let err = run_str_err("f x:n>n;x with y:1", Some("f"), vec![Value::Number(1.0)]);
        assert!(err.contains("'with' requires a record"));
    }

    // ── Missing operational tests ───────────────────────────────────────

    #[test]
    fn interpret_foreach() {
        // Sum the list by calling an accumulator pattern
        // Simple: foreach that returns last value (last element * 2)
        let source = "f>n;s=0;@x [1, 2, 3]{+s x}";
        let result = run_str(source, Some("f"), vec![]);
        // ForEach returns the last body value: 0 + 3 = 3
        // (each iteration: s is still 0 because we don't reassign, body is +s x)
        // iteration 1: +0 1 = 1, iteration 2: +0 2 = 2, iteration 3: +0 3 = 3
        assert_eq!(result, Value::Number(3.0));
    }

    #[test]
    fn interpret_subtract() {
        let source = "f a:n b:n>n;-a b";
        let result = run_str(
            source,
            Some("f"),
            vec![Value::Number(10.0), Value::Number(3.0)],
        );
        assert_eq!(result, Value::Number(7.0));
    }

    #[test]
    fn interpret_divide() {
        let source = "f a:n b:n>n;/a b";
        let result = run_str(
            source,
            Some("f"),
            vec![Value::Number(10.0), Value::Number(4.0)],
        );
        assert_eq!(result, Value::Number(2.5));
    }

    #[test]
    fn interpret_equals() {
        let source = "f a:n b:n>b;=a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(1.0), Value::Number(1.0)]),
            Value::Bool(true)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(1.0), Value::Number(2.0)]),
            Value::Bool(false)
        );
    }

    #[test]
    fn interpret_not_equals() {
        let source = "f a:n b:n>b;!=a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(1.0), Value::Number(2.0)]),
            Value::Bool(true)
        );
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(1.0), Value::Number(1.0)]),
            Value::Bool(false)
        );
    }

    #[test]
    fn values_equal_numbers() {
        assert!(values_equal(&Value::Number(1.0), &Value::Number(1.0)));
        assert!(!values_equal(&Value::Number(1.0), &Value::Number(2.0)));
    }

    #[test]
    fn values_equal_bools() {
        assert!(values_equal(&Value::Bool(true), &Value::Bool(true)));
        assert!(!values_equal(&Value::Bool(true), &Value::Bool(false)));
    }

    #[test]
    fn values_equal_nil() {
        assert!(values_equal(&Value::Nil, &Value::Nil));
    }

    #[test]
    fn values_equal_mismatched() {
        assert!(!values_equal(&Value::Number(1.0), &Value::Text("1".into())));
        assert!(!values_equal(&Value::Nil, &Value::Bool(false)));
    }

    #[test]
    fn is_truthy_nil() {
        assert!(!is_truthy(&Value::Nil));
    }

    #[test]
    fn is_truthy_number_zero() {
        assert!(!is_truthy(&Value::Number(0.0)));
    }

    #[test]
    fn is_truthy_number_nonzero() {
        assert!(is_truthy(&Value::Number(1.0)));
        assert!(is_truthy(&Value::Number(-5.0)));
    }

    #[test]
    fn is_truthy_text() {
        assert!(!is_truthy(&Value::Text("".into())));
        assert!(is_truthy(&Value::Text("hello".into())));
    }

    #[test]
    fn is_truthy_list() {
        assert!(!is_truthy(&Value::List(vec![])));
        assert!(is_truthy(&Value::List(vec![Value::Number(1.0)])));
    }

    #[test]
    fn is_truthy_other() {
        // Records, Ok, Err are always truthy
        assert!(is_truthy(&Value::Ok(Box::new(Value::Nil))));
        assert!(is_truthy(&Value::Err(Box::new(Value::Nil))));
    }

    #[test]
    fn interpret_literal_bool() {
        let source = "f>b;true";
        assert_eq!(run_str(source, Some("f"), vec![]), Value::Bool(true));
        let source2 = "f>b;false";
        assert_eq!(run_str(source2, Some("f"), vec![]), Value::Bool(false));
    }

    #[test]
    fn interpret_match_no_subject() {
        // ?{...} — match with no subject means subject is Nil
        let source = r#"f>n;?{_:42}"#;
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn interpret_match_expr_with_bindings() {
        // Match expression that binds a value from Ok pattern
        let source = "f x:R n t>n;y=?x{~v:v;_:0};y";
        let result = run_str(
            source,
            Some("f"),
            vec![Value::Ok(Box::new(Value::Number(99.0)))],
        );
        assert_eq!(result, Value::Number(99.0));
    }

    #[test]
    fn interpret_match_expr_no_arm_matches() {
        // No arm matches in a match expression → returns Nil
        let source = r#"f>n;y=?1{2:99};y"#;
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn interpret_typedef_in_declarations() {
        // TypeDef should be silently skipped during registration
        let source = "type point{x:n;y:n}\nf>n;42";
        let result = run_str(source, None, vec![]);
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn interpret_pattern_literal_no_match() {
        // A literal pattern that does not match falls through
        let source = r#"f x:n>n;?x{1:10;2:20;_:0}"#;
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(0.0));
    }

    #[test]
    fn interpret_foreach_on_non_list() {
        let err = run_str_err("f x:n>n;@i x{i}", Some("f"), vec![Value::Number(1.0)]);
        assert!(err.contains("foreach requires a list"));
    }

    #[test]
    fn interpret_tool_call() {
        let source = "tool fetch\"HTTP GET\" url:t>R _ t timeout:30\nf>R _ t;fetch \"http://example.com\"";
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Ok(Box::new(Value::Nil)));
    }

    #[test]
    fn interpret_typedef_not_callable() {
        // TypeDef names are not registered as functions, so calling one
        // results in an "undefined function" error
        let source = "type point{x:n;y:n}\nf>n;point 1 2";
        let err = run_str_err(source, Some("f"), vec![]);
        assert!(
            err.contains("undefined function") || err.contains("type") || err.contains("not callable"),
            "unexpected error: {}", err
        );
    }

    #[test]
    fn interpret_greater_than() {
        let source = "f a:n b:n>b;>a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(5.0), Value::Number(3.0)]),
            Value::Bool(true)
        );
    }

    #[test]
    fn interpret_less_than() {
        let source = "f a:n b:n>b;<a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(3.0), Value::Number(5.0)]),
            Value::Bool(true)
        );
    }

    #[test]
    fn interpret_less_or_equal() {
        let source = "f a:n b:n>b;<=a b";
        assert_eq!(
            run_str(source, Some("f"), vec![Value::Number(3.0), Value::Number(3.0)]),
            Value::Bool(true)
        );
    }

    #[test]
    fn interpret_unsupported_binop() {
        let source = "f a:b b:b>b;-a b";
        let err = run_str_err(
            source,
            Some("f"),
            vec![Value::Bool(true), Value::Bool(false)],
        );
        assert!(
            err.contains("unsupported operation"),
            "unexpected error: {}", err
        );
    }

    #[test]
    fn interpret_foreach_early_return() {
        let source = "f xs:L n>n;@x xs{>=x 3{x}};0";
        let result = run_str(
            source,
            Some("f"),
            vec![Value::List(vec![
                Value::Number(1.0),
                Value::Number(5.0),
                Value::Number(2.0),
            ])],
        );
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn interpret_match_not_last_stmt() {
        let source = "f x:n>n;?x{0:x;_:x};+x 1";
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(6.0));
    }

    #[test]
    fn interpret_match_expr_no_subject() {
        let source = r#"f>t;x=?{_:"always"};x"#;
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Text("always".to_string()));
    }

    #[test]
    fn interpret_pattern_ok_no_match() {
        let source = r#"f>t;x=^"err";?x{~v:v;_:"default"}"#;
        let result = run_str(source, Some("f"), vec![]);
        assert_eq!(result, Value::Text("default".to_string()));
    }

    #[test]
    fn interpret_match_stmt_no_arm_matches() {
        // Standalone match statement (Stmt::Match) where no arm matches → Ok(None) at L307
        // The match is not the last stmt; function continues to 0 after no match.
        let source = "f x:n>n;?x{1:99};0";
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(0.0));
    }

    #[test]
    fn interpret_match_arm_body_with_guard_return() {
        // Match arm body contains a guard that fires → BodyResult::Return propagates (L297)
        // When x=1: pattern 1 matches, arm body has guard >=x 0 which is true → returns 42
        // The match is not the last stmt (y=0 is first), so BodyResult::Return propagation matters
        // Note: arm body syntax uses `;` not braces: `1:>=x 0{42}` means guard in arm 1 body
        let source = "f x:n>n;y=0;?x{1:>=x 0{42};_:0}";
        let result = run_str(source, Some("f"), vec![Value::Number(1.0)]);
        assert_eq!(result, Value::Number(42.0));
    }

    // L239: call_function with Decl::TypeDef → "is a type, not callable"
    #[test]
    fn call_typedef_as_function() {
        let mut env = Env::new();
        // Manually insert a TypeDef into the env's functions map
        env.functions.insert("point".to_string(), Decl::TypeDef {
            name: "point".to_string(),
            fields: vec![],
            span: Span::UNKNOWN,
        });
        let result = call_function(&mut env, "point", vec![]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("is a type, not callable"), "got: {}", err);
    }

    // L242: call_function with Decl::Error → "failed to parse"
    #[test]
    fn call_error_decl_as_function() {
        let mut env = Env::new();
        // Manually insert a Decl::Error into the env's functions map
        env.functions.insert("broken".to_string(), Decl::Error {
            span: Span::UNKNOWN,
        });
        let result = call_function(&mut env, "broken", vec![]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failed to parse"), "got: {}", err);
    }

    fn make_result_program(inner_body: Vec<Spanned<Stmt>>) -> Program {
        // Build: inner x:n>R n t;{inner_body}  outer x:n>R n t;d=inner! x;~d
        Program {
            declarations: vec![
                Decl::Function {
                    name: "inner".to_string(),
                    params: vec![Param { name: "x".to_string(), ty: Type::Number }],
                    return_type: Type::Result(Box::new(Type::Number), Box::new(Type::Text)),
                    body: inner_body,
                    span: Span::UNKNOWN,
                },
                Decl::Function {
                    name: "outer".to_string(),
                    params: vec![Param { name: "x".to_string(), ty: Type::Number }],
                    return_type: Type::Result(Box::new(Type::Number), Box::new(Type::Text)),
                    body: vec![
                        Spanned::unknown(Stmt::Let {
                            name: "d".to_string(),
                            value: Expr::Call {
                                function: "inner".to_string(),
                                args: vec![Expr::Ref("x".to_string())],
                                unwrap: true,
                            },
                        }),
                        Spanned::unknown(Stmt::Expr(Expr::Ok(Box::new(Expr::Ref("d".to_string()))))),
                    ],
                    span: Span::UNKNOWN,
                },
            ],
            source: None,
        }
    }

    #[test]
    fn unwrap_ok_path() {
        let prog = make_result_program(vec![
            Spanned::unknown(Stmt::Expr(Expr::Ok(Box::new(Expr::Ref("x".to_string()))))),
        ]);
        let result = run(&prog, Some("outer"), vec![Value::Number(42.0)]).unwrap();
        assert_eq!(result, Value::Ok(Box::new(Value::Number(42.0))));
    }

    #[test]
    fn unwrap_err_path() {
        let prog = make_result_program(vec![
            Spanned::unknown(Stmt::Expr(Expr::Err(Box::new(
                Expr::Literal(Literal::Text("fail".to_string()))
            )))),
        ]);
        let result = run(&prog, Some("outer"), vec![Value::Number(42.0)]).unwrap();
        assert_eq!(result, Value::Err(Box::new(Value::Text("fail".to_string()))));
    }

    #[test]
    fn unwrap_nested_propagation() {
        // c returns Err, b uses ! to call c, a uses ! to call b
        let unwrap_body = |callee: &str| vec![
            Spanned::unknown(Stmt::Let {
                name: "d".to_string(),
                value: Expr::Call {
                    function: callee.to_string(),
                    args: vec![Expr::Ref("x".to_string())],
                    unwrap: true,
                },
            }),
            Spanned::unknown(Stmt::Expr(Expr::Ok(Box::new(Expr::Ref("d".to_string()))))),
        ];
        let rnt = Type::Result(Box::new(Type::Number), Box::new(Type::Text));
        let prog = Program {
            declarations: vec![
                Decl::Function {
                    name: "c".to_string(),
                    params: vec![Param { name: "x".to_string(), ty: Type::Number }],
                    return_type: rnt.clone(),
                    body: vec![Spanned::unknown(Stmt::Expr(
                        Expr::Err(Box::new(Expr::Literal(Literal::Text("deep".to_string()))))
                    ))],
                    span: Span::UNKNOWN,
                },
                Decl::Function {
                    name: "b".to_string(),
                    params: vec![Param { name: "x".to_string(), ty: Type::Number }],
                    return_type: rnt.clone(),
                    body: unwrap_body("c"),
                    span: Span::UNKNOWN,
                },
                Decl::Function {
                    name: "a".to_string(),
                    params: vec![Param { name: "x".to_string(), ty: Type::Number }],
                    return_type: rnt,
                    body: unwrap_body("b"),
                    span: Span::UNKNOWN,
                },
            ],
            source: None,
        };
        let result = run(&prog, Some("a"), vec![Value::Number(1.0)]).unwrap();
        assert_eq!(result, Value::Err(Box::new(Value::Text("deep".to_string()))));
    }

    // ---- Braceless guards ----

    #[test]
    fn interpret_braceless_guard() {
        let source = r#"cls sp:n>t;>=sp 1000 "gold";>=sp 500 "silver";"bronze""#;
        assert_eq!(
            run_str(source, Some("cls"), vec![Value::Number(1500.0)]),
            Value::Text("gold".to_string())
        );
        assert_eq!(
            run_str(source, Some("cls"), vec![Value::Number(750.0)]),
            Value::Text("silver".to_string())
        );
        assert_eq!(
            run_str(source, Some("cls"), vec![Value::Number(100.0)]),
            Value::Text("bronze".to_string())
        );
    }

    #[test]
    fn interpret_braceless_guard_factorial() {
        let source = "fac n:n>n;<=n 1 1;r=fac -n 1;*n r";
        assert_eq!(
            run_str(source, Some("fac"), vec![Value::Number(5.0)]),
            Value::Number(120.0)
        );
    }

    #[test]
    fn interpret_braceless_guard_fibonacci() {
        let source = "fib n:n>n;<=n 1 n;a=fib -n 1;b=fib -n 2;+a b";
        assert_eq!(
            run_str(source, Some("fib"), vec![Value::Number(10.0)]),
            Value::Number(55.0)
        );
    }

    #[test]
    fn interpret_spl_basic() {
        let source = r#"f>L t;spl "a,b,c" ",""#;
        assert_eq!(
            run_str(source, Some("f"), vec![]),
            Value::List(vec![
                Value::Text("a".to_string()),
                Value::Text("b".to_string()),
                Value::Text("c".to_string()),
            ])
        );
    }

    #[test]
    fn interpret_spl_empty() {
        let source = r#"f>L t;spl "" ",""#;
        assert_eq!(
            run_str(source, Some("f"), vec![]),
            Value::List(vec![Value::Text("".to_string())])
        );
    }
}
