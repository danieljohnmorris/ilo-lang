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
        Ok(Program { declarations })
    }

    fn parse_decl(&mut self) -> Result<Decl> {
        match self.peek() {
            Some(Token::Type) => self.parse_type_decl(),
            Some(Token::Tool) => self.parse_tool_decl(),
            Some(Token::Ident(_)) => self.parse_fn_decl(),
            Some(tok) => Err(self.error(format!("expected declaration, got {:?}", tok))),
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
        Ok(Decl::TypeDef { name, fields })
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
                let operand = self.parse_atom()?;
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
        let first = self.parse_atom()?;
        if self.can_start_atom() {
            let second = self.parse_atom()?;
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
        let left = self.parse_atom()?;
        let right = self.parse_atom()?;
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
            if self.can_start_atom() {
                let mut args = Vec::new();
                while self.can_start_atom() {
                    args.push(self.parse_atom()?);
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
            Decl::TypeDef { name, fields } => {
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
    fn parse_not_with_binop_via_let() {
        // NOT combined with AND requires binding: n=!x;&n y
        let prog = parse_str("f x:b y:b>b;n=!x;&n y");
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Let { name, value } => {
                        assert_eq!(name, "n");
                        assert!(matches!(value, Expr::UnaryOp { op: UnaryOp::Not, .. }));
                    }
                    _ => panic!("expected let with NOT, got {:?}", body[0]),
                }
                match &body[1] {
                    Stmt::Expr(Expr::BinOp { op, .. }) => {
                        assert_eq!(*op, BinOp::And);
                    }
                    _ => panic!("expected AND, got {:?}", body[1]),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_not_still_allows_negated_guard() {
        // !x{body} is negated guard, not logical NOT
        let prog = parse_str(r#"f x:b>t;!x{"no"};"yes""#);
        match &prog.declarations[0] {
            Decl::Function { body, .. } => {
                match &body[0] {
                    Stmt::Guard { negated, .. } => {
                        assert!(negated, "expected negated guard");
                    }
                    _ => panic!("expected negated guard, got {:?}", body[0]),
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
            Decl::Function { name, params, return_type, body } => {
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
    fn roundtrip_serialize_deserialize() {
        let prog = parse_file("research/explorations/idea9-ultra-dense-short/01-simple-function.ilo");
        let json = serde_json::to_string(&prog).unwrap();
        let deserialized: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(prog, deserialized);
    }
}
