# ilo

*ilo* — Toki Pona for "tool." A constructed language for AI agents to program in.

Programming languages were designed for humans — visual parsing, readable syntax, spatial navigation. AI agents are not humans. They generate tokens. Every token costs latency, money, and context window. The only metric that matters is **total tokens from intent to working code**.

ilo is designed from first principles around that metric.

## The Single Principle

**Minimise total token cost across the full execution loop.**

```
Total cost = generation + context loading + error feedback + retries
```

Not just concise syntax. Not just short keywords. Concise across the *entire* loop — writing, executing, understanding results, fixing mistakes. A verbose keyword that prevents one retry is cheaper than a terse keyword that causes one.

Three strategies achieve this:

1. **Concise** — minimum tokens to express correct intent
2. **Constrained** — small possibility space makes it hard to generate invalid programs, eliminating retry cycles
3. **Self-contained** — each unit declares what it needs, minimising context that must be loaded

Strategies 2 and 3 serve strategy 1. Constraints reduce retries (fewer tokens). Self-containment reduces context loading (fewer tokens).

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

Design phase. Defining the language specification before writing the implementation.

## Structure

```
ilo-lang/
├── MANIFESTO.md     # Design rationale
├── README.md        # This file
├── src/             # Rust implementation (coming)
└── examples/        # Example programs (coming)
```
