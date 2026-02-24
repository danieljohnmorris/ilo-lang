# ilo

*ilo* — Toki Pona for "tool" ([sona.pona.la/wiki/ilo](https://sona.pona.la/wiki/ilo)). A constructed language for AI agents.

Languages were designed for humans — visual parsing, readable syntax, spatial navigation. AI agents are not humans. They generate tokens. Every token costs latency, money, and context window. The only metric that matters is **total tokens from intent to working code**.

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

## Five Principles

1. **Token-conservative** — the north star. Every choice evaluated against total token cost across the full loop: generation, retries, error feedback, context loading. Not just "short syntax."

2. **Constrained** — small vocabulary, closed world, one way to do things. Fewer valid next-tokens = fewer wrong choices = fewer retries.

3. **Self-contained** — each unit carries its own context: deps, types, rules. The spec can travel with the program. Minimal external knowledge required.

4. **Language-agnostic** — no dependency on English or any natural language. An agent trained on any corpus can use ilo.

5. **Graph-native** — programs express relationships (calls, depends-on, has-type). Navigable as a graph, not just readable as linear text.

See [MANIFESTO.md](MANIFESTO.md) for the full rationale.

## What An Agent Actually Does

```
1. Receive a goal
2. Break it into steps
3. Generate code for a step
4. Execute it
5. Read the result
6. Decide what to do next
7. Repeat
```

Every design decision is evaluated against this loop. Does it reduce the total tokens spent across all iterations?

## Status

Design phase. Defining the language principles and exploring syntax through examples before writing the implementation.

## Structure

```
ilo-lang/
├── MANIFESTO.md     # Design rationale and principles
├── README.md        # This file
├── examples/        # Syntax exploration programs
├── src/             # Rust implementation (coming)
└── docs/            # Specification (emerging from examples)
```
