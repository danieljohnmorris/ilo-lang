# Prefix vs Infix: Token & Character Savings

ilo uses prefix notation (`+a b`) instead of infix (`a + b`). This exploration measures the actual token and character savings across real expression patterns.

## Key insight

Prefix notation saves **characters** (no spaces around operators, no parentheses for nesting) but token savings depend on the tokenizer. The `cl100k_base` tokenizer used by Claude often merges operator+operand sequences differently than spaced infix.

## Results

Run the benchmark:
```bash
python3 research/explorations/prefix-vs-infix/bench.py
```

## Patterns tested

| Pattern | Infix | Prefix | Char savings |
|---------|-------|--------|-------------|
| Binary op | `a + b` | `+a b` | 1 char |
| Nested (2 deep) | `(a * b) + c` | `+*a b c` | 4 chars (2 parens + 2 spaces) |
| Nested (3 deep) | `((a + b) * c) >= 100` | `>=*+a b c 100` | 6 chars |
| Chained comparison | `x >= 0 and x <= 100` | `&>=x 0 <=x 100` | 4 chars |
| Negation | `not (a == b)` | `!=a b` | 8 chars (or `!= a b` → `!=a b`) |
| Complex guard | `if not d.verified:` | `!d.verified{...}` | varies |

Each nested operator saves 2 characters (no `(` `)` needed). Flat binary expressions save 1 character vs infix.
