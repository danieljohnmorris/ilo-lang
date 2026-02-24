# ilo — Current State

## What Exists

### Documents
| File | Purpose |
|------|---------|
| `README.md` | Overview, quick example, links |
| `MANIFESTO.md` | Five principles, design rationale, dropped principles |
| `SPEC.md` | Language syntax, types, operators, naming rules |
| `OPEN.md` | Unresolved questions (execution model, graph loading, interop) |
| `STATUS.md` | This file |

### Examples

```
examples/
├── idea1/                        # Current ilo syntax (0.72x Python)
├── idea2-tool-calling/           # JSON tool call sequence (0.85x Python)
├── idea3-constrained-decoding/   # Grammar-constrained JSON (0.85x Python)
├── idea4-ast-bytecode/           # AST bytecode (1.39x Python)
├── idea5-workflow-dag/           # YAML workflow DAG (1.13x Python)
├── idea6-mcp-composition/        # MCP composition layer (0.80x Python)
└── idea7-dense-wire/             # Dense wire format (0.76x Python)
```

**idea1** contains the current ilo syntax examples, evaluation, and grammar notes.
**idea2–idea7** are alternative approaches explored for comparison.

### Rust Implementation
| File | Status |
|------|--------|
| `src/lexer/mod.rs` | Working lexer (logos). **Out of sync** — uses verbose syntax (`define function`, word operators) |
| `src/ast/mod.rs` | Full AST definition. **Out of sync** — matches verbose syntax |
| `src/main.rs` | Stub |
| `src/parser/` | Empty |
| `src/runtime/` | Empty |
| `src/stdlib/` | Empty |

## Decisions Made

1. **Five principles**: token-conservative, constrained, self-contained, language-agnostic, graph-native
2. **Terse syntax**: `fn`/`type`/`tool`, `@` deps, `->` return, `?` tests, `ok`/`err`, prefix operators, tab indentation
3. **Naming**: prefer single words. Hyphens only when ambiguous. Never abbreviate.
4. **Error handling**: `result T, E` only. No exceptions, no null.
5. **One spec doc**: SPEC.md serves both agents and humans.
6. **Current ilo syntax wins on tokens**: 0.72x Python for realistic programs (tool interaction, error handling). All alternative approaches (JSON, YAML, bytecode) are worse.

## Decisions Not Yet Made

1. **Execution model**: graph engine vs tool orchestration engine vs transpilation
2. **Graph loading**: full graph vs query-on-demand vs progressive disclosure
3. **Interop**: how tools connect to real systems (MCP? auto-discovery?)
4. **`let` keyword**: keep or drop
5. **Scope**: general-purpose language vs verified workflow composition layer on MCP

## Key Insight From Token Analysis

ilo is cheaper than Python for **complex programs** (error handling, tool calls, control flow) but more expensive for **simple math**. The more error handling and tool orchestration, the bigger ilo's advantage — because Python's boilerplate (`isinstance`, `await`, `f-strings`, `.value`, `.error`) adds up while ilo's `match`/`ok`/`err` is compact.

The tokeniser is biased toward existing language patterns. Novel syntax will never tokenise as efficiently as Python for the constructs Python is good at. ilo wins by having constructs Python doesn't — inline tests, dependency declarations, forced error handling — that reduce total cost even though they add generation tokens.

## Commit History

```
87846fd Add concrete AST bytecode example, correct token counts
974bfd2 Add exploration files for alternative approaches
431e53d Add graph loading and interop questions to OPEN.md
a5eb20d Move unresolved questions to OPEN.md
e7c0636 Merge spec and reference into single SPEC.md
bba2f09 Add language spec and reference, fix examples and docs
43f70b5 Prefer single-word names, add naming rule to docs
8d52a84 Replace verbose examples with terse syntax
61cfc92 Finalise five principles, add example programs and evaluation
f698f27 Show reference URL instead of linking word
73b690f Add Rust CI workflow
511aca4 Link ilo definition to Toki Pona wiki, remove 'programming' framing
edfecb1 Initial commit: ilo manifesto and project structure
```
