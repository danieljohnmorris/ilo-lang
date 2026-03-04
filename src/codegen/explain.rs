/// --explain / -x: annotate a program showing the expanded (indented) code
/// with structural roles on the right.
///
/// Output format:
///
///   fac n:n>n              fn start
///      <=n 1 1             guard
///      r=fac -n 1          bind → r
///      *n r                return
///
///   fib n:n>n              fn start
///      <=n 1 n             guard
///      a=fib -n 1          bind → a
///      b=fib -n 2          bind → b
///      +a b                return
use crate::ast::{Decl, Param, Program, Stmt, Type};

pub fn explain(program: &Program) -> String {
    let source = program.source.as_deref().unwrap_or("");
    let mut out = String::new();
    let mut first = true;

    for decl in &program.declarations {
        if let Decl::Error { .. } = decl { continue; }
        if !first { out.push('\n'); }
        first = false;

        match decl {
            Decl::Function { name, params, return_type, body, .. } => {
                let sig = if params.is_empty() {
                    format!("{}>{}", name, fmt_type(return_type))
                } else {
                    format!("{} {}>{}", name, fmt_params_sig(params), fmt_type(return_type))
                };

                // Collect all (code, role, indent) lines so we can compute a shared column
                let mut lines: Vec<(String, String, usize)> = Vec::new();
                lines.push((sig, "fn start".into(), 0));
                let n = body.len();
                for (i, spanned) in body.iter().enumerate() {
                    let is_last = i == n - 1;
                    let src = extract(source, spanned.span.start, spanned.span.end).to_string();
                    let role = role_of(&spanned.node, is_last);
                    lines.push((src, role, 3));
                }

                // Comment column = max(indent + code_len) + 2 gap, minimum 22
                let col = lines.iter()
                    .map(|(code, _, indent)| indent + code.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(20) + 2;

                for (code, role, indent) in &lines {
                    out.push_str(&annotate_line_col(code, role, *indent, col));
                }
            }

            Decl::TypeDef { name, fields, .. } => {
                let fields_str = fields.iter()
                    .map(|f| format!("{}:{}", f.name, fmt_type(&f.ty)))
                    .collect::<Vec<_>>()
                    .join("; ");
                out.push_str(&annotate_line(&format!("type {name} {{{fields_str}}}"), "type def", 0));
            }

            Decl::Tool { name, params, return_type, .. } => {
                let sig = format!("@{} {}>{}", name, fmt_params_sig(params), fmt_type(return_type));
                out.push_str(&annotate_line(&sig, "tool", 0));
            }

            Decl::Alias { name, target, .. } => {
                out.push_str(&annotate_line(&format!("alias {name}={}", fmt_type(target)), "alias", 0));
            }

            Decl::Error { .. } => {}
        }
    }

    out
}

/// Format one annotated line with an explicit comment column.
fn annotate_line_col(code: &str, role: &str, indent: usize, col: usize) -> String {
    let used = indent + code.chars().count();
    let pad = if used < col { col - used } else { 1 };
    format!("{}{}{}-- {}\n", " ".repeat(indent), code, " ".repeat(pad), role)
}

/// Format a single-line decl with auto column.
fn annotate_line(code: &str, role: &str, indent: usize) -> String {
    let col = (indent + code.chars().count()).max(20) + 2;
    annotate_line_col(code, role, indent, col)
}

fn role_of(stmt: &Stmt, is_last: bool) -> String {
    match stmt {
        Stmt::Let { name, .. }        => format!("bind → {name}"),
        Stmt::Guard { negated, else_body, .. } => {
            if else_body.is_some() {
                if *negated { "ternary !".into() } else { "ternary".into() }
            } else {
                if *negated { "guard !".into() } else { "guard".into() }
            }
        }
        Stmt::Match { .. }            => "match".into(),
        Stmt::ForEach { binding, .. } => format!("foreach → {binding}"),
        Stmt::ForRange { binding, .. }=> format!("for range → {binding}"),
        Stmt::While { .. }            => "while".into(),
        Stmt::Return(_)               => "ret".into(),
        Stmt::Break(Some(_))          => "break (value)".into(),
        Stmt::Break(None)             => "break".into(),
        Stmt::Continue                => "continue".into(),
        Stmt::Destructure { bindings, .. } => format!("destructure → {}", bindings.join(", ")),
        Stmt::Expr(_) => {
            if is_last { "return".into() } else { "expr".into() }
        }
    }
}

fn fmt_params_sig(params: &[Param]) -> String {
    params.iter()
        .map(|p| format!("{}:{}", p.name, fmt_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn fmt_type(ty: &Type) -> String {
    match ty {
        Type::Number          => "n".into(),
        Type::Text            => "t".into(),
        Type::Bool            => "b".into(),
        Type::Nil             => "_".into(),
        Type::List(inner)     => format!("L {}", fmt_type(inner)),
        Type::Result(ok, err) => format!("R {} {}", fmt_type(ok), fmt_type(err)),
        Type::Named(name)     => name.clone(),
    }
}

fn extract(source: &str, start: usize, end: usize) -> &str {
    source.get(start..end).unwrap_or("?").trim()
}
