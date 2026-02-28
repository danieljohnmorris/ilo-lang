use crate::ast::*;
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("Parse error at token {position}: {message}")]
pub struct ParseError {
    pub code: &'static str,
    pub position: usize,
    pub span: Span,
    pub message: String,
}

type Result<T> = std::result::Result<T, ParseError>;

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        // Filter out newlines — idea9 uses ; as separator
        let tokens: Vec<(Token, Span)> = tokens
            .into_iter()
            .filter(|(t, _)| *t != Token::Newline)
            .collect();
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|(_, s)| *s)
            .unwrap_or(Span::UNKNOWN)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos).map(|(t, _)| t);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<Span> {
        match self.peek() {
            Some(tok) if tok == expected => {
                let span = self.peek_span();
                self.advance();
                Ok(span)
            }
            Some(tok) => Err(self.error("ILO-P003", format!("expected {:?}, got {:?}", expected, tok))),
            None => Err(self.error("ILO-P004", format!("expected {:?}, got EOF", expected))),
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(name)
            }
            Some(tok) => Err(self.error("ILO-P005", format!("expected identifier, got {:?}", tok))),
            None => Err(self.error("ILO-P006", "expected identifier, got EOF".into())),
        }
    }

    fn error(&self, code: &'static str, message: String) -> ParseError {
        ParseError {
            code,
            position: self.pos,
            span: self.peek_span(),
            message,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Check if we're at a body terminator (end of input, `}`, or end of declaration)
    fn at_body_end(&self) -> bool {
        matches!(self.peek(), None | Some(Token::RBrace))
    }

    /// Access raw token (for lookahead). Returns just the Token reference.
    fn token_at(&self, idx: usize) -> Option<&Token> {
        self.tokens.get(idx).map(|(t, _)| t)
    }

    // ---- Top-level parsing ----

    pub fn parse_program(&mut self) -> (Program, Vec<ParseError>) {
        let mut declarations = Vec::new();
        let mut errors: Vec<ParseError> = Vec::new();
        const MAX_ERRORS: usize = 20;

        while !self.at_end() {
            if errors.len() >= MAX_ERRORS {
                break;
            }
            match self.parse_decl() {
                Ok(decl) => declarations.push(decl),
                Err(e) => {
                    let err_span = e.span;
                    errors.push(e);
                    let end_span = self.sync_to_decl_boundary();
                    declarations.push(Decl::Error { span: err_span.merge(end_span) });
                }
            }
        }

        (Program { declarations, source: None }, errors)
    }

    /// Return true if the tokens at `pos` look like the start of a function declaration:
    /// `Ident` followed by `>` (no-param function) OR `Ident Ident :` (has params).
    fn is_fn_decl_start(&self, pos: usize) -> bool {
        if !matches!(self.token_at(pos), Some(Token::Ident(_))) {
            return false;
        }
        match self.token_at(pos + 1) {
            // name>return — zero-param function
            Some(Token::Greater) => true,
            // name param:type ... — has params
            Some(Token::Ident(_)) => matches!(self.token_at(pos + 2), Some(Token::Colon)),
            _ => false,
        }
    }

    /// Advance past tokens until we reach what looks like the start of the next
    /// declaration (or EOF). Returns the span of the last token consumed.
    /// Tracks brace depth so nested `{…}` blocks are skipped atomically.
    fn sync_to_decl_boundary(&mut self) -> Span {
        let mut depth: usize = 0;
        let mut last_span = self.peek_span();

        loop {
            match self.peek() {
                None => break,
                Some(Token::LBrace) => {
                    depth += 1;
                    last_span = self.peek_span();
                    self.advance();
                }
                Some(Token::RBrace) => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    last_span = self.peek_span();
                    self.advance();
                }
                // Unambiguous declaration starters
                Some(Token::Type) | Some(Token::Tool) if depth == 0 => break,
                // An identifier that looks like a function header
                _ if depth == 0 && self.is_fn_decl_start(self.pos) => break,
                _ => {
                    last_span = self.peek_span();
                    self.advance();
                }
            }
        }

        last_span
    }

    fn parse_decl(&mut self) -> Result<Decl> {
        match self.peek() {
            Some(Token::Type) => self.parse_type_decl(),
            Some(Token::Tool) => self.parse_tool_decl(),
            Some(Token::Ident(_)) => self.parse_fn_decl(),
            Some(tok) => {
                let hint = if matches!(tok,
                    Token::Plus | Token::Minus | Token::Star | Token::Slash
                    | Token::Greater | Token::Less | Token::GreaterEq | Token::LessEq
                    | Token::Eq | Token::NotEq | Token::Amp | Token::Pipe
                    | Token::Bang | Token::Tilde | Token::Caret
                ) {
                    "\n  hint: prefix operators can't start a declaration.\n        Bind call results to variables: r=fac -n 1;*n r"
                } else {
                    ""
                };
                Err(self.error("ILO-P001", format!("expected declaration, got {:?}{}", tok, hint)))
            }
            None => Err(self.error("ILO-P002", "expected declaration, got EOF".into())),
        }
    }

    /// `type name{field:type;...}`
    fn parse_type_decl(&mut self) -> Result<Decl> {
        let start = self.peek_span();
        self.expect(&Token::Type)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        while self.peek() != Some(&Token::RBrace) {
            if !fields.is_empty() {
                self.expect(&Token::Semi)?;
            }
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Param { name: fname, ty });
        }
        let end = self.peek_span();
        self.expect(&Token::RBrace)?;
        Ok(Decl::TypeDef { name, fields, span: start.merge(end) })
    }

    /// `tool name"desc" params>return timeout:n,retry:n`
    fn parse_tool_decl(&mut self) -> Result<Decl> {
        let start = self.peek_span();
        self.expect(&Token::Tool)?;
        let name = self.expect_ident()?;
        let description = match self.peek().cloned() {
            Some(Token::Text(s)) => {
                self.advance();
                s
            }
            _ => return Err(self.error("ILO-P015", "expected tool description string".into())),
        };
        let params = self.parse_params()?;
        self.expect(&Token::Greater)?;
        let return_type = self.parse_type()?;

        let mut timeout = None;
        let mut retry = None;

        // Parse optional tool options: timeout:n,retry:n
        while matches!(self.peek(), Some(Token::Timeout) | Some(Token::Retry)) {
            match self.peek() {
                Some(Token::Timeout) => {
                    self.advance();
                    self.expect(&Token::Colon)?;
                    timeout = Some(self.parse_number()?);
                }
                Some(Token::Retry) => {
                    self.advance();
                    self.expect(&Token::Colon)?;
                    retry = Some(self.parse_number()?);
                }
                _ => break,
            }
            if self.peek() == Some(&Token::Comma) {
                self.advance();
            }
        }

        // End span: last consumed token
        let end_span = self.prev_span();

        Ok(Decl::Tool {
            name,
            description,
            params,
            return_type,
            timeout,
            retry,
            span: start.merge(end_span),
        })
    }

    /// `name params>return;body`
    fn parse_fn_decl(&mut self) -> Result<Decl> {
        let start = self.peek_span();
        let name = self.expect_ident()?;
        let params = self.parse_params()?;
        self.expect(&Token::Greater)?;
        let return_type = self.parse_type()?;
        self.expect(&Token::Semi)?;
        let body = self.parse_body()?;
        let end = self.prev_span();
        Ok(Decl::Function {
            name,
            params,
            return_type,
            body,
            span: start.merge(end),
        })
    }

    /// Span of the previously consumed token.
    fn prev_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].1
        } else {
            Span::UNKNOWN
        }
    }

    // ---- Types ----

    fn parse_type(&mut self) -> Result<Type> {
        match self.peek().cloned() {
            Some(Token::Ident(ref s)) if s == "n" => {
                self.advance();
                Ok(Type::Number)
            }
            Some(Token::Ident(ref s)) if s == "t" => {
                self.advance();
                Ok(Type::Text)
            }
            Some(Token::Ident(ref s)) if s == "b" => {
                self.advance();
                Ok(Type::Bool)
            }
            Some(Token::Underscore) => {
                self.advance();
                Ok(Type::Nil)
            }
            Some(Token::ListType) => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::List(Box::new(inner)))
            }
            Some(Token::ResultType) => {
                self.advance();
                let ok_type = self.parse_type()?;
                let err_type = self.parse_type()?;
                Ok(Type::Result(Box::new(ok_type), Box::new(err_type)))
            }
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(Type::Named(name))
            }
            Some(tok) => Err(self.error("ILO-P007", format!("expected type, got {:?}", tok))),
            None => Err(self.error("ILO-P008", "expected type, got EOF".into())),
        }
    }

    /// Parse parameter list: `name:type name:type ...`
    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        while let Some(Token::Ident(_)) = self.peek() {
            // Look ahead for colon to distinguish params from other constructs
            if self.pos + 1 < self.tokens.len() && self.token_at(self.pos + 1) == Some(&Token::Colon) {
                let name = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let ty = self.parse_type()?;
                params.push(Param { name, ty });
            } else {
                break;
            }
        }
        Ok(params)
    }

    // ---- Body & Statements ----

    /// Parse a semicolon-separated body
    fn parse_body(&mut self) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        if !self.at_body_end() {
            stmts.push(self.parse_stmt()?);
            while self.peek() == Some(&Token::Semi) {
                self.advance();
                if self.at_body_end() {
                    break;
                }
                stmts.push(self.parse_stmt()?);
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        match self.peek() {
            Some(Token::Question) => self.parse_match_stmt(),
            Some(Token::At) => self.parse_foreach(),
            Some(Token::Ident(_)) => {
                // Check for let binding: ident '='
                if self.pos + 1 < self.tokens.len() && self.token_at(self.pos + 1) == Some(&Token::Eq) {
                    self.parse_let()
                } else {
                    // Could be a guard or an expression statement
                    self.parse_expr_or_guard()
                }
            }
            Some(Token::Bang) => {
                // !cond{body} — negated guard
                self.parse_bang_stmt()
            }
            Some(Token::Caret) => {
                // ^expr — Err constructor as statement
                self.parse_caret_stmt()
            }
            _ => {
                let expr = self.parse_expr()?;
                // Check if this is a guard: expr followed by {
                if self.peek() == Some(&Token::LBrace) {
                    let body = self.parse_brace_body()?;
                    Ok(Stmt::Guard {
                        condition: expr,
                        negated: false,
                        body,
                    })
                } else {
                    Ok(Stmt::Expr(expr))
                }
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt> {
        let name = self.expect_ident()?;
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Let { name, value })
    }

    /// `?{arms}` or `?expr{arms}`
    fn parse_match_stmt(&mut self) -> Result<Stmt> {
        self.expect(&Token::Question)?;
        let subject = if self.peek() == Some(&Token::LBrace) {
            None
        } else {
            Some(self.parse_atom()?)
        };
        self.expect(&Token::LBrace)?;
        let arms = self.parse_match_arms()?;
        self.expect(&Token::RBrace)?;
        Ok(Stmt::Match { subject, arms })
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>> {
        let mut arms = Vec::new();
        while self.peek() != Some(&Token::RBrace) {
            if !arms.is_empty() {
                self.expect(&Token::Semi)?;
                if self.peek() == Some(&Token::RBrace) {
                    break;
                }
            }
            arms.push(self.parse_match_arm()?);
        }
        Ok(arms)
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm> {
        let pattern = self.parse_pattern()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_arm_body()?;
        Ok(MatchArm { pattern, body })
    }

    /// Parse body of a match arm — multiple statements until next arm pattern or `}`
    fn parse_arm_body(&mut self) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        if !self.at_arm_end() {
            stmts.push(self.parse_stmt()?);
            // Continue consuming statements if `;` is followed by non-pattern content
            while self.peek() == Some(&Token::Semi) && !self.semi_starts_new_arm() {
                self.advance(); // consume ;
                if self.at_arm_end() {
                    break;
                }
                stmts.push(self.parse_stmt()?);
            }
        }
        Ok(stmts)
    }

    /// Check if the `;` at current position starts a new match arm.
    /// A new arm starts with a pattern followed by `:`.
    fn semi_starts_new_arm(&self) -> bool {
        if self.peek() != Some(&Token::Semi) {
            return false;
        }
        // Look past the `;`
        let after_semi = self.pos + 1;
        if after_semi >= self.tokens.len() {
            return false;
        }
        match self.token_at(after_semi) {
            // ^ident: or ^_: → err pattern
            Some(Token::Caret) => {
                if after_semi + 2 < self.tokens.len() {
                    matches!(
                        (self.token_at(after_semi + 1), self.token_at(after_semi + 2)),
                        (Some(Token::Ident(_) | Token::Underscore), Some(Token::Colon))
                    )
                } else {
                    false
                }
            }
            // ~ident: or ~_: → ok pattern
            Some(Token::Tilde) => {
                if after_semi + 2 < self.tokens.len() {
                    matches!(
                        (self.token_at(after_semi + 1), self.token_at(after_semi + 2)),
                        (Some(Token::Ident(_) | Token::Underscore), Some(Token::Colon))
                    )
                } else {
                    false
                }
            }
            // _: → wildcard
            Some(Token::Underscore) => {
                after_semi + 1 < self.tokens.len()
                    && self.token_at(after_semi + 1) == Some(&Token::Colon)
            }
            // literal: → literal pattern (number, string, bool)
            Some(Token::Number(_) | Token::Text(_) | Token::True | Token::False) => {
                after_semi + 1 < self.tokens.len()
                    && self.token_at(after_semi + 1) == Some(&Token::Colon)
            }
            _ => false,
        }
    }

    fn at_arm_end(&self) -> bool {
        matches!(self.peek(), None | Some(Token::RBrace) | Some(Token::Semi))
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        match self.peek() {
            Some(Token::Caret) => {
                self.advance();
                let name = match self.peek() {
                    Some(Token::Underscore) => {
                        self.advance();
                        "_".to_string()
                    }
                    _ => self.expect_ident()?,
                };
                Ok(Pattern::Err(name))
            }
            Some(Token::Tilde) => {
                self.advance();
                let name = match self.peek() {
                    Some(Token::Underscore) => {
                        self.advance();
                        "_".to_string()
                    }
                    _ => self.expect_ident()?,
                };
                Ok(Pattern::Ok(name))
            }
            Some(Token::Underscore) => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            Some(Token::Number(_)) => {
                if let Some(Token::Number(n)) = self.advance().cloned() {
                    Ok(Pattern::Literal(Literal::Number(n)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::Text(_)) => {
                if let Some(Token::Text(s)) = self.advance().cloned() {
                    Ok(Pattern::Literal(Literal::Text(s)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::True) => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(true)))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(false)))
            }
            Some(tok) => Err(self.error("ILO-P011", format!("expected pattern, got {:?}", tok))),
            None => Err(self.error("ILO-P012", "expected pattern, got EOF".into())),
        }
    }

    /// `@binding collection{body}`
    fn parse_foreach(&mut self) -> Result<Stmt> {
        self.expect(&Token::At)?;
        let binding = self.expect_ident()?;
        let collection = self.parse_atom()?;
        let body = self.parse_brace_body()?;
        Ok(Stmt::ForEach {
            binding,
            collection,
            body,
        })
    }

    /// Parse `!` at statement position — negated guard `!cond{body}` or logical NOT `!expr`
    fn parse_bang_stmt(&mut self) -> Result<Stmt> {
        self.expect(&Token::Bang)?;
        let inner = self.parse_expr_inner()?;

        if self.peek() == Some(&Token::LBrace) {
            // Negated guard: !cond{body}
            let body = self.parse_brace_body()?;
            Ok(Stmt::Guard {
                condition: inner,
                negated: true,
                body,
            })
        } else {
            // Logical NOT as expression statement: !expr
            Ok(Stmt::Expr(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(inner),
            }))
        }
    }

    /// Parse `^` at statement position — Err constructor: `^expr`
    fn parse_caret_stmt(&mut self) -> Result<Stmt> {
        self.expect(&Token::Caret)?;
        let inner = self.parse_expr_inner()?;
        Ok(Stmt::Expr(Expr::Err(Box::new(inner))))
    }

    /// Parse ident-starting statement — could be guard (expr{body}) or expr statement
    fn parse_expr_or_guard(&mut self) -> Result<Stmt> {
        let expr = self.parse_expr()?;
        if self.peek() == Some(&Token::LBrace) {
            let body = self.parse_brace_body()?;
            Ok(Stmt::Guard {
                condition: expr,
                negated: false,
                body,
            })
        } else {
            Ok(Stmt::Expr(expr))
        }
    }

    fn parse_brace_body(&mut self) -> Result<Vec<Stmt>> {
        self.expect(&Token::LBrace)?;
        let body = self.parse_body()?;
        self.expect(&Token::RBrace)?;
        Ok(body)
    }

    // ---- Expressions ----

    fn parse_expr(&mut self) -> Result<Expr> {
        let expr = match self.peek() {
            Some(Token::Tilde) => {
                self.advance();
                let inner = self.parse_expr_inner()?;
                Expr::Ok(Box::new(inner))
            }
            Some(Token::Caret) => {
                self.advance();
                let inner = self.parse_expr_inner()?;
                Expr::Err(Box::new(inner))
            }
            _ => self.parse_expr_inner()?,
        };
        self.maybe_with(expr)
    }

    /// Parse expression, possibly followed by `with`
    fn maybe_with(&mut self, expr: Expr) -> Result<Expr> {
        if matches!(self.peek(), Some(Token::With)) {
            self.advance();
            let mut updates = Vec::new();
            while let Some(Token::Ident(_)) = self.peek() {
                if self.pos + 1 < self.tokens.len() && self.token_at(self.pos + 1) == Some(&Token::Colon) {
                    let name = self.expect_ident()?;
                    self.expect(&Token::Colon)?;
                    let value = self.parse_atom()?;
                    updates.push((name, value));
                } else {
                    break;
                }
            }
            Ok(Expr::With {
                object: Box::new(expr),
                updates,
            })
        } else {
            Ok(expr)
        }
    }

    /// Core expression parsing — handles prefix ops, match expr, calls, atoms
    fn parse_expr_inner(&mut self) -> Result<Expr> {
        match self.peek() {
            // Minus is special: could be unary negation (-x) or binary subtract (-a b)
            Some(Token::Minus) => self.parse_minus(),
            // Logical NOT: !x
            Some(Token::Bang) => {
                self.advance();
                let operand = self.parse_operand()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                })
            }
            // Prefix binary operators: +a b, *a b, etc.
            Some(Token::Plus) | Some(Token::Star) | Some(Token::Slash)
            | Some(Token::Greater) | Some(Token::Less) | Some(Token::GreaterEq)
            | Some(Token::LessEq) | Some(Token::Eq) | Some(Token::NotEq)
            | Some(Token::Amp) | Some(Token::Pipe)
            | Some(Token::PlusEq) => {
                self.parse_prefix_binop()
            }
            // Match expression: ?expr{...} or ?{...}
            Some(Token::Question) => self.parse_match_expr(),
            _ => self.parse_call_or_atom(),
        }
    }

    /// Parse match as expression: `?expr{arms}` or `?{arms}`
    fn parse_match_expr(&mut self) -> Result<Expr> {
        self.expect(&Token::Question)?;
        let subject = if self.peek() == Some(&Token::LBrace) {
            None
        } else {
            Some(Box::new(self.parse_atom()?))
        };
        self.expect(&Token::LBrace)?;
        let arms = self.parse_match_arms()?;
        self.expect(&Token::RBrace)?;
        Ok(Expr::Match { subject, arms })
    }

    /// Parse `-`: unary negation (`-x`) when one atom follows,
    /// binary subtract (`-a b`) when two atoms follow.
    fn parse_minus(&mut self) -> Result<Expr> {
        self.advance(); // consume `-`
        let first = self.parse_operand()?;
        if self.can_start_operand() {
            let second = self.parse_operand()?;
            Ok(Expr::BinOp {
                op: BinOp::Subtract,
                left: Box::new(first),
                right: Box::new(second),
            })
        } else {
            Ok(Expr::UnaryOp {
                op: UnaryOp::Negate,
                operand: Box::new(first),
            })
        }
    }

    fn parse_prefix_binop(&mut self) -> Result<Expr> {
        let op = match self.advance() {
            Some(Token::Plus) => BinOp::Add,
            Some(Token::Minus) => BinOp::Subtract,
            Some(Token::Star) => BinOp::Multiply,
            Some(Token::Slash) => BinOp::Divide,
            Some(Token::Greater) => BinOp::GreaterThan,
            Some(Token::Less) => BinOp::LessThan,
            Some(Token::GreaterEq) => BinOp::GreaterOrEqual,
            Some(Token::LessEq) => BinOp::LessOrEqual,
            Some(Token::Eq) => BinOp::Equals,
            Some(Token::NotEq) => BinOp::NotEquals,
            Some(Token::Amp) => BinOp::And,
            Some(Token::Pipe) => BinOp::Or,
            Some(Token::PlusEq) => BinOp::Append,
            _ => unreachable!(),
        };
        let left = self.parse_operand()?;
        let right = self.parse_operand()?;
        Ok(Expr::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    /// Parse function call or plain atom
    /// call = IDENT atom+ (greedy, when not a record)
    /// Also handles zero-arg calls: `func()`
    fn parse_call_or_atom(&mut self) -> Result<Expr> {
        let atom = self.parse_atom()?;

        // If atom is a Ref, check if it's a call or record construction
        if let Expr::Ref(ref name) = atom {
            let name = name.clone();

            // Check for zero-arg call: name()
            if self.peek() == Some(&Token::LParen)
                && self.pos + 1 < self.tokens.len()
                && self.token_at(self.pos + 1) == Some(&Token::RParen)
            {
                self.advance(); // (
                self.advance(); // )
                return Ok(Expr::Call {
                    function: name,
                    args: vec![],
                });
            }

            // Check for record construction: name field:value
            if self.is_named_field_ahead() {
                return self.parse_record(name);
            }

            // Check for function call: name followed by args
            // Use can_start_operand/parse_operand so prefix expressions work as args:
            //   fac -n 1  →  Call(fac, [Subtract(n, 1)])
            if self.can_start_operand() {
                let mut args = Vec::new();
                while self.can_start_operand() {
                    args.push(self.parse_operand()?);
                }
                return Ok(Expr::Call {
                    function: name,
                    args,
                });
            }
        }

        Ok(atom)
    }

    /// Check if next tokens look like `ident:expr` (named field)
    fn is_named_field_ahead(&self) -> bool {
        if let Some(Token::Ident(_)) = self.peek()
            && self.pos + 1 < self.tokens.len() && self.token_at(self.pos + 1) == Some(&Token::Colon) {
                // Make sure it's not a param pattern (type follows colon)
                return true;
            }
        false
    }

    /// Parse record: `typename field:val field:val`
    fn parse_record(&mut self, type_name: String) -> Result<Expr> {
        let mut fields = Vec::new();
        while self.is_named_field_ahead() {
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let value = self.parse_atom()?;
            fields.push((fname, value));
        }
        Ok(Expr::Record { type_name, fields })
    }

    /// Can the current token start an atom?
    fn can_start_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Ident(_))
                | Some(Token::Number(_))
                | Some(Token::Text(_))
                | Some(Token::True)
                | Some(Token::False)
                | Some(Token::Underscore)
                | Some(Token::LParen)
                | Some(Token::LBracket)
        )
    }

    /// Can the next token start an operand? (atom or prefix operator)
    fn can_start_operand(&self) -> bool {
        self.can_start_atom()
            || matches!(
                self.peek(),
                Some(Token::Plus)
                    | Some(Token::Minus)
                    | Some(Token::Star)
                    | Some(Token::Slash)
                    | Some(Token::Greater)
                    | Some(Token::Less)
                    | Some(Token::GreaterEq)
                    | Some(Token::LessEq)
                    | Some(Token::Eq)
                    | Some(Token::NotEq)
                    | Some(Token::Amp)
                    | Some(Token::Pipe)
                    | Some(Token::PlusEq)
                    | Some(Token::Bang)
                    | Some(Token::Tilde)
                    | Some(Token::Caret)
            )
    }

    /// Parse an operand — an atom or a nested prefix operator.
    /// This sits between `parse_atom` (terminals only) and `parse_expr_inner`
    /// (which includes function calls). Prefix operators use this so that
    /// `+*a b c` works without greedy call parsing.
    fn parse_operand(&mut self) -> Result<Expr> {
        match self.peek() {
            Some(Token::Plus) | Some(Token::Star) | Some(Token::Slash)
            | Some(Token::Greater) | Some(Token::Less) | Some(Token::GreaterEq)
            | Some(Token::LessEq) | Some(Token::Eq) | Some(Token::NotEq)
            | Some(Token::Amp) | Some(Token::Pipe)
            | Some(Token::PlusEq) => self.parse_prefix_binop(),
            Some(Token::Minus) => self.parse_minus(),
            Some(Token::Bang) => {
                self.advance();
                let operand = self.parse_operand()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                })
            }
            Some(Token::Tilde) => {
                self.advance();
                let inner = self.parse_operand()?;
                Ok(Expr::Ok(Box::new(inner)))
            }
            Some(Token::Caret) => {
                self.advance();
                let inner = self.parse_operand()?;
                Ok(Expr::Err(Box::new(inner)))
            }
            _ => self.parse_atom(),
        }
    }

    /// Parse an atom — the smallest expression unit
    fn parse_atom(&mut self) -> Result<Expr> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(Expr::Literal(Literal::Number(n)))
            }
            Some(Token::Text(s)) => {
                self.advance();
                Ok(Expr::Literal(Literal::Text(s)))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            Some(Token::Underscore) => {
                self.advance();
                Ok(Expr::Ref("_".to_string()))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                if self.peek() != Some(&Token::RBracket) {
                    items.push(self.parse_expr()?);
                    while self.peek() == Some(&Token::Comma) {
                        self.advance();
                        if self.peek() == Some(&Token::RBracket) {
                            break; // trailing comma
                        }
                        items.push(self.parse_expr()?);
                    }
                }
                self.expect(&Token::RBracket)?;
                Ok(Expr::List(items))
            }
            Some(Token::Ident(name)) => {
                self.advance();
                // Check for field access chain: ident.field.field...
                let mut expr = Expr::Ref(name);
                while self.peek() == Some(&Token::Dot) {
                    self.advance();
                    match self.peek().cloned() {
                        Some(Token::Number(n)) if n.fract() == 0.0 && n >= 0.0 => {
                            self.advance();
                            expr = Expr::Index {
                                object: Box::new(expr),
                                index: n as usize,
                            };
                        }
                        _ => {
                            let field = self.expect_ident()?;
                            expr = Expr::Field {
                                object: Box::new(expr),
                                field,
                            };
                        }
                    }
                }
                Ok(expr)
            }
            Some(tok) => Err(self.error("ILO-P009", format!("expected expression, got {:?}", tok))),
            None => Err(self.error("ILO-P010", "expected expression, got EOF".into())),
        }
    }

    fn parse_number(&mut self) -> Result<f64> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(n)
            }
            Some(tok) => Err(self.error("ILO-P013", format!("expected number, got {:?}", tok))),
            None => Err(self.error("ILO-P014", "expected number, got EOF".into())),
        }
    }
}

/// Parse from token+span pairs.
/// Returns `(program, errors)`. The program may contain `Decl::Error` poison nodes
/// for declarations that failed to parse. Check `errors.is_empty()` before using
/// the program for execution — error nodes are skipped by the verifier but not
/// by the backends.
pub fn parse(tokens: Vec<(Token, Span)>) -> (Program, Vec<ParseError>) {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

/// Parse from bare tokens (no span information, UNKNOWN spans).
/// Returns `Err` if any parse errors are present (first error).
/// Used by test helpers in interpreter, vm, and codegen modules.
#[cfg(test)]
pub fn parse_tokens(tokens: Vec<Token>) -> std::result::Result<Program, Vec<ParseError>> {
    let pairs: Vec<(Token, Span)> = tokens
        .into_iter()
        .map(|t| (t, Span::UNKNOWN))
        .collect();
    let (prog, errors) = parse(pairs);
    if errors.is_empty() { Ok(prog) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_str(source: &str) -> Program {
        let tokens = lexer::lex(source).unwrap();
        let token_spans: Vec<(Token, Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
            .collect();
        let (prog, errors) = parse(token_spans);
        assert!(errors.is_empty(), "parse errors: {:?}", errors);
        prog
    }

    fn parse_str_errors(source: &str) -> (Program, Vec<ParseError>) {
        let tokens = lexer::lex(source).unwrap();
        let token_spans: Vec<(Token, Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
            .collect();
        parse(token_spans)
    }

    fn parse_file(path: &str) -> Program {
        let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
        parse_str(&source)
    }

    #[test]
    fn parse_simple_function() {
        // tot p:n q:n r:n>n;s=*p q;t=*s r;+s t
        let prog = parse_str("tot p:n q:n r:n>n;s=*p q;t=*s r;+s t");
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Decl::Function { name, params, body, .. } => {
                assert_eq!(name, "tot");
                assert_eq!(params.len(), 3);
                assert_eq!(body.len(), 3); // s=..., t=..., +s t
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_let_binding() {
        let prog = parse_str("f x:n>n;y=+x 1;y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 2);
                match &body[0] {
                    Stmt::Let { name, .. } => assert_eq!(name, "y"),
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_type_def() {
        let prog = parse_str("type point{x:n;y:n}");
        match &prog.declarations[0] {
            Decl::TypeDef { name, fields, .. } => {
                assert_eq!(name, "point");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("expected type def"),
        }
    }

    #[test]
    fn parse_guard() {
        let prog = parse_str(r#"cls sp:n>t;>=sp 1000{"gold"};"bronze""#);
        match &prog.declarations[0] {
            Decl::Function { name, body, .. } => {
                assert_eq!(name, "cls");
                assert!(body.len() >= 2);
                match &body[0] {
                    Stmt::Guard { negated, .. } => assert!(!negated),
                    _ => panic!("expected guard, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_stmt() {
        let prog = parse_str(r#"f x:n>t;?{^e:^"error";~v:v;_:"default"}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { subject, arms } => {
                        assert!(subject.is_none());
                        assert_eq!(arms.len(), 3);
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_ok_err_exprs() {
        let prog = parse_str("f x:n>R n t;~x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Ok(_)) => {}
                    _ => panic!("expected Ok expr"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_foreach() {
        let prog = parse_str("f xs:L n>n;s=0;@x xs{s=+s x};s");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert!(body.len() >= 3);
                match &body[1] {
                    Stmt::ForEach { binding, .. } => assert_eq!(binding, "x"),
                    _ => panic!("expected foreach"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_multi_decl() {
        let prog = parse_str("f x:n>n;*x 2 g x:n>n;+x 1");
        assert_eq!(prog.declarations.len(), 2);
    }

    #[test]
    fn parse_nested_prefix() {
        let prog = parse_str("f a:n b:n c:n>n;+*a b c");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op: BinOp::Add, left, .. }) => {
                        assert!(matches!(**left, Expr::BinOp { op: BinOp::Multiply, .. }));
                    }
                    _ => panic!("expected binop"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_list_literal() {
        let prog = parse_str("f x:n>L n;[x, *x 2, *x 3]");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::List(items)) => assert_eq!(items.len(), 3),
                    _ => panic!("expected list"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_field_access() {
        let prog = parse_str("f p:point>n;p.x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Field { field, .. }) => assert_eq!(field, "x"),
                    _ => panic!("expected field access"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_index_access() {
        let prog = parse_str("f xs:L n>n;xs.0");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Index { index, .. }) => assert_eq!(*index, 0),
                    _ => panic!("expected index access"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_negated_guard() {
        let prog = parse_str(r#"f x:b>t;!x{"yes"};"no""#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { negated, .. } => assert!(negated),
                    _ => panic!("expected guard"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_record_construction() {
        let prog = parse_str("type point{x:n;y:n} f a:n b:n>point;point x:a y:b");
        match &prog.declarations[1] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Record { type_name, fields }) => {
                        assert_eq!(type_name, "point");
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected record"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_with_expr() {
        let prog = parse_str("type point{x:n;y:n} f p:point>point;p with x:1 y:2");
        match &prog.declarations[1] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::With { updates, .. }) => {
                        assert_eq!(updates.len(), 2);
                    }
                    _ => panic!("expected with expr"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_tool_decl() {
        let prog = parse_str(r#"tool fetch"http get" url:t>t timeout:30,retry:3"#);
        match &prog.declarations[0] {
            Decl::Tool { name, description, timeout, retry, .. } => {
                assert_eq!(name, "fetch");
                assert_eq!(description, "http get");
                assert_eq!(*timeout, Some(30.0));
                assert_eq!(*retry, Some(3.0));
            }
            _ => panic!("expected tool"),
        }
    }

    #[test]
    fn parse_match_with_subject() {
        let prog = parse_str("f x:R n t>n;?x{~v:v;^e:0}");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { subject, arms } => {
                        assert!(subject.is_some());
                        assert_eq!(arms.len(), 2);
                    }
                    _ => panic!("expected match stmt"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_expr_in_let() {
        let prog = parse_str(r#"f x:R n t>n;r=?x{~v:v;^e:0};r"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 2);
                match &body[0] {
                    Stmt::Let { value: Expr::Match { .. }, .. } => {}
                    _ => panic!("expected let with match expr, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_call_with_prefix_arg() {
        // fac -n 1 should parse as Call(fac, [Subtract(n, 1)])
        let prog = parse_str("fac n:n>n;r=fac -n 1;*n r");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { value: Expr::Call { function, args }, .. } => {
                        assert_eq!(function, "fac");
                        assert_eq!(args.len(), 1);
                        assert!(matches!(&args[0], Expr::BinOp { op: BinOp::Subtract, .. }));
                    }
                    _ => panic!("expected call with prefix arg"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_zero_arg_call() {
        let prog = parse_str("f>n;g() g>n;42");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Call { function, args }) => {
                        assert_eq!(function, "g");
                        assert!(args.is_empty());
                    }
                    _ => panic!("expected zero-arg call"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_paren_expr() {
        let prog = parse_str("f x:n>n;*(+x 1) 2");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op: BinOp::Multiply, left, .. }) => {
                        assert!(matches!(**left, Expr::BinOp { op: BinOp::Add, .. }));
                    }
                    _ => panic!("expected binop"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_list_append() {
        let prog = parse_str("f xs:L n x:n>L n;+=xs x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op: BinOp::Append, .. }) => {}
                    _ => panic!("expected append"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_trailing_comma_in_list() {
        let prog = parse_str("f>L n;[1, 2, 3,]");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::List(items)) => assert_eq!(items.len(), 3),
                    _ => panic!("expected list"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_empty_list() {
        let prog = parse_str("f>L n;[]");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::List(items)) => assert!(items.is_empty()),
                    _ => panic!("expected list"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_caret_stmt_in_match() {
        let prog = parse_str(r#"f x:R n t>n;?x{^e:^"error";~v:v;_:0}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        match &arms[0].body[0] {
                            Stmt::Expr(Expr::Err(_)) => {}
                            _ => panic!("expected Err expr in first arm"),
                        }
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_chained_field_access() {
        let prog = parse_str("type inner{v:n} type outer{i:inner} f o:outer>n;o.i.v");
        // Should parse as o.i.v (chained field access)
        match &prog.declarations[2] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Field { object, field }) => {
                        assert_eq!(field, "v");
                        assert!(matches!(**object, Expr::Field { .. }));
                    }
                    _ => panic!("expected chained field"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_multi_stmt_match_arm() {
        let prog = parse_str("f x:R n t>n;?x{~v:y=+v 1;*y 2;^e:0}");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms[0].body.len(), 2); // y=+v 1, *y 2
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_negated_guard_vs_not_expr() {
        // !x{body} is negated guard; !x as last stmt is logical NOT
        let prog = parse_str("f x:b>b;!x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::UnaryOp { op: UnaryOp::Not, .. }) => {}
                    _ => panic!("expected NOT expr, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_bool_literals() {
        let prog = parse_str("f x:b>n;?x{true:1;false:0}");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert!(matches!(arms[0].pattern, Pattern::Literal(Literal::Bool(true))));
                        assert!(matches!(arms[1].pattern, Pattern::Literal(Literal::Bool(false))));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_number_with_wildcard() {
        let prog = parse_str(r#"f x:n>t;?x{1:"one";2:"two";_:"other"}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 3);
                        assert!(matches!(arms[2].pattern, Pattern::Wildcard));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_string_patterns() {
        let prog = parse_str(r#"f x:t>n;?x{"a":1;"b":2;_:0}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 3);
                        assert!(matches!(&arms[0].pattern, Pattern::Literal(Literal::Text(s)) if s == "a"));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_all_comparison_ops() {
        // Each op produces a different BinOp
        let tests = vec![
            (">=a b", BinOp::GreaterOrEqual),
            ("<=a b", BinOp::LessOrEqual),
            ("!=a b", BinOp::NotEquals),
            ("=a b", BinOp::Equals),
            (">a b", BinOp::GreaterThan),
            ("<a b", BinOp::LessThan),
            ("&a b", BinOp::And),
            ("|a b", BinOp::Or),
        ];
        for (expr_str, expected_op) in tests {
            let code = format!("f a:b b:b>b;{}", expr_str);
            let prog = parse_str(&code);
            match &prog.declarations[0] {
                Decl::Function { body, .. } => {
                    match &body[0] {
                        Stmt::Expr(Expr::BinOp { op, .. }) => {
                            assert_eq!(*op, expected_op, "failed for expr: {}", expr_str);
                        }
                        _ => panic!("expected binop for {}", expr_str),
                    }
                }
                _ => panic!("expected function"),
            }
        }
    }

    #[test]
    fn parse_error_has_span() {
        // "f x:n>n;+" — the + at byte 8 triggers an error because no operands follow
        let source = "f x:n>n;+";
        let tokens = lexer::lex(source).unwrap();
        let token_spans: Vec<(Token, Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
            .collect();
        let (_prog, errors) = parse(token_spans);
        let err = errors.into_iter().next().expect("expected parse error");
        // Error message should mention the problem
        assert!(!err.message.is_empty());
        // Position should be non-zero (error is after the initial tokens)
        assert!(err.position > 0, "error position should be > 0");
    }

    // ---- Span-specific tests ----

    #[test]
    fn fn_decl_span_covers_full_declaration() {
        let prog = parse_str("f x:n>n;*x 2");
        match &prog.declarations[0] {
            Decl::Function { span, .. } => {
                assert_eq!(span.start, 0);
                assert!(span.end > 0, "function span end should be > 0");
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn type_decl_span_covers_full_declaration() {
        let prog = parse_str("type point{x:n;y:n}");
        match &prog.declarations[0] {
            Decl::TypeDef { span, .. } => {
                assert_eq!(span.start, 0);
                // Should extend to cover the closing }
                assert!(span.end >= 18, "type span end should cover closing brace, got {}", span.end);
            }
            _ => panic!("expected type def"),
        }
    }

    #[test]
    fn multi_decl_spans_are_distinct() {
        let prog = parse_str("f x:n>n;*x 2 g y:n>n;+y 1");
        assert_eq!(prog.declarations.len(), 2);
        let span_f = match &prog.declarations[0] {
            Decl::Function { span, .. } => *span,
            _ => panic!("expected function"),
        };
        let span_g = match &prog.declarations[1] {
            Decl::Function { span, .. } => *span,
            _ => panic!("expected function"),
        };
        // f starts at 0, g starts after f
        assert_eq!(span_f.start, 0);
        assert!(span_g.start > span_f.start, "g should start after f");
        assert!(span_g.start >= span_f.end, "g span should not overlap f span");
    }

    #[test]
    fn tool_decl_has_span() {
        let prog = parse_str(r#"tool fetch"http get" url:t>t"#);
        match &prog.declarations[0] {
            Decl::Tool { span, .. } => {
                assert_eq!(span.start, 0);
                assert!(span.end > 0);
            }
            _ => panic!("expected tool"),
        }
    }

    // ---- File-based tests ----

    #[test]
    fn parse_example_01_simple_function() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo");
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Decl::Function { name, params, return_type, body, .. } => {
                assert_eq!(name, "tot");
                assert_eq!(params.len(), 3);
                assert_eq!(*return_type, Type::Number);
                assert_eq!(body.len(), 3);
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_example_02_with_dependencies() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/02-with-dependencies.ilo");
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Decl::Function { name, return_type, .. } => {
                assert_eq!(name, "prc");
                assert!(matches!(return_type, Type::Result(_, _)));
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_error_messages() {
        let bad = "42 x:n>n;x";
        let tokens = lexer::lex(bad).unwrap();
        let token_spans: Vec<(Token, Span)> = tokens
            .into_iter()
            .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
            .collect();
        let (_prog, errors) = parse(token_spans);
        let err = errors.into_iter().next().expect("expected parse error");
        assert!(err.message.contains("expected declaration"), "got: {}", err.message);
    }

    #[test]
    fn parse_complex_match_patterns() {
        let prog = parse_str(r#"f x:R n t>n;?x{^e:0;~v:?v{1:100;2:200;_:v}}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                        // Second arm body should be a nested match statement
                        assert!(matches!(&arms[1].body[0], Stmt::Match { .. }));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_deeply_nested_prefix() {
        let prog = parse_str("f x:n>n;+*+x 1 2 3");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                // Should be: +(*(+(x,1), 2), 3)
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op: BinOp::Add, left, .. }) => {
                        match &**left {
                            Expr::BinOp { op: BinOp::Multiply, left: inner, .. } => {
                                assert!(matches!(&**inner, Expr::BinOp { op: BinOp::Add, .. }));
                            }
                            _ => panic!("expected nested multiply"),
                        }
                    }
                    _ => panic!("expected add"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_tokens_legacy_api() {
        // Test the legacy parse_tokens API
        let source = "f x:n>n;*x 2";
        let tokens: Vec<Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        let prog = parse_tokens(tokens).unwrap();
        assert_eq!(prog.declarations.len(), 1);
    }

    // ---- Error recovery tests ----

    #[test]
    fn recovery_second_function_parsed_after_first_error() {
        // First function has missing `>` (no params, hits `;` instead of `>`)
        // Second function should still parse correctly.
        let (prog, errors) = parse_str_errors("f x:n n;bad g y:n>n;y");
        // One error from `f`, one valid `g`
        assert!(!errors.is_empty(), "expected parse error from f");
        let valid: Vec<_> = prog.declarations.iter().filter(|d| !matches!(d, Decl::Error { .. })).collect();
        assert_eq!(valid.len(), 1, "g should parse successfully");
        match valid[0] {
            Decl::Function { name, .. } => assert_eq!(name, "g"),
            _ => panic!("expected function g"),
        }
    }

    #[test]
    fn recovery_error_node_in_declarations() {
        let (prog, errors) = parse_str_errors("f x:n n;bad g y:n>n;y");
        assert!(!errors.is_empty());
        // Program.declarations has two entries: an Error and a Function
        assert_eq!(prog.declarations.len(), 2);
        assert!(matches!(prog.declarations[0], Decl::Error { .. }));
        assert!(matches!(prog.declarations[1], Decl::Function { .. }));
    }

    #[test]
    fn recovery_two_errors_both_reported() {
        // Both functions have bad signatures
        let (prog, errors) = parse_str_errors("f x:n n;bad g y:n n;bad");
        assert_eq!(errors.len(), 2, "expected two errors");
        assert_eq!(prog.declarations.len(), 2);
        assert!(prog.declarations.iter().all(|d| matches!(d, Decl::Error { .. })));
    }

    #[test]
    fn recovery_error_node_not_in_json() {
        // Decl::Error nodes must be filtered from JSON AST output
        let (prog, _errors) = parse_str_errors("f x:n n;bad g y:n>n;y");
        let json = serde_json::to_string(&prog).unwrap();
        // Only g should appear; the error node is suppressed
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let decls = parsed["declarations"].as_array().unwrap();
        assert_eq!(decls.len(), 1, "only valid declarations should appear in JSON");
    }

    #[test]
    fn recovery_stops_at_20_errors() {
        // Build a string with 25 bad single-token "functions" followed by a valid one
        let bad: String = (0..25).map(|i| format!("f{i} x:n n;bad ")).collect();
        let good = "g y:n>n;y";
        let source = format!("{bad}{good}");
        let (_prog, errors) = parse_str_errors(&source);
        assert!(errors.len() <= 20, "should cap at 20 errors, got {}", errors.len());
    }

    #[test]
    fn recovery_type_decl_after_error() {
        // A type declaration after a broken function should be recovered
        let (prog, errors) = parse_str_errors("f x:n n;bad type point{x:n;y:n}");
        assert!(!errors.is_empty());
        let valid: Vec<_> = prog.declarations.iter().filter(|d| !matches!(d, Decl::Error { .. })).collect();
        assert_eq!(valid.len(), 1);
        assert!(matches!(valid[0], Decl::TypeDef { .. }));
    }
}
