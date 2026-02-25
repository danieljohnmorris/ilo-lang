mod ast;
mod lexer;
mod parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ilo <file.ilo>");
        std::process::exit(1);
    }

    let path = &args[1];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        }
    };

    let tokens = match lexer::lex(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lex error: {}", e);
            std::process::exit(1);
        }
    };

    let token_values: Vec<lexer::Token> = tokens.into_iter().map(|(t, _)| t).collect();

    let program = match parser::parse(token_values) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    match serde_json::to_string_pretty(&program) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Serialization error: {}", e);
            std::process::exit(1);
        }
    }
}
