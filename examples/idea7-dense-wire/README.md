# Idea 7: Dense Wire Format

ilo as a compressed text format optimised for tokeniser efficiency. Spaces removed where tokeniser merges help. Semicolons instead of newlines. No `fn`/`let` keywords — just bare assignments.

- Maximally terse — function names double as declarations
- Positional-style args (space-separated, no commas)
- Braces for blocks instead of indentation
- Looks ugly. Optimised for token count, not readability.
