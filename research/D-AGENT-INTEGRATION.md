# Phase D: Agent Integration — Design Notes

Captured design thinking from Phase D exploration sessions. For the roadmap, see [TODO.md](../TODO.md). For open questions, see [OPEN.md](OPEN.md).

## D1: Type System Decisions

### No new types for D1

- `null` → `nil` (already exists as `_` type)
- Unknown shape → `t` escape hatch (tool returns text, caller parses)
- Known shape → records (already exist as `type name{field:type;...}`)
- **Deferred to Phase E:** optionals, maps, sum types

### Result type is sufficient for tool calls

Tools return `R ok err` — covers success/failure, wraps structured data in `~v`, surfaces errors in `^e`. The `!` auto-unwrap operator eliminates match boilerplate.

## MCP Alignment

- ilo records map directly to JSON Schema objects — field names and types correspond 1:1
- MCP dual-track output: `structuredContent` (typed) + `content` text (display)
- Tool declarations ARE schemas — `tool name"desc" params>return` is a schema declaration
- Format parsing (JSON, HTML, XML, YAML) is a **tool concern**, not a language concern. ilo composes typed tool results. Only JSON ↔ Value mapping needed at the tool boundary.

## "Constrained" Principle Gap

The closed-world verifier only works when tool signatures come from reality (MCP discovery), not just agent-authored declarations. `ToolProvider` should eventually be a **signature source**, not just an executor — feeding discovered tool schemas into the verifier's function table.

## Graph-Native Concrete Ideas

The verifier already computes a call graph internally but discards it. Opportunities:

- **Expose the call graph** as a first-class query result
- **Subgraph execution** — run function X + transitive deps only (deploy a slice)
- **Impact analysis** — "what breaks if tool Y is unavailable?" (remove node, find unreachable paths)
- **Program composition** — merge declaration graphs from multiple sources with conflict checking (duplicate names, incompatible signatures)

## Design Space Positioning

ilo sits in the **"schema-validated + dynamic types"** quadrant alongside MCP `outputSchema`:

- Not as rigid as Rust serde (no compile-time generics, no lifetimes)
- Not as loose as bash text (types are checked, errors are structured)
- Closest analogue: Nushell/PowerShell structured pipelines (objects/tables between stages vs bash text pipes). ilo's `?` operator is the typed pipe equivalent.

## Naming Convention

- `get` at 3 chars fits the builtin pattern (`len`, `str`, `num`, `abs`, `min`, `max`, `flr`, `cel`)
- `g` rejected as ambiguous
- Builtins stay terse but readable — single English words preferred over abbreviations

## Conciseness Benchmarks

Auto-unwrap example: `d=get! url;d.name` (16 chars) beats:
- `curl -s url | jq '.name'` (25 chars) — AND adds verification + error handling
- Python equivalent: ~80 tokens / 4 lines (requests + json + error handling)
