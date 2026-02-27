# ilo

*ilo* — Toki Pona for "tool" ([sona.pona.la/wiki/ilo](https://sona.pona.la/wiki/ilo)). A constructed language for AI agents.

Languages were designed for humans — visual parsing, readable syntax, spatial navigation. AI agents are not humans. They generate tokens. Every token costs latency, money, and context window. The only metric that matters is **total tokens from intent to working code**.

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

## What It Looks Like

Python:
```python
def total(price: float, quantity: int, rate: float) -> float:
    sub = price * quantity
    tax = sub * rate
    return sub + tax
```

ilo (idea9 — ultra-dense-short):
```
tot p:n q:n r:n>n;s=*p q;t=*s r;+s t
```

0.33x the tokens, 0.22x the characters. Same semantics.

### Why prefix notation?

ilo uses prefix notation (`+a b` instead of `a + b`). Nesting eliminates parentheses entirely:

```
(a * b) + c       →  +*a b c        -- saves 4 chars, 1 token
((a + b) * c) >= 100  →  >=*+a b c 100  -- saves 7 chars, 3 tokens
```

Across 25 expression patterns: **22% fewer tokens, 42% fewer characters** vs infix. See the [prefix-vs-infix benchmark](research/explorations/prefix-vs-infix/).

## Principles

1. **Token-conservative** — every choice evaluated against total token cost across the full loop: generation, retries, error feedback, context loading.
2. **Constrained** — small vocabulary, closed world, one way to do things. Fewer valid next-tokens = fewer wrong choices = fewer retries.
3. **Self-contained** — each unit carries its own context: deps, types, rules. The spec travels with the program.
4. **Language-agnostic** — structural tokens (`@`, `>`, `?`, `^`, `~`, `!`) over English words.
5. **Graph-native** — programs express relationships (calls, depends-on, has-type). Navigable as a graph, not just readable as linear text.

See [MANIFESTO.md](MANIFESTO.md) for the full rationale.

## Syntax Variants

Each idea explores a different syntax. Every folder has a SPEC and 5 example programs.

| Idea | Tokens | vs Py | Chars | vs Py | Score |
|------|--------|-------|-------|-------|-------|
| python-baseline | 871 | 1.00x | 3635 | 1.00x | — |
| [idea1-basic](research/explorations/idea1-basic/) | 921 | 1.06x | 3108 | 0.86x | 10.0 |
| [idea1-compact](research/explorations/idea1-compact/) | 677 | 0.78x | 2564 | 0.71x | 10.0 |
| [idea2-tool-calling](research/explorations/idea2-tool-calling/) | 983 | 1.13x | 3203 | 0.88x | 10.0 |
| [idea3-constrained-decoding](research/explorations/idea3-constrained-decoding/) | 598 | 0.69x | 2187 | 0.60x | 10.0 |
| [idea4-ast-bytecode](research/explorations/idea4-ast-bytecode/) | 584 | 0.67x | 1190 | 0.33x | 9.8 |
| [idea5-workflow-dag](research/explorations/idea5-workflow-dag/) | 710 | 0.82x | 2603 | 0.72x | 10.0 |
| [idea6-mcp-composition](research/explorations/idea6-mcp-composition/) | 956 | 1.10x | 2978 | 0.82x | 9.5 |
| [idea7-dense-wire](research/explorations/idea7-dense-wire/) | 351 | 0.40x | 1292 | 0.36x | 10.0 |
| [idea8-ultra-dense](research/explorations/idea8-ultra-dense/) | 285 | 0.33x | 901 | 0.25x | 10.0 |
| [idea9-ultra-dense-short](research/explorations/idea9-ultra-dense-short/) | 287 | 0.33x | 787 | 0.22x | 10.0 |

Score = LLM generation accuracy /10 (claude-haiku-4-5, spec + all examples as context). See [test-summary.txt](research/explorations/test-summary.txt) for per-task breakdown.

## Install

**One-liner (macOS / Linux):**
```bash
curl -fsSL https://raw.githubusercontent.com/danieljohnmorris/ilo-lang/main/install.sh | sh
```

**Direct download (example: macOS Apple Silicon):**
```bash
curl -fsSL https://github.com/danieljohnmorris/ilo-lang/releases/latest/download/ilo-aarch64-apple-darwin -o /usr/local/bin/ilo && chmod +x /usr/local/bin/ilo
```

**From source (developers):**
```bash
cargo install --git https://github.com/danieljohnmorris/ilo-lang
```

## Running

**Run inline code:**
```bash
ilo 'tot p:n q:n r:n>n;s=*p q;t=*s r;+s t' 10 20 30
# → 6200
```

No flags needed. The first arg is code (or a file path — auto-detected). Remaining args are passed to the first function. To select a specific function in multi-function programs, name it:

```bash
ilo 'dbl x:n>n;s=*x 2;+s 0 tot p:n q:n r:n>n;s=*p q;t=*s r;+s t' tot 10 20 30
```

**Pass list arguments** with commas:
```bash
ilo 'f xs:L n>n;len xs' 1,2,3         # → 3
ilo 'f xs:L t>t;xs.0' 'a,b,c'         # → a
```

**Run from a file:**
```bash
ilo program.ilo 10 20 30
```

**Help & language spec:**
```bash
ilo help                     # usage and examples
ilo -h                       # same as ilo help
ilo help lang                # print the full language specification
ilo help ai                  # compact spec for LLM consumption (~16 lines)
ilo -ai                      # same as ilo help ai
```

**Backends:**

By default, ilo uses Cranelift JIT and falls back to the interpreter for non-JIT-eligible functions.

```bash
ilo 'code' args              # default: Cranelift JIT → interpreter fallback
ilo 'code' --run-interp ...  # tree-walking interpreter
ilo 'code' --run-vm ...      # register VM
ilo 'code' --run-cranelift . # Cranelift JIT
ilo 'code' --run-jit ...     # custom ARM64 JIT (macOS Apple Silicon only)
```

**Static verification:**

All programs are verified before execution. The verifier checks function existence, arity, variable scope, type compatibility, record fields, and more — reporting all errors at once:

```bash
ilo 'f x:n>n;*y 2' 5
# verify: undefined variable 'y' in 'f'
#   hint: did you mean 'x'?

ilo 'f x:t>n;*x 2' hello
# verify: '*' expects n and n, got t and n in 'f'
```

This matches the manifesto: "verification before execution — all calls resolve, all types align, all dependencies exist."

**Other modes:**
```bash
ilo 'code' --emit python     # transpile to Python
ilo 'code'                    # no args → AST JSON
ilo program.ilo --bench tot 10 20 30  # benchmark
```

**Run tests:**
```bash
cargo test
```

459 tests: lexer, parser, interpreter, VM, verifier, codegen, diagnostic, and CLI integration tests.

## Documentation

| Document | Purpose |
|----------|---------|
| [SPEC.md](SPEC.md) | Language specification |
| [MANIFESTO.md](MANIFESTO.md) | Design rationale |
| [research/TODO.md](research/TODO.md) | Planned work |
| [research/OPEN.md](research/OPEN.md) | Open design questions |
| [research/BUILDING-A-LANGUAGE.md](research/BUILDING-A-LANGUAGE.md) | How to build a language — research & plan |

```
  _  _          _
 (_)| | ___    | |  __ _  _ __    __ _
 | || |/ _ \   | | / _` || '_ \  / _` |
 | || | (_) |  | || (_| || | | || (_| |
 |_||_|\___/   |_| \__,_||_| |_| \__, |
                                   |___/
```
