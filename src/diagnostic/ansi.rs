use crate::ast::SourceMap;
use super::{Diagnostic, Severity};

pub struct AnsiRenderer {
    pub use_color: bool,
}

impl AnsiRenderer {
    fn bold(&self, s: &str) -> String {
        if self.use_color { format!("\x1b[1m{s}\x1b[0m") } else { s.to_string() }
    }

    fn bold_red(&self, s: &str) -> String {
        if self.use_color { format!("\x1b[1;31m{s}\x1b[0m") } else { s.to_string() }
    }

    fn cyan(&self, s: &str) -> String {
        if self.use_color { format!("\x1b[36m{s}\x1b[0m") } else { s.to_string() }
    }

    fn dim(&self, s: &str) -> String {
        if self.use_color { format!("\x1b[2m{s}\x1b[0m") } else { s.to_string() }
    }

    pub fn render(&self, d: &Diagnostic) -> String {
        let mut out = String::new();

        // "error: message"
        let severity_label = match d.severity {
            Severity::Error => self.bold_red("error"),
            Severity::Warning => self.bold(&self.cyan("warning")),
        };
        out.push_str(&format!("{}: {}\n", severity_label, self.bold(&d.message)));

        // Render primary label with source snippet
        let primary = d.labels.iter().find(|l| l.is_primary);
        if let (Some(label), Some(source)) = (primary, &d.source) {
            let map = SourceMap::new(source);
            let (line, col) = map.lookup(label.span.start);
            let line_text = map.line_text(source, line);

            // "  --> line:col"
            out.push_str(&format!("  {} {}:{}\n", self.cyan("-->"), line, col));

            // Gutter width based on line number digits
            let gutter = line.to_string().len();
            let pipe = self.cyan("|");
            let pad = " ".repeat(gutter);

            // Empty gutter line
            out.push_str(&format!("{pad} {pipe}\n"));

            // Source line
            let line_num = self.cyan(&format!("{line:>gutter$}"));
            out.push_str(&format!("{line_num} {pipe} {line_text}\n"));

            // Caret line
            let span_start_in_line = col.saturating_sub(1);
            let span_len = (label.span.end.saturating_sub(label.span.start)).max(1);
            let carets = self.bold_red(&"^".repeat(span_len));
            let indent = " ".repeat(span_start_in_line);
            if label.message.is_empty() {
                out.push_str(&format!("{pad} {pipe} {indent}{carets}\n"));
            } else {
                out.push_str(&format!("{pad} {pipe} {indent}{carets} {}\n",
                    self.bold_red(&label.message)));
            }

            // Empty gutter line after
            out.push_str(&format!("{pad} {pipe}\n"));
        }

        // Secondary labels (no source snippet, just mention span)
        for label in d.labels.iter().filter(|l| !l.is_primary) {
            if !label.message.is_empty() {
                out.push_str(&format!("  {} {}\n", self.dim("="), label.message));
            }
        }

        // Notes
        for note in &d.notes {
            out.push_str(&format!("  {} note: {}\n", self.dim("="), note));
        }

        // Suggestion
        if let Some(suggestion) = &d.suggestion {
            out.push_str(&format!("  {} suggestion: {}\n", self.dim("="), suggestion));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Span;

    fn make_diag(source: &str, start: usize, end: usize) -> Diagnostic {
        Diagnostic::error("type mismatch")
            .with_span(Span { start, end }, "here")
            .with_source(source.to_string())
            .with_note("in function 'f'")
            .with_suggestion("use n instead of t")
    }

    #[test]
    fn render_contains_error_label() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains("error:"), "missing 'error:' in:\n{out}");
        assert!(out.contains("type mismatch"), "missing message in:\n{out}");
    }

    #[test]
    fn render_contains_location() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains("-->"), "missing '-->' in:\n{out}");
        assert!(out.contains("1:"), "missing line number in:\n{out}");
    }

    #[test]
    fn render_contains_source_line() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains("f x:n>n;x"), "missing source line in:\n{out}");
    }

    #[test]
    fn render_contains_carets() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains('^'), "missing carets in:\n{out}");
    }

    #[test]
    fn render_contains_note_and_suggestion() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains("note:"), "missing note in:\n{out}");
        assert!(out.contains("suggestion:"), "missing suggestion in:\n{out}");
        assert!(out.contains("in function 'f'"), "missing note text in:\n{out}");
    }

    #[test]
    fn render_no_source_still_works() {
        let r = AnsiRenderer { use_color: false };
        let d = Diagnostic::error("something bad");
        let out = r.render(&d);
        assert!(out.contains("error: something bad"));
        // No source → no snippet
        assert!(!out.contains("-->"));
    }

    #[test]
    fn render_with_color_contains_ansi_codes() {
        let r = AnsiRenderer { use_color: true };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(out.contains("\x1b["), "expected ANSI codes when use_color=true");
    }

    #[test]
    fn render_without_color_no_ansi_codes() {
        let r = AnsiRenderer { use_color: false };
        let d = make_diag("f x:n>n;x", 2, 3);
        let out = r.render(&d);
        assert!(!out.contains("\x1b["), "unexpected ANSI codes when use_color=false");
    }

    #[test]
    fn render_multiline_source_correct_line() {
        let source = "f x:n>n;x\ng y:t>t;y";
        let r = AnsiRenderer { use_color: false };
        // Error on 'g' at byte 10 (start of second line)
        let d = Diagnostic::error("bad")
            .with_span(Span { start: 10, end: 11 }, "here")
            .with_source(source.to_string());
        let out = r.render(&d);
        assert!(out.contains("2:"), "expected line 2 in:\n{out}");
        assert!(out.contains("g y:t>t;y"), "expected second line in:\n{out}");
    }

    #[test]
    fn caret_length_matches_span() {
        let r = AnsiRenderer { use_color: false };
        // span covers 3 bytes ("x:n")
        let d = Diagnostic::error("bad")
            .with_span(Span { start: 2, end: 5 }, "")
            .with_source("f x:n>n;x".to_string());
        let out = r.render(&d);
        assert!(out.contains("^^^"), "expected 3 carets in:\n{out}");
    }
}
