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
            Value::Err(v) => write!(f, "!{}", v),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Runtime error: {message}")]
pub struct RuntimeError {
    pub message: String,
}

impl RuntimeError {
    fn new(msg: impl Into<String>) -> Self {
        RuntimeError { message: msg.into() }
    }
}

type Result<T> = std::result::Result<T, RuntimeError>;

struct Env {
    scopes: Vec<HashMap<String, Value>>,
    functions: HashMap<String, Decl>,
}

impl Env {
    fn new() -> Self {
        Env {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
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
        Err(RuntimeError::new(format!("undefined variable: {}", name)))
    }

    fn function(&self, name: &str) -> Result<Decl> {
        self.functions.get(name).cloned().ok_or_else(|| {
            RuntimeError::new(format!("undefined function: {}", name))
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
            Decl::TypeDef { .. } => {}
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
                .ok_or_else(|| RuntimeError::new("no functions defined"))?
        }
    };

    call_function(&mut env, &target, args)
}

fn call_function(env: &mut Env, name: &str, args: Vec<Value>) -> Result<Value> {
    // Builtins
    if name == "len" {
        if args.len() != 1 {
            return Err(RuntimeError::new(format!("len: expected 1 arg, got {}", args.len())));
        }
        return match &args[0] {
            Value::Text(s) => Ok(Value::Number(s.len() as f64)),
            Value::List(l) => Ok(Value::Number(l.len() as f64)),
            other => Err(RuntimeError::new(format!("len requires string or list, got {:?}", other))),
        };
    }

    let decl = env.function(name)?;
    match decl {
        Decl::Function { params, body, .. } => {
            if args.len() != params.len() {
                return Err(RuntimeError::new(format!(
                    "{}: expected {} args, got {}", name, params.len(), args.len()
                )));
            }
            env.push_scope();
            for (param, arg) in params.iter().zip(args) {
                env.set(&param.name, arg);
            }
            let result = eval_body(env, &body);
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
            Err(RuntimeError::new(format!("{} is a type, not callable", name)))
        }
    }
}

fn eval_body(env: &mut Env, stmts: &[Stmt]) -> Result<BodyResult> {
    let mut last = Value::Nil;
    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;
        match eval_stmt(env, stmt, is_last)? {
            Some(BodyResult::Return(v)) => return Ok(BodyResult::Return(v)),
            Some(BodyResult::Value(v)) => last = v,
            None => {}
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
                _ => Err(RuntimeError::new("foreach requires a list")),
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
                        RuntimeError::new(format!("no field '{}' on record", field))
                    })
                }
                _ => Err(RuntimeError::new(format!("cannot access field '{}' on non-record", field))),
            }
        }
        Expr::Call { function, args } => {
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(eval_expr(env, arg)?);
            }
            call_function(env, function, arg_vals)
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
                    _ => Err(RuntimeError::new("cannot negate non-number")),
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
                _ => Err(RuntimeError::new("'with' requires a record")),
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
                Err(RuntimeError::new("division by zero"))
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
        _ => Err(RuntimeError::new(format!(
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
        let tokens: Vec<crate::lexer::Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        parser::parse(tokens).unwrap()
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
        let source = r#"f x:n>R n t;!"bad""#;
        let result = run_str(source, Some("f"), vec![Value::Number(0.0)]);
        assert_eq!(result, Value::Err(Box::new(Value::Text("bad".to_string()))));
    }

    #[test]
    fn interpret_match_ok_err_patterns() {
        let source = r#"f x:R n t>n;?x{!e:0;~v:v}"#;
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
    fn interpret_multi_function() {
        let source = "double x:n>n;*x 2\nf x:n>n;double x";
        let result = run_str(source, Some("f"), vec![Value::Number(5.0)]);
        assert_eq!(result, Value::Number(10.0));
    }
}
