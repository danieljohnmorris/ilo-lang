#![warn(clippy::all)]
#![deny(rust_2018_idioms)]

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod diagnostic;
pub mod graph;
pub mod interpreter;
pub mod lexer;
#[cfg(feature = "lsp")]
pub mod lsp;
pub mod parser;
pub mod tools;
pub mod verify;
pub mod vm;
