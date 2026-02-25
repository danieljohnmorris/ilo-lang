# Idea 1 Compact: ilo (single-line syntax)

ilo compact is the same language as idea1 but compressed onto single lines for token efficiency.

- `fn name(params)->return_type` declares a function, body follows after `;`
- `type name{fields}` declares a record type with `;`-separated fields
- `tool name"description"(params)->return_type timeout:N,retry:N`
- `@dep from source` imports, comma-separated in parens
- Semicolons replace newlines; braces replace indentation
- `?input==expected` for inline tests
- `match value{arm:expr;arm:expr}` for pattern matching
- `for x in collection{body}` for iteration
- `err"msg"` / `ok value` for results (spaces optional)
- No `let` keyword — bare `name=expr` for bindings
- Comments start with `--`
