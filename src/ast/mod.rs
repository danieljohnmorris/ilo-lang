use serde::{Deserialize, Serialize};

/// Types in idea9 — single-char base types, composable
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Number,  // n
    Text,    // t
    Bool,    // b
    Nil,     // _
    List(Box<Type>),             // L type
    Result(Box<Type>, Box<Type>), // R ok err
    Named(String),               // user-defined type name
}

/// A parameter: `name:type`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// Top-level declarations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Decl {
    /// `name params>return;body`
    Function {
        name: String,
        params: Vec<Param>,
        return_type: Type,
        body: Vec<Stmt>,
    },

    /// `type name{field:type;...}`
    TypeDef {
        name: String,
        fields: Vec<Param>,
    },

    /// `tool name"desc" params>return timeout:n,retry:n`
    Tool {
        name: String,
        description: String,
        params: Vec<Param>,
        return_type: Type,
        timeout: Option<f64>,
        retry: Option<f64>,
    },
}

/// Statements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// `name=expr`
    Let { name: String, value: Expr },

    /// `cond{body}` or `!cond{body}`
    Guard {
        condition: Expr,
        negated: bool,
        body: Vec<Stmt>,
    },

    /// `?expr{arms}` or `?{arms}`
    Match {
        subject: Option<Expr>,
        arms: Vec<MatchArm>,
    },

    /// `@binding collection{body}`
    ForEach {
        binding: String,
        collection: Expr,
        body: Vec<Stmt>,
    },

    /// Expression as statement (last expr is return value)
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// `!e:` — binds error value
    Err(String),
    /// `~v:` — binds ok value
    Ok(String),
    /// Literal pattern: `"gold":`, `1000:`
    Literal(Literal),
    /// `_:` — wildcard / catch-all
    Wildcard,
}

/// Expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Literal),

    /// Variable reference
    Ref(String),

    /// Field access: `obj.field`
    Field { object: Box<Expr>, field: String },

    /// Function call with positional args: `func arg1 arg2`
    Call {
        function: String,
        args: Vec<Expr>,
    },

    /// Prefix binary op: `+a b`, `*a b`
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary negation: `!expr` (logical) or `-expr` (numeric)
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Ok constructor: `~expr`
    Ok(Box<Expr>),

    /// Err constructor: `!expr`
    Err(Box<Expr>),

    /// List literal
    List(Vec<Expr>),

    /// Record construction: `typename field:val field:val`
    Record {
        type_name: String,
        fields: Vec<(String, Expr)>,
    },

    /// Match expression: `?expr{arms}` or `?{arms}` used as value
    Match {
        subject: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
    },

    /// With expression: `obj with field:val`
    With {
        object: Box<Expr>,
        updates: Vec<(String, Expr)>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Number(f64),
    Text(String),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    And,
    Or,
    Append,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Negate,
}

/// A complete program is a list of declarations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Decl>,
}
