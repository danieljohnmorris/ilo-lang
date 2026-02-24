use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t]+")]
#[logos(skip r"--[^\n]*")]
pub enum Token {
    // Keywords — all lowercase, no abbreviations
    #[token("define")]
    Define,
    #[token("function")]
    Function,
    #[token("type")]
    TypeKw,
    #[token("tool")]
    ToolKw,
    #[token("amend")]
    Amend,
    #[token("input")]
    Input,
    #[token("output")]
    Output,
    #[token("body")]
    Body,
    #[token("requires")]
    Requires,
    #[token("properties")]
    Properties,
    #[token("description")]
    Description,
    #[token("timeout")]
    Timeout,
    #[token("retry")]
    Retry,

    #[token("let")]
    Let,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("for-each")]
    ForEach,
    #[token("in")]
    In,
    #[token("do")]
    Do,
    #[token("match")]
    Match,
    #[token("on")]
    On,
    #[token("transaction")]
    Transaction,
    #[token("on-failure")]
    OnFailure,
    #[token("log")]
    Log,
    #[token("assert")]
    Assert,
    #[token("end")]
    End,

    #[token("as")]
    As,
    #[token("from")]
    From,
    #[token("equals")]
    Equals,
    #[token("not-equals")]
    NotEquals,
    #[token("greater-than")]
    GreaterThan,
    #[token("less-than")]
    LessThan,
    #[token("greater-or-equal")]
    GreaterOrEqual,
    #[token("less-or-equal")]
    LessOrEqual,

    #[token("add")]
    Add,
    #[token("subtract")]
    Subtract,
    #[token("multiply")]
    Multiply,
    #[token("divide")]
    Divide,
    #[token("modulo")]
    Modulo,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("concat")]
    Concat,

    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("nothing")]
    Nothing,

    // Built-in types
    #[token("number")]
    NumberType,
    #[token("text")]
    TextType,
    #[token("bool")]
    BoolType,
    #[token("list")]
    ListType,
    #[token("void")]
    VoidType,
    #[token("option")]
    OptionType,
    #[token("result")]
    ResultType,

    // Punctuation — minimal
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("=")]
    Assign,
    #[token("_")]
    Wildcard,

    // Literals
    #[regex(r"-?[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Number(f64),

    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    Text(String),

    // Identifiers: lowercase with hyphens (agent-friendly, no camelCase confusion)
    #[regex(r"[a-z][a-z0-9]*(-[a-z0-9]+)*", |lex| lex.slice().to_string())]
    Ident(String),

    // Newlines are significant (statement terminators)
    #[token("\n")]
    Newline,
}

/// Lex source code into a stream of tokens with positions.
/// Returns errors as specific locations rather than failing silently.
pub fn lex(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, LexError> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();

    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => tokens.push((token, lexer.span())),
            Err(()) => {
                let span = lexer.span();
                return Err(LexError {
                    position: span.start,
                    snippet: source[span.clone()].to_string(),
                    suggestion: suggest_fix(&source[span.clone()]),
                });
            }
        }
    }

    Ok(tokens)
}

/// Every lex error comes with a suggested fix — agents need actionable feedback.
fn suggest_fix(bad_token: &str) -> String {
    if bad_token.contains('_') {
        format!(
            "Use hyphens instead of underscores: '{}'",
            bad_token.replace('_', "-")
        )
    } else if bad_token.chars().next().is_some_and(|c| c.is_uppercase()) {
        format!(
            "Use lowercase: '{}'",
            bad_token.to_lowercase()
        )
    } else {
        format!("Unexpected character(s): '{}'. Identifiers must be lowercase with hyphens.", bad_token)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Lex error at position {position}: '{snippet}'. {suggestion}")]
pub struct LexError {
    pub position: usize,
    pub snippet: String,
    pub suggestion: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_simple_function() {
        let source = r#"define function add-numbers
  input: a as number, b as number
  output: number
  body:
    return add a b
end"#;
        let tokens = lex(source).unwrap();
        assert!(!tokens.is_empty());
        assert_eq!(tokens[0].0, Token::Define);
        assert_eq!(tokens[1].0, Token::Function);
    }

    #[test]
    fn lex_string_literal() {
        let source = r#""hello world""#;
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].0, Token::Text("hello world".to_string()));
    }

    #[test]
    fn lex_comment_ignored() {
        let source = "-- this is a comment\ndefine";
        let tokens = lex(source).unwrap();
        // Comment skipped, newline and define remain
        assert!(tokens.iter().any(|(t, _)| *t == Token::Define));
    }
}
