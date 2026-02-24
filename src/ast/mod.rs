use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Every value in LFAA is typed and named.
/// No positional arguments, no implicit conversions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Number,
    Text,
    Bool,
    List(Box<Type>),
    Record(HashMap<String, Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Void,
    /// User-defined type name
    Named(String),
}

/// A named parameter: `amount as number`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// A named argument at a call site: `amount: 42`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedArg {
    pub name: String,
    pub value: Expr,
}

/// Property assertions on functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Property {
    /// `fibonacci 0 equals 0`
    Example {
        args: Vec<NamedArg>,
        expected: Expr,
    },
    /// Free-form invariant (checked at runtime)
    Invariant {
        description: String,
        condition: Expr,
    },
}

/// Top-level declarations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Decl {
    /// `define function <name>`
    Function {
        name: String,
        requires: Vec<Import>,
        inputs: Vec<Param>,
        output: Type,
        properties: Vec<Property>,
        body: Vec<Stmt>,
    },

    /// `define type <name>`
    TypeDef {
        name: String,
        fields: Vec<Param>,
    },

    /// `define tool <name>` — external tool available to the agent
    Tool {
        name: String,
        description: String,
        inputs: Vec<Param>,
        output: Type,
        timeout_secs: Option<u64>,
        retry: Option<u32>,
    },

    /// `amend function <name>` — patch an existing function
    Amend {
        target: String,
        after: Option<String>,
        before: Option<String>,
        insert: Vec<Stmt>,
        remove: Vec<String>,
    },
}

/// Import: `get-order from orders`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub name: String,
    pub module: String,
}

/// Statements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// `let name = expr`
    Let { name: String, value: Expr },

    /// `return expr`
    Return { value: Expr },

    /// `call function-name arg1:val1 arg2:val2`
    Call {
        function: String,
        args: Vec<NamedArg>,
    },

    /// `if condition then ... else ...`
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },

    /// `for-each item in collection do ...`
    ForEach {
        binding: String,
        collection: Expr,
        body: Vec<Stmt>,
    },

    /// `match expr on ...`
    Match {
        subject: Expr,
        arms: Vec<MatchArm>,
    },

    /// `transaction ... on-failure ...`
    Transaction {
        body: Vec<Stmt>,
        on_failure: Vec<Stmt>,
    },

    /// `log level: "info" message: "something happened"`
    Log { level: String, message: Expr },

    /// `assert condition message: "..."` — runtime check
    Assert {
        condition: Expr,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Literal(Literal),
    Name(String),
    Wildcard,
}

/// Expressions — always evaluate to a value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Literal),

    /// Variable reference
    Ref(String),

    /// Field access: `order.amount`
    Field { object: Box<Expr>, field: String },

    /// Named function call as expression
    Call {
        function: String,
        args: Vec<NamedArg>,
    },

    /// Binary operation (all named): `add a b`, `equals a b`
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary: `not condition`
    UnaryOp { op: UnaryOp, operand: Box<Expr> },

    /// List literal
    List(Vec<Expr>),

    /// Record literal
    Record(Vec<NamedArg>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Number(f64),
    Text(String),
    Bool(bool),
    Nothing,
}

/// All operators are words, not symbols. No `+`, `-`, `==`.
/// Agents don't confuse `add` with `equals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    And,
    Or,
    Concat,
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
