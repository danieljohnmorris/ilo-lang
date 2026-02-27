/// Maps byte offsets to line/column positions within source text.
pub struct SourceMap {
    line_starts: Vec<usize>,
}

impl SourceMap {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        SourceMap { line_starts }
    }

    /// Returns (line, col), both 1-based.
    pub fn lookup(&self, offset: usize) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        (line + 1, col + 1)
    }

    /// Returns the full text of the given 1-based line number.
    pub fn line_text<'a>(&self, source: &'a str, line: usize) -> &'a str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line - 1];
        let end = if line < self.line_starts.len() {
            self.line_starts[line]
        } else {
            source.len()
        };
        // Trim trailing newline
        let text = &source[start..end];
        text.trim_end_matches('\n').trim_end_matches('\r')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let src = "f x:n>n;*x 2";
        let sm = SourceMap::new(src);
        assert_eq!(sm.lookup(0), (1, 1));
        assert_eq!(sm.lookup(2), (1, 3));
        assert_eq!(sm.lookup(12), (1, 13));
    }

    #[test]
    fn multi_line() {
        let src = "line one\nline two\nline three";
        let sm = SourceMap::new(src);
        assert_eq!(sm.lookup(0), (1, 1));   // 'l' of "line one"
        assert_eq!(sm.lookup(8), (1, 9));   // '\n' after "line one"
        assert_eq!(sm.lookup(9), (2, 1));   // 'l' of "line two"
        assert_eq!(sm.lookup(18), (3, 1));  // 'l' of "line three"
    }

    #[test]
    fn line_text_single() {
        let src = "f x:n>n;*x 2";
        let sm = SourceMap::new(src);
        assert_eq!(sm.line_text(src, 1), "f x:n>n;*x 2");
    }

    #[test]
    fn line_text_multi() {
        let src = "first\nsecond\nthird";
        let sm = SourceMap::new(src);
        assert_eq!(sm.line_text(src, 1), "first");
        assert_eq!(sm.line_text(src, 2), "second");
        assert_eq!(sm.line_text(src, 3), "third");
    }

    #[test]
    fn line_text_out_of_bounds() {
        let src = "hello";
        let sm = SourceMap::new(src);
        assert_eq!(sm.line_text(src, 0), "");
        assert_eq!(sm.line_text(src, 99), "");
    }

    #[test]
    fn empty_source() {
        let src = "";
        let sm = SourceMap::new(src);
        assert_eq!(sm.lookup(0), (1, 1));
        assert_eq!(sm.line_text(src, 1), "");
    }

    #[test]
    fn trailing_newline() {
        let src = "hello\n";
        let sm = SourceMap::new(src);
        assert_eq!(sm.line_text(src, 1), "hello");
        assert_eq!(sm.line_text(src, 2), "");
    }

    #[test]
    fn offset_at_newline_boundary() {
        let src = "ab\ncd\nef";
        let sm = SourceMap::new(src);
        // offset 2 = '\n', belongs to line 1
        assert_eq!(sm.lookup(2), (1, 3));
        // offset 3 = 'c', line 2 col 1
        assert_eq!(sm.lookup(3), (2, 1));
        // offset 5 = '\n', belongs to line 2
        assert_eq!(sm.lookup(5), (2, 3));
        // offset 6 = 'e', line 3 col 1
        assert_eq!(sm.lookup(6), (3, 1));
    }
}
