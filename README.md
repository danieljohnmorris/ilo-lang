# ilo

*ilo* — Toki Pona for "tool" ([sona.pona.la/wiki/ilo](https://sona.pona.la/wiki/ilo)). A constructed language for AI agents.

Languages were designed for humans — visual parsing, readable syntax, spatial navigation. AI agents are not humans. They generate tokens. Every token costs latency, money, and context window. The only metric that matters is **total tokens from intent to working code**.

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

## What It Looks Like

```
fn total
	price: number, quantity: number, rate: number -> number
	? price: 10, quantity: 2, rate: 0.2 == 24
	let sub = * price quantity
	let tax = * sub rate
	+ sub tax
```

Same function in Python (for comparison):

```python
def total(price: float, quantity: int, rate: float) -> float:
    sub = price * quantity
    tax = sub * rate
    return sub + tax
```

ilo adds named args at call sites, inline tests (`?`), and explicit dependency declarations (`@`) — features that cost a few extra tokens to generate but prevent entire categories of retries.

## Five Principles

1. **Token-conservative** — the north star. Every choice evaluated against total token cost across the full loop: generation, retries, error feedback, context loading. Not just "short syntax."

2. **Constrained** — small vocabulary, closed world, one way to do things. Fewer valid next-tokens = fewer wrong choices = fewer retries.

3. **Self-contained** — each unit carries its own context: deps, types, rules. The spec can travel with the program. Minimal external knowledge required.

4. **Language-agnostic** — minimise dependency on English or any natural language. Structural tokens (`@`, `->`, `?`, `*`) over English words where possible.

5. **Graph-native** — programs express relationships (calls, depends-on, has-type). Navigable as a graph, not just readable as linear text.

## Documentation

| Document | Audience | Purpose |
|----------|----------|---------|
| [SPEC.md](SPEC.md) | Agents | Compact spec, loadable into context window |
| [docs/reference.md](docs/reference.md) | Humans | Full reference with explanations and examples |
| [MANIFESTO.md](MANIFESTO.md) | Both | Design rationale and principles |
| [examples/](examples/) | Both | Working programs demonstrating the language |

## Status

Design phase. Defining the language through principles, examples, and specification before writing the implementation.

## Structure

```
ilo-lang/
├── MANIFESTO.md          # Design rationale and principles
├── SPEC.md               # Agent-facing language spec
├── README.md             # This file
├── docs/reference.md     # Human-facing language reference
├── examples/             # Syntax exploration programs
└── src/                  # Rust implementation (coming)
```
