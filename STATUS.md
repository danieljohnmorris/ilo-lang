# ilo — Current State

## Documents

| File | Purpose |
|------|---------|
| [README.md](README.md) | Overview and quick example |
| [MANIFESTO.md](MANIFESTO.md) | Five principles and design rationale |
| [SPEC.md](SPEC.md) | Language syntax, types, operators, naming rules |
| [OPEN.md](OPEN.md) | Unresolved questions (execution model, graph loading, interop) |

## Examples

```
examples/
├── python-baseline/              # Python equivalents for token comparison
├── idea1/                        # Current ilo syntax, multiline (1.06x Python)
├── idea1-compact/                # Same syntax, no newlines (0.78x Python)
├── idea2-tool-calling/           # JSON tool call sequence
├── idea3-constrained-decoding/   # Grammar-constrained JSON
├── idea4-ast-bytecode/           # AST bytecode
├── idea5-workflow-dag/           # YAML workflow DAG
├── idea6-mcp-composition/        # MCP composition layer
├── idea7-dense-wire/             # Dense wire format
└── compare.py                    # Token counting script (cl100k_base)
```

**idea1** and **idea1-compact** have all 5 examples. **idea2–idea7** are partial explorations. Run `python3 examples/compare.py` for current token counts.

## Rust Implementation

| File | Status |
|------|--------|
| `src/lexer/mod.rs` | Working lexer (logos). **Out of sync** — uses verbose syntax |
| `src/ast/mod.rs` | Full AST definition. **Out of sync** — matches verbose syntax |
| `src/main.rs` | Stub |
| `src/parser/` | Empty |
| `src/runtime/` | Empty |

## Decisions Made

1. **Five principles**: token-conservative, constrained, self-contained, language-agnostic, graph-native
2. **Terse syntax**: `fn`/`type`/`tool`, `@` deps, `->` return, `?` tests, `ok`/`err`, prefix operators, tab indentation
3. **Naming**: prefer single words. Hyphens only when ambiguous. Never abbreviate.
4. **Error handling**: `result T, E` only. No exceptions, no null.
5. **One spec doc**: SPEC.md serves both agents and humans.

## Decisions Not Yet Made

See [OPEN.md](OPEN.md) for details.

1. Execution model (graph engine vs tool orchestration vs transpilation)
2. Graph loading (full graph vs query-on-demand vs progressive disclosure)
3. Interop (how tools connect to real systems)
4. `let` keyword (keep or drop)
5. Scope (general-purpose language vs workflow composition layer on MCP)

## Key Insight

ilo beats Python on **complex programs** (error handling, tool calls, control flow) but not on simple math. Python's boilerplate (`isinstance`, `await`, `f-strings`, `.value`, `.error`) adds up while ilo's `match`/`ok`/`err` is compact. The more error handling and tool orchestration, the bigger ilo's advantage.
