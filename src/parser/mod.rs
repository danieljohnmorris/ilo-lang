use crate::ast::*;
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("Parse error at token {position}: {message}")]
pub struct ParseError {
    pub position: usize,
    pub message: String,
}

type Result<T> = std::result::Result<T, ParseError>;

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        // Filter out newlines — idea9 uses ; as separator
        let tokens: Vec<Token> = tokens.into_iter().filter(|t| *t != Token::Newline).collect();
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        match self.peek() {
            Some(tok) if tok == expected => {
                self.advance();
                Ok(())
            }
            Some(tok) => Err(self.error(format!("expected {:?}, got {:?}", expected, tok))),
            None => Err(self.error(format!("expected {:?}, got EOF", expected))),
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(name)
            }
            Some(tok) => Err(self.error(format!("expected identifier, got {:?}", tok))),
            None => Err(self.error("expected identifier, got EOF".into())),
        }
    }

    fn error(&self, message: String) -> ParseError {
        ParseError {
            position: self.pos,
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

    // ---- Top-level parsing ----

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut declarations = Vec::new();
        while !self.at_end() {
            declarations.push(self.parse_decl()?);
        }
        Ok(Program { declarations, source: None })
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
                Err(self.error(format!("expected declaration, got {:?}{}", tok, hint)))
            }
            None => Err(self.error("expected declaration, got EOF".into())),
        }
    }

    /// `type name{field:type;...}`
    fn parse_type_decl(&mut self) -> Result<Decl> {
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
        self.expect(&Token::RBrace)?;
        Ok(Decl::TypeDef { name, fields, span: Span::default() })
    }

    /// `tool name"desc" params>return timeout:n,retry:n`
    fn parse_tool_decl(&mut self) -> Result<Decl> {
        self.expect(&Token::Tool)?;
        let name = self.expect_ident()?;
        let description = match self.peek().cloned() {
            Some(Token::Text(s)) => {
                self.advance();
                s
            }
            _ => return Err(self.error("expected tool description string".into())),
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

        Ok(Decl::Tool {
            name,
            description,
            params,
            return_type,
            timeout,
            retry,
            span: Span::default(),
        })
    }

    /// `name params>return;body`
    fn parse_fn_decl(&mut self) -> Result<Decl> {
        let name = self.expect_ident()?;
        let params = self.parse_params()?;
        self.expect(&Token::Greater)?;
        let return_type = self.parse_type()?;
        self.expect(&Token::Semi)?;
        let body = self.parse_body()?;
        Ok(Decl::Function {
            name,
            params,
            return_type,
            body,
            span: Span::default(),
        })
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
            Some(tok) => Err(self.error(format!("expected type, got {:?}", tok))),
            None => Err(self.error("expected type, got EOF".into())),
        }
    }

    /// Parse parameter list: `name:type name:type ...`
    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        while let Some(Token::Ident(_)) = self.peek() {
            // Look ahead for colon to distinguish params from other constructs
            if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
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
                if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Eq {
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
        match &self.tokens[after_semi] {
            // ^ident: or ^_: → err pattern
            Token::Caret => {
                if after_semi + 2 < self.tokens.len() {
                    matches!(
                        (&self.tokens[after_semi + 1], &self.tokens[after_semi + 2]),
                        (Token::Ident(_) | Token::Underscore, Token::Colon)
                    )
                } else {
                    false
                }
            }
            // ~ident: or ~_: → ok pattern
            Token::Tilde => {
                if after_semi + 2 < self.tokens.len() {
                    matches!(
                        (&self.tokens[after_semi + 1], &self.tokens[after_semi + 2]),
                        (Token::Ident(_) | Token::Underscore, Token::Colon)
                    )
                } else {
                    false
                }
            }
            // _: → wildcard
            Token::Underscore => {
                after_semi + 1 < self.tokens.len()
                    && self.tokens[after_semi + 1] == Token::Colon
            }
            // literal: → literal pattern (number, string, bool)
            Token::Number(_) | Token::Text(_) | Token::True | Token::False => {
                after_semi + 1 < self.tokens.len()
                    && self.tokens[after_semi + 1] == Token::Colon
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
            Some(tok) => Err(self.error(format!("expected pattern, got {:?}", tok))),
            None => Err(self.error("expected pattern, got EOF".into())),
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
                if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
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
                && self.tokens[self.pos + 1] == Token::RParen
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
            && self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
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
            Some(tok) => Err(self.error(format!("expected expression, got {:?}", tok))),
            None => Err(self.error("expected expression, got EOF".into())),
        }
    }

    fn parse_number(&mut self) -> Result<f64> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(n)
            }
            Some(tok) => Err(self.error(format!("expected number, got {:?}", tok))),
            None => Err(self.error("expected number, got EOF".into())),
        }
    }
}

/// Convenience function
pub fn parse(tokens: Vec<Token>) -> Result<Program> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_str(source: &str) -> Program {
        let tokens: Vec<Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        parse(tokens).unwrap()
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
    fn parse_field_access() {
        let prog = parse_str("f x:order>n;x.total");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Field { field, .. }) => {
                        assert_eq!(field, "total");
                    }
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
                    Stmt::Expr(Expr::Index { index, .. }) => {
                        assert_eq!(*index, 0);
                    }
                    _ => panic!("expected index access"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_chained_index() {
        // x.1 should parse as index access with index 1
        let prog = parse_str("f xs:L n>n;xs.1");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Index { index, .. }) => {
                        assert_eq!(*index, 1);
                    }
                    _ => panic!("expected index access"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_foreach() {
        let prog = parse_str("f xs:L n>n;@x xs{+x 1}");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::ForEach { binding, .. } => {
                        assert_eq!(binding, "x");
                    }
                    _ => panic!("expected foreach"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_match_expr_in_let() {
        let prog = parse_str(r#"f x:t>n;y=?x{"a":1;"b":2;_:0};y"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "y");
                        assert!(matches!(value, Expr::Match { .. }));
                    }
                    _ => panic!("expected let with match expr"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_record_construction() {
        let prog = parse_str("f x:n y:t>point;point x:x y:y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Record { type_name, fields }) => {
                        assert_eq!(type_name, "point");
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected record, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_with_expr() {
        let prog = parse_str("f x:order>order;x with total:100");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::With { updates, .. }) => {
                        assert_eq!(updates.len(), 1);
                        assert_eq!(updates[0].0, "total");
                    }
                    _ => panic!("expected with expr"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_zero_arg_call() {
        let prog = parse_str("f>t;make-id()");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Call { function, args }) => {
                        assert_eq!(function, "make-id");
                        assert!(args.is_empty());
                    }
                    _ => panic!("expected zero-arg call"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_unary_negate() {
        // -x as the return expression should be unary negate
        let prog = parse_str("f x:n>n;-x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::UnaryOp { op, operand }) => {
                        assert_eq!(*op, UnaryOp::Negate);
                        assert!(matches!(operand.as_ref(), Expr::Ref(name) if name == "x"));
                    }
                    _ => panic!("expected unary negate, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_unary_negate_in_let() {
        // y=-x should bind unary negate
        let prog = parse_str("f x:n>n;y=-x;y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "y");
                        assert!(matches!(value, Expr::UnaryOp { op: UnaryOp::Negate, .. }));
                    }
                    _ => panic!("expected let with negate"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_binary_subtract_still_works() {
        // -a b should still be binary subtract
        let prog = parse_str("f x:n y:n>n;-x y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::Subtract);
                    }
                    _ => panic!("expected binary subtract, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_list_literal() {
        let prog = parse_str("f>L n;[1, 2, 3]");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::List(items)) => {
                        assert_eq!(items.len(), 3);
                    }
                    _ => panic!("expected list literal, got {:?}", body[0]),
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
                    Stmt::Expr(Expr::List(items)) => {
                        assert_eq!(items.len(), 0);
                    }
                    _ => panic!("expected empty list, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_list_in_let() {
        let prog = parse_str("f>L n;xs=[1, 2, 3];xs");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "xs");
                        assert!(matches!(value, Expr::List(_)));
                    }
                    _ => panic!("expected let with list"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_negated_guard() {
        let prog = parse_str(r#"f x:b>t;!x{"nope"};x"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { negated, .. } => assert!(negated),
                    _ => panic!("expected negated guard"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_logical_not() {
        let prog = parse_str("f x:b>b;!x");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::UnaryOp { op, operand }) => {
                        assert_eq!(*op, UnaryOp::Not);
                        assert!(matches!(operand.as_ref(), Expr::Ref(name) if name == "x"));
                    }
                    _ => panic!("expected logical NOT, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_logical_not_in_let() {
        let prog = parse_str("f x:b>b;y=!x;y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "y");
                        assert!(matches!(value, Expr::UnaryOp { op: UnaryOp::Not, .. }));
                    }
                    _ => panic!("expected let with NOT"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_multi_stmt_match_arm() {
        let prog = parse_str(r#"f>R _ t;?{^e:^"fail";~d:call d;~_}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        // ~d: arm should have 2 stmts: call d; ~_
                        assert_eq!(arms.len(), 2);
                        assert!(arms[1].body.len() >= 2);
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // Integration tests — parse all 5 example files

    fn parse_file(path: &str) -> Program {
        let source = std::fs::read_to_string(path).unwrap();
        parse_str(&source)
    }

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
    fn parse_example_03_data_transform() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/03-data-transform.ilo");
        assert_eq!(prog.declarations.len(), 2);
        match &prog.declarations[0] {
            Decl::Function { name, .. } => assert_eq!(name, "cls"),
            _ => panic!("expected function cls"),
        }
        match &prog.declarations[1] {
            Decl::Function { name, .. } => assert_eq!(name, "sms"),
            _ => panic!("expected function sms"),
        }
    }

    #[test]
    fn parse_example_04_tool_interaction() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/04-tool-interaction.ilo");
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Decl::Function { name, .. } => assert_eq!(name, "ntf"),
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_example_05_workflow() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/05-workflow.ilo");
        assert_eq!(prog.declarations.len(), 1);
        match &prog.declarations[0] {
            Decl::Function { name, .. } => assert_eq!(name, "chk"),
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_logical_and() {
        let prog = parse_str("f a:b b:b>b;&a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::And);
                    }
                    other => panic!("expected BinOp And, got {:?}", other),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_logical_or() {
        let prog = parse_str("f a:b b:b>b;|a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::Or);
                    }
                    other => panic!("expected BinOp Or, got {:?}", other),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_nested_prefix_binops() {
        // +*a b c → BinOp(Add, BinOp(Mul, a, b), c)
        let prog = parse_str("f a:n b:n c:n>n;+*a b c");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, right }) => {
                        assert_eq!(*op, BinOp::Add);
                        assert!(matches!(left.as_ref(), Expr::BinOp { op: BinOp::Multiply, .. }));
                        assert!(matches!(right.as_ref(), Expr::Ref(name) if name == "c"));
                    }
                    _ => panic!("expected nested binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_deep_nesting() {
        // >=*+a b c 100 → BinOp(GreaterOrEqual, BinOp(Mul, BinOp(Add, a, b), c), 100)
        let prog = parse_str("f a:n b:n c:n>b;>=*+a b c 100");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, .. }) => {
                        assert_eq!(*op, BinOp::GreaterOrEqual);
                        match left.as_ref() {
                            Expr::BinOp { op, left: inner_left, .. } => {
                                assert_eq!(*op, BinOp::Multiply);
                                assert!(matches!(inner_left.as_ref(), Expr::BinOp { op: BinOp::Add, .. }));
                            }
                            _ => panic!("expected nested mul"),
                        }
                    }
                    _ => panic!("expected nested binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_not_as_binop_operand() {
        // &!x y → BinOp(And, UnaryOp(Not, x), y)
        let prog = parse_str("f x:b y:b>b;&!x y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, right }) => {
                        assert_eq!(*op, BinOp::And);
                        assert!(matches!(left.as_ref(), Expr::UnaryOp { op: UnaryOp::Not, .. }));
                        assert!(matches!(right.as_ref(), Expr::Ref(name) if name == "y"));
                    }
                    _ => panic!("expected and-not, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_negate_binop() {
        // -*a b → UnaryOp(Negate, BinOp(Mul, a, b))
        let prog = parse_str("f a:n b:n>n;-*a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::UnaryOp { op, operand }) => {
                        assert_eq!(*op, UnaryOp::Negate);
                        assert!(matches!(operand.as_ref(), Expr::BinOp { op: BinOp::Multiply, .. }));
                    }
                    _ => panic!("expected negate-product, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_ok_as_operand() {
        // +~a b → BinOp(Add, Ok(a), b)
        let prog = parse_str("f a:n b:n>n;+~a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, .. }) => {
                        assert_eq!(*op, BinOp::Add);
                        assert!(matches!(left.as_ref(), Expr::Ok(_)));
                    }
                    _ => panic!("expected binop with Ok operand, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_err_as_operand() {
        // +^a b → BinOp(Add, Err(a), b)
        let prog = parse_str("f a:n b:n>n;+^a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, .. }) => {
                        assert_eq!(*op, BinOp::Add);
                        assert!(matches!(left.as_ref(), Expr::Err(_)));
                    }
                    _ => panic!("expected binop with Err operand, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo");
        let json = serde_json::to_string(&prog).unwrap();
        let deserialized: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(prog, deserialized);
    }

    // --- Helper for error tests ---

    fn parse_str_err(source: &str) -> ParseError {
        let tokens: Vec<Token> = lexer::lex(source)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        parse(tokens).unwrap_err()
    }

    fn parse_tokens_err(tokens: Vec<Token>) -> ParseError {
        parse(tokens).unwrap_err()
    }

    // === Error path tests ===

    // 1. expect: wrong token
    #[test]
    fn error_expect_wrong_token() {
        // Function decl expects > after params, giving it something else
        // "f x:n n" — after type 'n', parser expects '>' but gets 'n' (Ident)
        let err = parse_str_err("f x:n n");
        assert!(err.message.contains("expected"), "got: {}", err.message);
    }

    // 1. expect: EOF
    #[test]
    fn error_expect_eof() {
        // "f x:n" — after type, parser expects '>' but hits EOF
        let err = parse_str_err("f x:n");
        assert!(err.message.contains("EOF"), "got: {}", err.message);
    }

    // 2. expect_ident: wrong token
    #[test]
    fn error_expect_ident_wrong_token() {
        // "type 123" — after 'type', parser expects ident but gets number
        let err = parse_tokens_err(vec![Token::Type, Token::Number(123.0)]);
        assert!(err.message.contains("expected identifier"), "got: {}", err.message);
    }

    // 2. expect_ident: EOF
    #[test]
    fn error_expect_ident_eof() {
        // "type" — after 'type', parser expects ident but hits EOF
        let err = parse_tokens_err(vec![Token::Type]);
        assert!(err.message.contains("expected identifier") && err.message.contains("EOF"), "got: {}", err.message);
    }

    // 3. parse_decl: unknown token
    #[test]
    fn error_parse_decl_unknown_token() {
        let err = parse_tokens_err(vec![Token::Plus]);
        assert!(err.message.contains("expected declaration"), "got: {}", err.message);
    }

    // 3. parse_decl: EOF — this shouldn't happen normally since parse_program checks at_end,
    //    but we can test it by calling parse_decl directly
    #[test]
    fn error_parse_decl_eof() {
        // Construct a parser that's already at the end, then call parse_decl
        let mut parser = Parser::new(vec![]);
        let err = parser.parse_program();
        // Empty program is valid, so test via parse_decl directly
        // We need a situation where parse_program calls parse_decl at EOF.
        // Actually an empty token list produces an empty program, which is fine.
        // Instead, test with a token that causes parse_decl to be called when at EOF.
        // We can't easily trigger this via parse_program, but parse_decl itself handles None.
        assert!(err.is_ok()); // empty program is valid

        // Direct test of parse_decl at EOF
        let mut parser = Parser::new(vec![]);
        let err = parser.parse_decl().unwrap_err();
        assert!(err.message.contains("EOF"), "got: {}", err.message);
    }

    // 4. parse_type: unexpected token
    #[test]
    fn error_parse_type_unexpected_token() {
        // "f>+" — after '>', parser expects type but gets '+'
        let err = parse_str_err("f>+");
        assert!(err.message.contains("expected type"), "got: {}", err.message);
    }

    // 4. parse_type: EOF
    #[test]
    fn error_parse_type_eof() {
        // "f>" — after '>', parser expects type but hits EOF
        let err = parse_str_err("f>");
        assert!(err.message.contains("expected type") && err.message.contains("EOF"), "got: {}", err.message);
    }

    // 5. parse_atom: unexpected token
    #[test]
    fn error_parse_atom_unexpected_token() {
        // Inside expression context, hit an unexpected token
        // "f>n;>" — body starts, parser tries to parse expr/atom, gets '>'
        let err = parse_str_err("f>n;>");
        assert!(err.message.contains("expected expression"), "got: {}", err.message);
    }

    // 5. parse_atom: EOF
    #[test]
    fn error_parse_atom_eof() {
        // "f>n;+x" — binary add needs two operands, second one hits EOF
        let err = parse_str_err("f>n;+x");
        assert!(err.message.contains("expected expression") && err.message.contains("EOF"), "got: {}", err.message);
    }

    // 6. parse_number: wrong token
    #[test]
    fn error_parse_number_wrong_token() {
        // Tool with timeout but non-number value: tool name"desc">n timeout:abc
        // We need to construct tokens manually since lexer would lex "abc" as ident
        let err = parse_tokens_err(vec![
            Token::Tool,
            Token::Ident("fetch".into()),
            Token::Text("desc".into()),
            Token::Greater,
            Token::Ident("n".into()),
            Token::Timeout,
            Token::Colon,
            Token::Ident("abc".into()),
        ]);
        assert!(err.message.contains("expected number"), "got: {}", err.message);
    }

    // 6. parse_number: EOF
    #[test]
    fn error_parse_number_eof() {
        let err = parse_tokens_err(vec![
            Token::Tool,
            Token::Ident("fetch".into()),
            Token::Text("desc".into()),
            Token::Greater,
            Token::Ident("n".into()),
            Token::Timeout,
            Token::Colon,
            // EOF here
        ]);
        assert!(err.message.contains("expected number") && err.message.contains("EOF"), "got: {}", err.message);
    }

    // 7. parse_tool_decl: missing description string
    #[test]
    fn error_tool_missing_description() {
        // "tool fetch" followed by non-string
        let err = parse_tokens_err(vec![
            Token::Tool,
            Token::Ident("fetch".into()),
            Token::Ident("x".into()),
        ]);
        assert!(err.message.contains("expected tool description"), "got: {}", err.message);
    }

    // === Edge case tests ===

    // 8. Number literal pattern in match
    #[test]
    fn parse_match_number_pattern() {
        let prog = parse_str(r#"f x:n>t;?x{1:"one";_:"other"}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { subject, arms } => {
                        assert!(subject.is_some());
                        assert_eq!(arms.len(), 2);
                        assert!(matches!(&arms[0].pattern, Pattern::Literal(Literal::Number(n)) if *n == 1.0));
                        assert!(matches!(&arms[1].pattern, Pattern::Wildcard));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 9. Boolean literal pattern in match
    #[test]
    fn parse_match_bool_pattern() {
        let prog = parse_str(r#"f x:b>t;?x{true:"yes";false:"no"}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                        assert!(matches!(&arms[0].pattern, Pattern::Literal(Literal::Bool(true))));
                        assert!(matches!(&arms[1].pattern, Pattern::Literal(Literal::Bool(false))));
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 10. Trailing semicolon in body
    #[test]
    fn parse_trailing_semicolon_in_body() {
        let prog = parse_str("f x:n>n;+x 1;");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected function"),
        }
    }

    // 10. Trailing semicolon in match arm body (line 342)
    #[test]
    fn parse_trailing_semicolon_in_match_arm() {
        let prog = parse_str(r#"f x:n>t;?x{1:"one";}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 1);
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 10. Trailing semicolon between match arms (line 318)
    #[test]
    fn parse_trailing_semicolon_between_arms() {
        // A trailing semi before } in the arms list
        let prog = parse_str(r#"f x:n>t;?x{1:"one";_:"other";}"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Match { arms, .. } => {
                        assert_eq!(arms.len(), 2);
                    }
                    _ => panic!("expected match"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 11. Empty body (guard with empty brace body)
    #[test]
    fn parse_empty_brace_body() {
        let prog = parse_str(r#"f x:b>n;x{};1"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { body: guard_body, .. } => {
                        assert!(guard_body.is_empty());
                    }
                    _ => panic!("expected guard, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 12. Prefix equals operator: =a b
    #[test]
    fn parse_prefix_equals() {
        let prog = parse_str("f a:n b:n>b;=a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::Equals);
                    }
                    _ => panic!("expected equals binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 12. Prefix not-equals operator: !=a b
    #[test]
    fn parse_prefix_not_equals() {
        let prog = parse_str("f a:n b:n>b;!=a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::NotEquals);
                    }
                    _ => panic!("expected not-equals binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 13. Minus as operand in another prefix op: +-a b c
    #[test]
    fn parse_minus_as_operand() {
        // +-a b c → BinOp(Add, BinOp(Subtract, a, b), c)
        let prog = parse_str("f a:n b:n c:n>n;+-a b c");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, left, .. }) => {
                        assert_eq!(*op, BinOp::Add);
                        assert!(matches!(left.as_ref(), Expr::BinOp { op: BinOp::Subtract, .. }));
                    }
                    _ => panic!("expected nested binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 14. ^expr in expression context (not statement) — in a let binding
    #[test]
    fn parse_caret_in_let_binding() {
        let prog = parse_str(r#"f x:n>R n t;y=^x;y"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "y");
                        assert!(matches!(value, Expr::Err(_)));
                    }
                    _ => panic!("expected let with err, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 15. Identifier-started guard: cond{body}
    #[test]
    fn parse_ident_guard() {
        let prog = parse_str(r#"f x:b>t;x{"yes"};"no""#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { condition, negated, body: guard_body } => {
                        assert!(!negated);
                        assert!(matches!(condition, Expr::Ref(name) if name == "x"));
                        assert_eq!(guard_body.len(), 1);
                    }
                    _ => panic!("expected guard, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 16. Subject-less ?{...} match in expression position (let binding)
    #[test]
    fn parse_subjectless_match_in_let() {
        let prog = parse_str(r#"f x:n>t;y=?{_:"all"};y"#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "y");
                        match value {
                            Expr::Match { subject, arms } => {
                                assert!(subject.is_none());
                                assert_eq!(arms.len(), 1);
                            }
                            _ => panic!("expected match expr in let"),
                        }
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 17. Trailing comma in list literal
    #[test]
    fn parse_list_trailing_comma() {
        let prog = parse_str("f>L n;[1, 2,]");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::List(items)) => {
                        assert_eq!(items.len(), 2);
                    }
                    _ => panic!("expected list, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 18. with block where field list ends due to non-colon token
    #[test]
    fn parse_with_field_list_ends_non_colon() {
        // "x with a:1" followed by something that's not "ident:"
        // The `with` parser stops when it can't find ident:value pairs
        let prog = parse_str("f x:order y:n>order;x with total:100");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::With { updates, .. }) => {
                        assert_eq!(updates.len(), 1);
                    }
                    _ => panic!("expected with expr, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // 18b. with block with multiple fields, parsing stops at non-field
    #[test]
    fn parse_with_stops_at_non_field_ident() {
        // After "with a:1", if next token is ident without colon, with stops
        // We test by having the with in a let, so parser continues to next stmt
        let prog = parse_str("f x:order>order;y=x with total:100;y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                assert_eq!(body.len(), 2);
                match &body[0] {
                    Stmt::Let { value, .. } => {
                        assert!(matches!(value, Expr::With { .. }));
                    }
                    _ => panic!("expected let with with-expr"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // Additional: ^expr as statement (caret_stmt)
    #[test]
    fn parse_caret_stmt_standalone() {
        let prog = parse_str(r#"f x:n>R n t;^"error""#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Err(inner)) => {
                        assert!(matches!(inner.as_ref(), Expr::Literal(Literal::Text(s)) if s == "error"));
                    }
                    _ => panic!("expected err expr stmt, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // Additional: guard from non-ident expression (line 274-286)
    #[test]
    fn parse_non_ident_expr_guard() {
        // A comparison expression followed by {body} triggers guard from the _ branch of parse_stmt
        let prog = parse_str(r#"f x:n>t;>=x 10{"big"};"small""#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { condition, negated, .. } => {
                        assert!(!negated);
                        assert!(matches!(condition, Expr::BinOp { op: BinOp::GreaterOrEqual, .. }));
                    }
                    _ => panic!("expected guard, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // Additional: prefix append operator +=
    #[test]
    fn parse_prefix_append() {
        let prog = parse_str("f a:L n b:L n>L n;+=a b");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::Append);
                    }
                    _ => panic!("expected append binop, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // --- Prefix expressions as call arguments ---

    #[test]
    fn parse_call_with_prefix_arg() {
        // fac -n 1 should parse as Call(fac, [Subtract(n, 1)])
        let prog = parse_str("fac n:n>n;<=n 1{1};p=-n 1;r=fac -n 1;*n r");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                // body[2]: r=fac -n 1
                match &body[2] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "r");
                        match value {
                            Expr::Call { function, args } => {
                                assert_eq!(function, "fac");
                                assert_eq!(args.len(), 1);
                                assert!(matches!(&args[0], Expr::BinOp { op: BinOp::Subtract, .. }));
                            }
                            _ => panic!("expected call, got {:?}", value),
                        }
                    }
                    _ => panic!("expected let, got {:?}", body[2]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_call_with_two_prefix_args() {
        // g +a b c should parse as Call(g, [Add(a,b), Ref(c)])
        let prog = parse_str("g a:n b:n c:n>n;f +a b c");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::Call { function, args }) => {
                        assert_eq!(function, "f");
                        assert_eq!(args.len(), 2);
                        assert!(matches!(&args[0], Expr::BinOp { op: BinOp::Add, .. }));
                        assert!(matches!(&args[1], Expr::Ref(name) if name == "c"));
                    }
                    _ => panic!("expected call, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_nested_prefix_still_works() {
        // +*a b c should still parse as Add(Mul(a,b), c)
        let prog = parse_str("f a:n b:n c:n>n;+*a b c");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Expr(Expr::BinOp { op: BinOp::Add, left, right }) => {
                        assert!(matches!(left.as_ref(), Expr::BinOp { op: BinOp::Multiply, .. }));
                        assert!(matches!(right.as_ref(), Expr::Ref(name) if name == "c"));
                    }
                    _ => panic!("expected nested prefix, got {:?}", body[0]),
                }
            }
            _ => panic!("expected function"),
        }
    }
}
