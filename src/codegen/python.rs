use crate::ast::*;

pub fn emit(program: &Program) -> String {
    let mut out = String::new();
    for decl in &program.declarations {
        emit_decl(&mut out, decl, 0);
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("    ");
    }
}

fn emit_decl(out: &mut String, decl: &Decl, level: usize) {
    match decl {
        Decl::Function { name, params, return_type, body } => {
            indent(out, level);
            out.push_str(&format!("def {}(", py_name(name)));
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{}: {}", py_name(&p.name), emit_type(&p.ty)));
            }
            out.push_str(&format!(") -> {}:\n", emit_type(return_type)));
            emit_body(out, body, level + 1, true);
        }
        Decl::TypeDef { name, fields } => {
            indent(out, level);
            out.push_str(&format!("# type {} = {{", name));
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{}: {}", f.name, emit_type(&f.ty)));
            }
            out.push_str("}\n");
        }
        Decl::Tool { name, description, params, return_type, .. } => {
            indent(out, level);
            out.push_str(&format!("def {}(", py_name(name)));
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{}: {}", py_name(&p.name), emit_type(&p.ty)));
            }
            out.push_str(&format!(") -> {}:\n", emit_type(return_type)));
            indent(out, level + 1);
            out.push_str(&format!("\"\"\"{}\"\"\"", description));
            out.push('\n');
            indent(out, level + 1);
            out.push_str("raise NotImplementedError\n");
        }
    }
}

fn emit_body(out: &mut String, stmts: &[Stmt], level: usize, is_fn_body: bool) {
    if stmts.is_empty() {
        indent(out, level);
        out.push_str("pass\n");
        return;
    }
    for (i, stmt) in stmts.iter().enumerate() {
        let is_last = i == stmts.len() - 1;
        emit_stmt(out, stmt, level, is_fn_body && is_last);
    }
}

fn emit_stmt(out: &mut String, stmt: &Stmt, level: usize, implicit_return: bool) {
    match stmt {
        Stmt::Let { name, value } => {
            indent(out, level);
            out.push_str(&format!("{} = {}\n", py_name(name), emit_expr(value)));
        }
        Stmt::Guard { condition, negated, body } => {
            indent(out, level);
            if *negated {
                out.push_str(&format!("if not ({}):\n", emit_expr(condition)));
            } else {
                out.push_str(&format!("if {}:\n", emit_expr(condition)));
            }
            // Guard bodies in ilo typically do early returns
            emit_body(out, body, level + 1, true);
        }
        Stmt::Match { subject, arms } => {
            emit_match_stmt(out, subject, arms, level);
        }
        Stmt::ForEach { binding, collection, body } => {
            indent(out, level);
            out.push_str(&format!("for {} in {}:\n", py_name(binding), emit_expr(collection)));
            emit_body(out, body, level + 1, false);
        }
        Stmt::Expr(expr) => {
            indent(out, level);
            if implicit_return {
                out.push_str(&format!("return {}\n", emit_expr(expr)));
            } else {
                out.push_str(&format!("{}\n", emit_expr(expr)));
            }
        }
    }
}

fn emit_match_stmt(out: &mut String, subject: &Option<Expr>, arms: &[MatchArm], level: usize) {
    let subj_str = match subject {
        Some(e) => emit_expr(e),
        None => "_subject".to_string(),
    };

    // Use if/elif chain for pattern matching
    for (i, arm) in arms.iter().enumerate() {
        indent(out, level);
        let keyword = if i == 0 { "if" } else { "elif" };
        match &arm.pattern {
            Pattern::Wildcard => {
                if i == 0 {
                    // Wildcard as first arm — just emit body
                    emit_body(out, &arm.body, level, true);
                    return;
                }
                out.push_str("else:\n");
            }
            Pattern::Ok(binding) => {
                out.push_str(&format!(
                    "{} isinstance({}, tuple) and {}[0] == \"ok\":\n",
                    keyword, subj_str, subj_str
                ));
                if binding != "_" {
                    indent(out, level + 1);
                    out.push_str(&format!("{} = {}[1]\n", py_name(binding), subj_str));
                }
            }
            Pattern::Err(binding) => {
                out.push_str(&format!(
                    "{} isinstance({}, tuple) and {}[0] == \"err\":\n",
                    keyword, subj_str, subj_str
                ));
                if binding != "_" {
                    indent(out, level + 1);
                    out.push_str(&format!("{} = {}[1]\n", py_name(binding), subj_str));
                }
            }
            Pattern::Literal(lit) => {
                out.push_str(&format!(
                    "{} {} == {}:\n",
                    keyword, subj_str, emit_literal(lit)
                ));
            }
        }
        emit_body(out, &arm.body, level + 1, true);
    }
}

fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal(lit) => emit_literal(lit),
        Expr::Ref(name) => py_name(name),
        Expr::Field { object, field } => {
            format!("{}[\"{}\"]", emit_expr(object), field)
        }
        Expr::Index { object, index } => {
            format!("{}[{}]", emit_expr(object), index)
        }
        Expr::Call { function, args } => {
            if function == "num" && args.len() == 1 {
                let arg = emit_expr(&args[0]);
                return format!("(lambda s: (\"ok\", float(s)) if s.replace('.','',1).replace('-','',1).isdigit() else (\"err\", s))({})", arg);
            }
            if function == "flr" && args.len() == 1 {
                return format!("float(__import__('math').floor({}))", emit_expr(&args[0]));
            }
            if function == "cel" && args.len() == 1 {
                return format!("float(__import__('math').ceil({}))", emit_expr(&args[0]));
            }
            let args_str: Vec<String> = args.iter().map(emit_expr).collect();
            format!("{}({})", py_name(function), args_str.join(", "))
        }
        Expr::BinOp { op, left, right } => {
            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Subtract => "-",
                BinOp::Multiply => "*",
                BinOp::Divide => "/",
                BinOp::Equals => "==",
                BinOp::NotEquals => "!=",
                BinOp::GreaterThan => ">",
                BinOp::LessThan => "<",
                BinOp::GreaterOrEqual => ">=",
                BinOp::LessOrEqual => "<=",
                BinOp::And => "and",
                BinOp::Or => "or",
                BinOp::Append => {
                    return format!("({} + [{}])", emit_expr(left), emit_expr(right));
                }
            };
            format!("({} {} {})", emit_expr(left), op_str, emit_expr(right))
        }
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => format!("(not {})", emit_expr(operand)),
            UnaryOp::Negate => format!("(-{})", emit_expr(operand)),
        },
        Expr::Ok(inner) => format!("(\"ok\", {})", emit_expr(inner)),
        Expr::Err(inner) => format!("(\"err\", {})", emit_expr(inner)),
        Expr::List(items) => {
            let items_str: Vec<String> = items.iter().map(emit_expr).collect();
            format!("[{}]", items_str.join(", "))
        }
        Expr::Record { type_name, fields } => {
            let mut parts = vec![format!("\"_type\": \"{}\"", type_name)];
            for (name, val) in fields {
                parts.push(format!("\"{}\": {}", name, emit_expr(val)));
            }
            format!("{{{}}}", parts.join(", "))
        }
        Expr::Match { subject, arms } => {
            emit_match_expr(subject, arms)
        }
        Expr::With { object, updates } => {
            let mut parts = vec![format!("**{}", emit_expr(object))];
            for (name, val) in updates {
                parts.push(format!("\"{}\": {}", name, emit_expr(val)));
            }
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn emit_match_expr(subject: &Option<Box<Expr>>, arms: &[MatchArm]) -> String {
    // Emit as a chained ternary expression
    let subj = match subject {
        Some(e) => emit_expr(e),
        None => "_subject".to_string(),
    };

    let mut parts: Vec<String> = Vec::new();
    let mut default = "None".to_string();

    for arm in arms {
        let arm_val = emit_arm_value(&arm.body);
        match &arm.pattern {
            Pattern::Wildcard => {
                default = arm_val;
            }
            Pattern::Literal(lit) => {
                parts.push(format!("{} if {} == {} else", arm_val, subj, emit_literal(lit)));
            }
            Pattern::Ok(_) => {
                parts.push(format!(
                    "{} if isinstance({}, tuple) and {}[0] == \"ok\" else",
                    arm_val, subj, subj
                ));
            }
            Pattern::Err(_) => {
                parts.push(format!(
                    "{} if isinstance({}, tuple) and {}[0] == \"err\" else",
                    arm_val, subj, subj
                ));
            }
        }
    }

    if parts.is_empty() {
        return default;
    }

    // Build: val1 if cond1 else val2 if cond2 else default
    format!("({} {})", parts.join(" "), default)
}

fn emit_arm_value(body: &[Stmt]) -> String {
    if let Some(last) = body.last() {
        match last {
            Stmt::Expr(e) => emit_expr(e),
            _ => "None".to_string(),
        }
    } else {
        "None".to_string()
    }
}

fn emit_literal(lit: &Literal) -> String {
    match lit {
        Literal::Number(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Literal::Text(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            format!("\"{}\"", escaped)
        }
        Literal::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
    }
}

fn emit_type(ty: &Type) -> String {
    match ty {
        Type::Number => "float".to_string(),
        Type::Text => "str".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Nil => "None".to_string(),
        Type::List(inner) => format!("list[{}]", emit_type(inner)),
        Type::Result(ok, err) => format!("tuple[str, {} | {}]", emit_type(ok), emit_type(err)),
        Type::Named(_name) => "dict".to_string(),
    }
}

/// Convert ilo names (kebab-case) to Python (snake_case)
fn py_name(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn parse_and_emit(source: &str) -> String {
        let tokens: Vec<crate::lexer::Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        let program = parser::parse(tokens).unwrap();
        emit(&program)
    }

    fn parse_file_and_emit(path: &str) -> String {
        let source = std::fs::read_to_string(path).unwrap();
        parse_and_emit(&source)
    }

    #[test]
    fn emit_simple_function() {
        let py = parse_and_emit("tot p:n q:n r:n>n;s=*p q;t=*s r;+s t");
        assert!(py.contains("def tot(p: float, q: float, r: float) -> float:"));
        assert!(py.contains("s = (p * q)"));
        assert!(py.contains("t = (s * r)"));
        assert!(py.contains("return (s + t)"));
    }

    #[test]
    fn emit_guard() {
        let py = parse_and_emit(r#"cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze""#);
        assert!(py.contains("def cls(sp: float) -> str:"));
        assert!(py.contains("if (sp >= 1000):"));
        assert!(py.contains("return \"gold\""));
        assert!(py.contains("return \"bronze\""));
    }

    #[test]
    fn emit_ok_err() {
        let py = parse_and_emit("f x:n>R n t;~x");
        assert!(py.contains("return (\"ok\", x)"));
    }

    #[test]
    fn emit_err_expr() {
        let py = parse_and_emit(r#"f x:n>R n t;^"bad""#);
        assert!(py.contains("return (\"err\", \"bad\")"));
    }

    #[test]
    fn emit_let_binding() {
        let py = parse_and_emit("f x:n>n;y=+x 1;y");
        assert!(py.contains("y = (x + 1)"));
        assert!(py.contains("return y"));
    }

    #[test]
    fn emit_foreach() {
        let py = parse_and_emit("f xs:L n>n;@x xs{+x 1}");
        assert!(py.contains("for x in xs:"));
    }

    #[test]
    fn emit_record() {
        let py = parse_and_emit("f x:n>point;point x:x y:10");
        assert!(py.contains("\"_type\": \"point\""));
        assert!(py.contains("\"x\": x"));
        assert!(py.contains("\"y\": 10"));
    }

    #[test]
    fn emit_with() {
        let py = parse_and_emit("f x:order>order;x with total:100");
        assert!(py.contains("**x"));
        assert!(py.contains("\"total\": 100"));
    }

    #[test]
    fn emit_field_access() {
        let py = parse_and_emit("f x:order>n;x.total");
        assert!(py.contains("x[\"total\"]"));
    }

    #[test]
    fn emit_type_def() {
        let py = parse_and_emit("type point{x:n;y:n}");
        assert!(py.contains("# type point = {x: float, y: float}"));
    }

    #[test]
    fn emit_tool() {
        let py = parse_and_emit(r#"tool send-email"Send an email" to:t body:t>R _ t timeout:30,retry:3"#);
        assert!(py.contains("def send_email(to: str, body: str)"));
        assert!(py.contains("Send an email"));
        assert!(py.contains("raise NotImplementedError"));
    }

    #[test]
    fn emit_example_01() {
        let py = parse_file_and_emit("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo");
        assert!(py.contains("def tot("));
        assert!(py.contains("return (s + t)"));
    }

    #[test]
    fn emit_example_02() {
        let py = parse_file_and_emit("research/explorations/idea9-ultra-dense-short/02-with-dependencies.ilo");
        assert!(py.contains("def prc("));
    }

    #[test]
    fn emit_example_03() {
        let py = parse_file_and_emit("research/explorations/idea9-ultra-dense-short/03-data-transform.ilo");
        assert!(py.contains("def cls("));
        assert!(py.contains("def sms("));
    }

    #[test]
    fn emit_example_04() {
        let py = parse_file_and_emit("research/explorations/idea9-ultra-dense-short/04-tool-interaction.ilo");
        assert!(py.contains("def ntf("));
    }

    #[test]
    fn emit_example_05() {
        let py = parse_file_and_emit("research/explorations/idea9-ultra-dense-short/05-workflow.ilo");
        assert!(py.contains("def chk("));
    }

    #[test]
    fn emit_match_stmt() {
        let py = parse_and_emit(r#"f x:t>n;?x{"a":1;"b":2;_:0}"#);
        assert!(py.contains("if x == \"a\":"));
        assert!(py.contains("return 1"));
        assert!(py.contains("elif x == \"b\":"));
        assert!(py.contains("return 2"));
        assert!(py.contains("else:"));
        assert!(py.contains("return 0"));
    }

    #[test]
    fn emit_negated_guard() {
        let py = parse_and_emit(r#"f x:b>t;!x{"nope"};x"#);
        assert!(py.contains("if not (x):"));
        assert!(py.contains("return \"nope\""));
    }

    #[test]
    fn emit_logical_not() {
        let py = parse_and_emit("f x:b>b;!x");
        assert!(py.contains("(not x)"));
    }

    #[test]
    fn emit_kebab_to_snake() {
        let py = parse_and_emit("f>t;make-id()");
        assert!(py.contains("make_id()"));
    }

    #[test]
    fn emit_logical_and_or() {
        let py = parse_and_emit("f a:b b:b>b;&a b");
        assert!(py.contains("(a and b)"));
        let py = parse_and_emit("f a:b b:b>b;|a b");
        assert!(py.contains("(a or b)"));
    }

    #[test]
    fn emit_len_builtin() {
        let py = parse_and_emit(r#"f s:t>n;len s"#);
        assert!(py.contains("len(s)"));
    }

    #[test]
    fn emit_list_append() {
        let py = parse_and_emit("f xs:L n>L n;+=xs 1");
        assert!(py.contains("(xs + [1])"));
    }

    #[test]
    fn emit_index_access() {
        let py = parse_and_emit("f xs:L n>n;xs.0");
        assert!(py.contains("xs[0]"));
    }

    #[test]
    fn emit_str_builtin() {
        let py = parse_and_emit("f n:n>t;str n");
        assert!(py.contains("str(n)"));
    }

    #[test]
    fn emit_num_builtin() {
        let py = parse_and_emit("f s:t>R n t;num s");
        assert!(py.contains("float(s)"));
        assert!(py.contains("\"ok\""));
        assert!(py.contains("\"err\""));
    }

    #[test]
    fn emit_abs_builtin() {
        let py = parse_and_emit("f n:n>n;abs n");
        assert!(py.contains("abs(n)"));
    }

    #[test]
    fn emit_min_max_builtin() {
        let py = parse_and_emit("f a:n b:n>n;min a b");
        assert!(py.contains("min(a, b)"));
        let py = parse_and_emit("f a:n b:n>n;max a b");
        assert!(py.contains("max(a, b)"));
    }

    #[test]
    fn emit_zero_arg_call() {
        let py = parse_and_emit("f>t;make-id()");
        assert!(py.contains("make_id()"));
    }

    #[test]
    fn emit_flr_cel_builtin() {
        let py = parse_and_emit("f n:n>n;flr n");
        assert!(py.contains("__import__('math').floor(n)"));
        let py = parse_and_emit("f n:n>n;cel n");
        assert!(py.contains("__import__('math').ceil(n)"));
    }

    #[test]
    fn emit_nested_prefix() {
        // +*a b c → (a * b) + c
        let py = parse_and_emit("f a:n b:n c:n>n;+*a b c");
        assert!(py.contains("((a * b) + c)"), "got: {}", py);
    }
}
