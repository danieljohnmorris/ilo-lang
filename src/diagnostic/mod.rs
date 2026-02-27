pub mod ansi;
pub mod json;

use crate::ast::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    #[allow(dead_code)] // forward infrastructure for future warning diagnostics
    Warning,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestion: Option<String>,
    pub source: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            suggestion: None,
            source: None,
        }
    }

    pub fn with_span(mut self, span: Span, label: impl Into<String>) -> Self {
        self.labels.push(Label { span, message: label.into(), is_primary: true });
        self
    }

    #[allow(dead_code)] // forward infrastructure for multi-label diagnostics (C3+)
    pub fn with_secondary_span(mut self, span: Span, label: impl Into<String>) -> Self {
        self.labels.push(Label { span, message: label.into(), is_primary: false });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

// ---- From impls for existing error types ----

impl From<&crate::lexer::LexError> for Diagnostic {
    fn from(e: &crate::lexer::LexError) -> Self {
        let span = Span {
            start: e.position,
            end: e.position + e.snippet.len().max(1),
        };
        let mut d = Diagnostic::error(format!("unexpected token '{}'", e.snippet))
            .with_span(span, "here");
        if !e.suggestion.is_empty() {
            d = d.with_suggestion(e.suggestion.clone());
        }
        d
    }
}

impl From<&crate::parser::ParseError> for Diagnostic {
    fn from(e: &crate::parser::ParseError) -> Self {
        Diagnostic::error(&e.message).with_span(e.span, "here")
    }
}

impl From<&crate::verify::VerifyError> for Diagnostic {
    fn from(e: &crate::verify::VerifyError) -> Self {
        let mut d = Diagnostic::error(&e.message)
            .with_note(format!("in function '{}'", e.function));
        if let Some(hint) = &e.hint {
            d = d.with_suggestion(hint.clone());
        }
        d
    }
}

impl From<&crate::interpreter::RuntimeError> for Diagnostic {
    fn from(e: &crate::interpreter::RuntimeError) -> Self {
        Diagnostic::error(&e.message)
    }
}

impl From<&crate::vm::VmError> for Diagnostic {
    fn from(e: &crate::vm::VmError) -> Self {
        Diagnostic::error(e.to_string())
    }
}

impl From<&crate::vm::CompileError> for Diagnostic {
    fn from(e: &crate::vm::CompileError) -> Self {
        Diagnostic::error(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Span;

    #[test]
    fn diagnostic_error_builder() {
        let d = Diagnostic::error("something went wrong");
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "something went wrong");
        assert!(d.labels.is_empty());
        assert!(d.notes.is_empty());
        assert!(d.suggestion.is_none());
    }

    #[test]
    fn diagnostic_with_span() {
        let d = Diagnostic::error("bad token")
            .with_span(Span { start: 5, end: 8 }, "here");
        assert_eq!(d.labels.len(), 1);
        assert_eq!(d.labels[0].span.start, 5);
        assert_eq!(d.labels[0].span.end, 8);
        assert!(d.labels[0].is_primary);
    }

    #[test]
    fn diagnostic_with_note_and_suggestion() {
        let d = Diagnostic::error("type mismatch")
            .with_note("in function 'foo'")
            .with_suggestion("use n instead of t");
        assert_eq!(d.notes, vec!["in function 'foo'"]);
        assert_eq!(d.suggestion.as_deref(), Some("use n instead of t"));
    }

    #[test]
    fn from_lex_error() {
        let e = crate::lexer::LexError {
            position: 3,
            snippet: "my_func".to_string(),
            suggestion: "Use hyphens: 'my-func'".to_string(),
        };
        let d = Diagnostic::from(&e);
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("my_func"));
        assert_eq!(d.labels[0].span.start, 3);
        assert_eq!(d.labels[0].span.end, 10); // 3 + len("my_func")
        assert!(d.suggestion.is_some());
    }

    #[test]
    fn from_parse_error() {
        let e = crate::parser::ParseError {
            position: 2,
            span: Span { start: 10, end: 15 },
            message: "expected identifier".to_string(),
        };
        let d = Diagnostic::from(&e);
        assert!(d.message.contains("expected identifier"));
        assert_eq!(d.labels[0].span, Span { start: 10, end: 15 });
    }

    #[test]
    fn from_verify_error() {
        let e = crate::verify::VerifyError {
            function: "myFunc".to_string(),
            message: "undefined variable 'x'".to_string(),
            hint: Some("did you mean 'y'?".to_string()),
        };
        let d = Diagnostic::from(&e);
        assert!(d.message.contains("undefined variable"));
        assert!(d.notes.iter().any(|n| n.contains("myFunc")));
        assert!(d.suggestion.is_some());
    }

    #[test]
    fn from_runtime_error() {
        let e = crate::interpreter::RuntimeError { message: "division by zero".to_string() };
        let d = Diagnostic::from(&e);
        assert!(d.message.contains("division by zero"));
        assert!(d.labels.is_empty()); // no span for runtime errors
    }

    #[test]
    fn from_vm_error() {
        let e = crate::vm::VmError::UndefinedFunction { name: "foo".to_string() };
        let d = Diagnostic::from(&e);
        assert!(d.message.contains("foo"));
    }

    #[test]
    fn from_compile_error() {
        let e = crate::vm::CompileError::UndefinedVariable { name: "x".to_string() };
        let d = Diagnostic::from(&e);
        assert!(d.message.contains("x"));
    }
}
