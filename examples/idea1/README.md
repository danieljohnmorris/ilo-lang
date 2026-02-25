# Idea 1: ilo (indented syntax)

ilo as an indented, whitespace-significant language inspired by Haskell/Elm.

- `fn name` declares a function, indented body
- `type name` declares a record type with named fields
- `tool name "description"` declares an external tool with timeout/retry
- `@ dep from source` imports dependencies
- Parameters: `name: type` with `->` return type
- `?` lines are inline tests (input == expected)
- `let x = expr` for bindings
- `match value` with indented arms for pattern matching
- `for x in collection` for iteration
- `err "msg"` / `ok value` for result types
- `result T, E` for Result return types
- Comments start with `--`
