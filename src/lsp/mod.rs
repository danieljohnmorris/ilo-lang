//! LSP server for ilo.
//!
//! Implements Language Server Protocol over stdio using tower-lsp.
//! Provides: diagnostics on change, hover, go-to-definition, completions,
//! and document symbols.

use std::collections::HashMap;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::ast::{self, Decl, Expr, Program, Span, Spanned, Stmt, Type};
use crate::lexer;
use crate::parser;
use crate::verify;

// ── Builtin catalogue (name, signature, description) ─────────────────────────

struct BuiltinInfo {
    name: &'static str,
    sig: &'static str,
    desc: &'static str,
}

const BUILTIN_INFO: &[BuiltinInfo] = &[
    BuiltinInfo { name: "len",   sig: "len list_or_text → n",        desc: "Length of a list, map, or text" },
    BuiltinInfo { name: "str",   sig: "str n → t",                   desc: "Convert number to text" },
    BuiltinInfo { name: "num",   sig: "num t → R n t",               desc: "Parse text as number" },
    BuiltinInfo { name: "abs",   sig: "abs n → n",                   desc: "Absolute value" },
    BuiltinInfo { name: "flr",   sig: "flr n → n",                   desc: "Floor (round down)" },
    BuiltinInfo { name: "cel",   sig: "cel n → n",                   desc: "Ceiling (round up)" },
    BuiltinInfo { name: "rou",   sig: "rou n → n",                   desc: "Round to nearest integer" },
    BuiltinInfo { name: "min",   sig: "min n n → n",                 desc: "Minimum of two numbers" },
    BuiltinInfo { name: "max",   sig: "max n n → n",                 desc: "Maximum of two numbers" },
    BuiltinInfo { name: "mod",   sig: "mod n n → n",                 desc: "Modulo" },
    BuiltinInfo { name: "sum",   sig: "sum L n → n",                 desc: "Sum all numbers in a list" },
    BuiltinInfo { name: "avg",   sig: "avg L n → n",                 desc: "Average of a list" },
    BuiltinInfo { name: "hd",    sig: "hd list_or_text → any",       desc: "First element" },
    BuiltinInfo { name: "tl",    sig: "tl list_or_text → list_or_text", desc: "All but first element" },
    BuiltinInfo { name: "rev",   sig: "rev list_or_text → list_or_text", desc: "Reverse" },
    BuiltinInfo { name: "srt",   sig: "srt list_or_text → list_or_text", desc: "Sort (or srt fn list)" },
    BuiltinInfo { name: "slc",   sig: "slc list_or_text n n → list_or_text", desc: "Slice from start to end index" },
    BuiltinInfo { name: "unq",   sig: "unq list_or_text → list_or_text", desc: "Remove duplicates" },
    BuiltinInfo { name: "flat",  sig: "flat L → L",                  desc: "Flatten one level" },
    BuiltinInfo { name: "has",   sig: "has list_or_text any → b",    desc: "Test membership" },
    BuiltinInfo { name: "spl",   sig: "spl t t → L t",              desc: "Split text by delimiter" },
    BuiltinInfo { name: "cat",   sig: "cat L t t → t",              desc: "Join list of text with separator" },
    BuiltinInfo { name: "map",   sig: "map fn list → list",          desc: "Apply function to each element" },
    BuiltinInfo { name: "flt",   sig: "flt fn list → list",          desc: "Filter list by predicate" },
    BuiltinInfo { name: "fld",   sig: "fld fn list any → any",       desc: "Fold/reduce list" },
    BuiltinInfo { name: "grp",   sig: "grp fn list → map",           desc: "Group list by key function" },
    BuiltinInfo { name: "rnd",   sig: "rnd → n",                     desc: "Random number [0, 1)" },
    BuiltinInfo { name: "now",   sig: "now → n",                     desc: "Current Unix timestamp" },
    BuiltinInfo { name: "rd",    sig: "rd t → R ? t",                desc: "Read file (rd path [fmt])" },
    BuiltinInfo { name: "rdl",   sig: "rdl t → R L t t",             desc: "Read file as lines" },
    BuiltinInfo { name: "rdb",   sig: "rdb t t → R ? t",             desc: "Parse string buffer" },
    BuiltinInfo { name: "wr",    sig: "wr t t → R t t",              desc: "Write text to file" },
    BuiltinInfo { name: "wrl",   sig: "wrl t L t → R t t",           desc: "Write lines to file" },
    BuiltinInfo { name: "prnt",  sig: "prnt any → any",              desc: "Print and return value" },
    BuiltinInfo { name: "env",   sig: "env t → R t t",               desc: "Read environment variable" },
    BuiltinInfo { name: "trm",   sig: "trm t → t",                   desc: "Trim whitespace" },
    BuiltinInfo { name: "fmt",   sig: "fmt t ... → t",               desc: "Format string with substitutions" },
    BuiltinInfo { name: "rgx",   sig: "rgx t t → L t",               desc: "Regex match all captures" },
    BuiltinInfo { name: "jpth",  sig: "jpth t t → R t t",            desc: "JSON path query" },
    BuiltinInfo { name: "jdmp",  sig: "jdmp any → t",                desc: "Serialize to JSON string" },
    BuiltinInfo { name: "jpar",  sig: "jpar t → R ? t",              desc: "Parse JSON string" },
    BuiltinInfo { name: "get",   sig: "get t → R t t",               desc: "HTTP GET request" },
    BuiltinInfo { name: "post",  sig: "post t t → R t t",            desc: "HTTP POST request" },
    BuiltinInfo { name: "mmap",  sig: "mmap → M t t",                desc: "Create empty map" },
    BuiltinInfo { name: "mget",  sig: "mget map t → O any",          desc: "Get map value by key" },
    BuiltinInfo { name: "mset",  sig: "mset map t any → map",        desc: "Set map key to value" },
    BuiltinInfo { name: "mhas",  sig: "mhas map t → b",              desc: "Test if map has key" },
    BuiltinInfo { name: "mkeys", sig: "mkeys map → L t",             desc: "Get all map keys" },
    BuiltinInfo { name: "mvals", sig: "mvals map → list",            desc: "Get all map values" },
    BuiltinInfo { name: "mdel",  sig: "mdel map t → map",            desc: "Delete key from map" },
];

/// All keywords that may appear at the start of a declaration or statement.
const KEYWORDS: &[&str] = &[
    "type", "tool", "use", "with", "alias",
    "ret", "brk", "cnt", "wh",
    "true", "false", "nil",
];

/// All type names for completion after `:`.
const TYPE_NAMES: &[&str] = &["n", "t", "b", "_", "L", "R", "M", "S", "F", "O"];

// ── Span ↔ LSP Position conversion ───────────────────────────────────────────

/// Convert a byte offset into (line, col) — both zero-based.
fn offset_to_position(source: &str, offset: usize) -> Position {
    let clamped = offset.min(source.len());
    let before = &source[..clamped];
    let line = before.bytes().filter(|&b| b == b'\n').count();
    let col = before
        .rfind('\n')
        .map(|p| clamped - p - 1)
        .unwrap_or(clamped);
    Position::new(line as u32, col as u32)
}

fn span_to_range(source: &str, span: Span) -> Range {
    Range::new(
        offset_to_position(source, span.start),
        offset_to_position(source, span.end),
    )
}

/// Convert an LSP position to a byte offset in the source.
fn position_to_offset(source: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut offset = 0usize;
    for (i, c) in source.char_indices() {
        if line == pos.line {
            offset = i + (pos.character as usize).min(source[i..].len());
            break;
        }
        if c == '\n' {
            line += 1;
        }
        offset = i + c.len_utf8();
    }
    // If we iterated all chars and are still on the right line, offset is at end
    offset.min(source.len())
}

/// Extract the identifier (or partial identifier) at a given byte offset.
fn ident_at(source: &str, offset: usize) -> Option<(String, usize)> {
    if offset > source.len() {
        return None;
    }
    let bytes = source.as_bytes();
    // Scan back to find start of identifier
    let mut start = offset;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    // Scan forward to find end
    let mut end = offset;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
            end += 1;
        } else {
            break;
        }
    }
    if start == end {
        return None;
    }
    Some((source[start..end].to_string(), start))
}

// ── AST walking helpers ───────────────────────────────────────────────────────

/// Walk all expressions in a statement and call `f` with each expression and its span.
fn walk_stmts_for_expr<'a>(
    stmts: &'a [Spanned<Stmt>],
    f: &mut impl FnMut(&'a Expr, Span),
) {
    for s in stmts {
        walk_stmt_for_expr(&s.node, s.span, f);
    }
}

fn walk_stmt_for_expr<'a>(
    stmt: &'a Stmt,
    span: Span,
    f: &mut impl FnMut(&'a Expr, Span),
) {
    match stmt {
        Stmt::Expr(e) => walk_expr(e, span, f),
        Stmt::Let { value, .. } => walk_expr(value, span, f),
        Stmt::Return(e) => walk_expr(e, span, f),
        Stmt::Break(Some(e)) => walk_expr(e, span, f),
        Stmt::Break(None) | Stmt::Continue => {}
        Stmt::Guard { condition, body, else_body, .. } => {
            walk_expr(condition, span, f);
            walk_stmts_for_expr(body, f);
            if let Some(eb) = else_body {
                walk_stmts_for_expr(eb, f);
            }
        }
        Stmt::Match { subject, arms } => {
            if let Some(e) = subject {
                walk_expr(e, span, f);
            }
            for arm in arms {
                walk_stmts_for_expr(&arm.body, f);
            }
        }
        Stmt::ForEach { collection, body, .. } => {
            walk_expr(collection, span, f);
            walk_stmts_for_expr(body, f);
        }
        Stmt::ForRange { start, end, body, .. } => {
            walk_expr(start, span, f);
            walk_expr(end, span, f);
            walk_stmts_for_expr(body, f);
        }
        Stmt::While { condition, body } => {
            walk_expr(condition, span, f);
            walk_stmts_for_expr(body, f);
        }
        Stmt::Destructure { value, .. } => walk_expr(value, span, f),
    }
}

fn walk_expr<'a>(expr: &'a Expr, span: Span, f: &mut impl FnMut(&'a Expr, Span)) {
    f(expr, span);
    match expr {
        Expr::Call { args, .. } => {
            for a in args {
                walk_expr(a, span, f);
            }
        }
        Expr::BinOp { left, right, .. } => {
            walk_expr(left, span, f);
            walk_expr(right, span, f);
        }
        Expr::UnaryOp { operand, .. } => walk_expr(operand, span, f),
        Expr::Ok(e) | Expr::Err(e) => walk_expr(e, span, f),
        Expr::List(items) => {
            for i in items {
                walk_expr(i, span, f);
            }
        }
        Expr::Record { fields, .. } => {
            for (_, v) in fields {
                walk_expr(v, span, f);
            }
        }
        Expr::NilCoalesce { value, default } => {
            walk_expr(value, span, f);
            walk_expr(default, span, f);
        }
        Expr::With { object, updates } => {
            walk_expr(object, span, f);
            for (_, v) in updates {
                walk_expr(v, span, f);
            }
        }
        Expr::Match { subject, arms } => {
            if let Some(s) = subject {
                walk_expr(s, span, f);
            }
            for arm in arms {
                walk_stmts_for_expr(&arm.body, f);
            }
        }
        Expr::Ternary { condition, then_expr, else_expr } => {
            walk_expr(condition, span, f);
            walk_expr(then_expr, span, f);
            walk_expr(else_expr, span, f);
        }
        Expr::Literal(_) | Expr::Ref(_) | Expr::Field { .. } | Expr::Index { .. } => {}
    }
}

// ── Compile source → diagnostics ─────────────────────────────────────────────

fn compile_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diags: Vec<Diagnostic> = Vec::new();

    // Lex
    let tokens = match lexer::lex(source) {
        Ok(t) => t,
        Err(e) => {
            let span = Span {
                start: e.position,
                end: (e.position + e.snippet.len().max(1)).min(source.len()),
            };
            diags.push(Diagnostic {
                range: span_to_range(source, span),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(e.code.to_string())),
                message: format!("unexpected token '{}'", e.snippet),
                ..Default::default()
            });
            return diags;
        }
    };

    let token_spans: Vec<_> = tokens
        .into_iter()
        .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
        .collect();

    // Parse
    let (mut program, parse_errors) = parser::parse(token_spans);
    ast::resolve_aliases(&mut program);
    program.source = Some(source.to_string());

    for e in &parse_errors {
        diags.push(Diagnostic {
            range: span_to_range(source, e.span),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(e.code.to_string())),
            message: e.message.clone(),
            ..Default::default()
        });
    }

    // Verify
    let vr = verify::verify(&program);
    for e in &vr.errors {
        let range = if let Some(span) = e.span {
            span_to_range(source, span)
        } else {
            // No span — point to start of file
            Range::new(Position::new(0, 0), Position::new(0, 0))
        };
        diags.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(e.code.to_string())),
            message: e.message.clone(),
            ..Default::default()
        });
    }
    for w in &vr.warnings {
        let range = if let Some(span) = w.span {
            span_to_range(source, span)
        } else {
            Range::new(Position::new(0, 0), Position::new(0, 0))
        };
        diags.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(w.code.to_string())),
            message: w.message.clone(),
            ..Default::default()
        });
    }

    diags
}

// ── Parse a program (best-effort) for semantic queries ───────────────────────

fn parse_program(source: &str) -> Option<Program> {
    let tokens = lexer::lex(source).ok()?;
    let token_spans: Vec<_> = tokens
        .into_iter()
        .map(|(t, r)| (t, Span { start: r.start, end: r.end }))
        .collect();
    let (mut program, _) = parser::parse(token_spans);
    ast::resolve_aliases(&mut program);
    program.source = Some(source.to_string());
    Some(program)
}

// ── Hover helpers ─────────────────────────────────────────────────────────────

/// Build hover markdown for a builtin.
fn builtin_hover(name: &str) -> Option<String> {
    BUILTIN_INFO
        .iter()
        .find(|b| b.name == name)
        .map(|b| format!("**{}**\n\n`{}`\n\n{}", b.name, b.sig, b.desc))
}

/// Format an ilo type as a string for hover.
fn type_display(ty: &Type) -> String {
    use crate::codegen::fmt::type_str;
    type_str(ty)
}

/// Build hover text for a declaration.
fn decl_hover(decl: &Decl) -> Option<String> {
    match decl {
        Decl::Function { name, params, return_type, .. } => {
            let params_str: String = params
                .iter()
                .map(|p| format!("{}:{}", p.name, type_display(&p.ty)))
                .collect::<Vec<_>>()
                .join(" ");
            Some(format!(
                "**fn** `{}`\n\n```\n{} {}>{}\n```",
                name,
                name,
                params_str,
                type_display(return_type)
            ))
        }
        Decl::TypeDef { name, fields, .. } => {
            let fields_str: String = fields
                .iter()
                .map(|f| format!("  {}:{}", f.name, type_display(&f.ty)))
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!("**type** `{}`\n\n```\ntype {} {{\n{}\n}}\n```", name, name, fields_str))
        }
        Decl::Alias { name, target, .. } => {
            Some(format!("**alias** `{}` = `{}`", name, type_display(target)))
        }
        Decl::Tool { name, description, params, return_type, .. } => {
            let params_str: String = params
                .iter()
                .map(|p| format!("{}:{}", p.name, type_display(&p.ty)))
                .collect::<Vec<_>>()
                .join(" ");
            Some(format!(
                "**tool** `{}`\n\n{}\n\n`{} {}>{}`",
                name, description, name, params_str, type_display(return_type)
            ))
        }
        _ => None,
    }
}

// ── The LSP backend ───────────────────────────────────────────────────────────

struct IloBackend {
    client: Client,
    /// uri → source text
    docs: Mutex<HashMap<String, String>>,
}

impl IloBackend {
    fn new(client: Client) -> Self {
        IloBackend {
            client,
            docs: Mutex::new(HashMap::new()),
        }
    }

    fn get_source(&self, uri: &Url) -> Option<String> {
        self.docs
            .lock()
            .ok()
            .and_then(|m| m.get(uri.as_str()).cloned())
    }

    fn set_source(&self, uri: &Url, text: String) {
        if let Ok(mut m) = self.docs.lock() {
            m.insert(uri.as_str().to_string(), text);
        }
    }

    async fn publish_diagnostics(&self, uri: Url, source: &str) {
        let diags = compile_diagnostics(source);
        self.client
            .publish_diagnostics(uri, diags, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for IloBackend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![":".to_string(), " ".to_string()]),
                    ..Default::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "ilo-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ilo LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    // ── Document lifecycle ────────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.set_source(&uri, text.clone());
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // We requested FULL sync
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.set_source(&uri, text.clone());
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Ok(mut m) = self.docs.lock() {
            m.remove(uri.as_str());
        }
        // Clear diagnostics on close
        self.client
            .publish_diagnostics(uri, vec![], None)
            .await;
    }

    // ── Hover ─────────────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let source = match self.get_source(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let offset = position_to_offset(&source, pos);
        let (ident, _) = match ident_at(&source, offset) {
            Some(i) => i,
            None => return Ok(None),
        };

        // 1. Check builtins
        if let Some(hover_text) = builtin_hover(&ident) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: None,
            }));
        }

        // 2. Check user-defined declarations
        if let Some(program) = parse_program(&source) {
            for decl in &program.declarations {
                let decl_name = match decl {
                    Decl::Function { name, .. }
                    | Decl::TypeDef { name, .. }
                    | Decl::Tool { name, .. }
                    | Decl::Alias { name, .. } => Some(name.as_str()),
                    _ => None,
                };
                if decl_name == Some(ident.as_str()) {
                    if let Some(text) = decl_hover(decl) {
                        return Ok(Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: text,
                            }),
                            range: None,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    // ── Go to definition ──────────────────────────────────────────────────────

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let source = match self.get_source(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let offset = position_to_offset(&source, pos);
        let (ident, _) = match ident_at(&source, offset) {
            Some(i) => i,
            None => return Ok(None),
        };

        if let Some(program) = parse_program(&source) {
            for decl in &program.declarations {
                let (name, span) = match decl {
                    Decl::Function { name, span, .. } => (name.as_str(), *span),
                    Decl::TypeDef { name, span, .. } => (name.as_str(), *span),
                    Decl::Tool { name, span, .. } => (name.as_str(), *span),
                    Decl::Alias { name, span, .. } => (name.as_str(), *span),
                    _ => continue,
                };
                if name == ident {
                    let range = span_to_range(&source, span);
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    })));
                }
            }
        }

        Ok(None)
    }

    // ── Completions ───────────────────────────────────────────────────────────

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> LspResult<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let source = match self.get_source(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let offset = position_to_offset(&source, pos);

        // Determine context: after `:` → type completions only
        let after_colon = offset > 0 && {
            let before = &source[..offset];
            before.trim_end_matches(|c: char| c.is_alphanumeric())
                .ends_with(':')
        };

        let mut items: Vec<CompletionItem> = Vec::new();

        if after_colon {
            // Type completions
            for &ty in TYPE_NAMES {
                items.push(CompletionItem {
                    label: ty.to_string(),
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    ..Default::default()
                });
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // User-defined functions and types
        if let Some(program) = parse_program(&source) {
            for decl in &program.declarations {
                match decl {
                    Decl::Function { name, params, return_type, .. } => {
                        let params_str: String = params
                            .iter()
                            .map(|p| format!("{}:{}", p.name, type_display(&p.ty)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some(format!("{} {}>{}", name, params_str, type_display(return_type))),
                            ..Default::default()
                        });
                    }
                    Decl::TypeDef { name, .. } => {
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::CLASS),
                            ..Default::default()
                        });
                    }
                    Decl::Tool { name, description, .. } => {
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some(description.clone()),
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
            }
        }

        // Builtins
        for b in BUILTIN_INFO {
            items.push(CompletionItem {
                label: b.name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(b.sig.to_string()),
                documentation: Some(Documentation::String(b.desc.to_string())),
                ..Default::default()
            });
        }

        // Keywords
        for &kw in KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    // ── Document symbols ──────────────────────────────────────────────────────

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let source = match self.get_source(uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let program = match parse_program(&source) {
            Some(p) => p,
            None => return Ok(None),
        };

        let mut symbols: Vec<SymbolInformation> = Vec::new();
        for decl in &program.declarations {
            let (name, kind, span) = match decl {
                Decl::Function { name, span, .. } => (name.as_str(), SymbolKind::FUNCTION, *span),
                Decl::TypeDef { name, span, .. } => (name.as_str(), SymbolKind::CLASS, *span),
                Decl::Tool { name, span, .. } => (name.as_str(), SymbolKind::INTERFACE, *span),
                Decl::Alias { name, span, .. } => (name.as_str(), SymbolKind::TYPE_PARAMETER, *span),
                _ => continue,
            };
            let range = span_to_range(&source, span);
            #[allow(deprecated)]
            symbols.push(SymbolInformation {
                name: name.to_string(),
                kind,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                container_name: None,
                tags: None,
            });
        }

        Ok(Some(DocumentSymbolResponse::Flat(symbols)))
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Start the LSP server on stdin/stdout. Blocks until the client disconnects.
pub fn run() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let (service, socket) = LspService::new(|client| IloBackend::new(client));
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
