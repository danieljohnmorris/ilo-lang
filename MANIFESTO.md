# The ilo Manifesto

## The Audience Is Not Human

Every programming language in use today was designed for people. The syntax, the error messages, the tooling — all optimised for a brain that reads left-to-right, tracks visual indentation, and cares about aesthetics.

AI agents are not that brain. They produce tokens sequentially. They consume tokens from a finite context window. Every token they spend — generating, reading, retrying — costs real time and real money.

ilo is designed for them.

## The Only Metric

**Total tokens from intent to working code.**

```
Total cost = generation + context loading + error feedback + retries
```

Every design decision is evaluated against this number. If a feature reduces it, it's in. If it increases it, it's out. No exceptions for elegance, readability, or convention.

## Strategy 1: Concise

Minimum tokens to express correct intent.

This does not mean "short syntax." It means the cheapest path through the entire loop — generation, execution, feedback, and any necessary retries combined.

A named argument like `amount: 42` costs more tokens than a positional `42`. But if positional args cause the agent to swap two parameters once in every ten calls, and each retry costs 200 tokens, then named args are cheaper on average. The math decides, not taste.

The language should be as terse as possible **without increasing retry rate**. Where there's a tradeoff between generation cost and error rate, we optimise for total cost across the loop.

## Strategy 2: Constrained

A small possibility space means fewer wrong choices.

When an agent generates the next token, how many valid options are there? The fewer, the better. Not because choice is bad, but because every invalid option that an agent might select costs a full retry cycle — often hundreds of tokens.

This means:
- **Closed world.** Every callable function is known ahead of time. The agent cannot hallucinate an API that doesn't exist. If it tries, the error names the closest valid alternative.
- **Small vocabulary.** Fewer keywords, fewer constructs, fewer ways to express the same thing. One way to define a function. One way to call it. One way to handle errors.
- **Verification before execution.** Check that all calls resolve, all types align, all dependencies exist — before running anything. Catch mistakes at the cheapest possible moment.

Constrained generation can go further: the runtime feeds valid next-token sets back to the agent, making it *impossible* to generate invalid code. The language becomes a set of rails, not an open field.

## Strategy 3: Self-Contained

Each unit of work needs minimal context.

An agent working on function A shouldn't need to load functions B through Z to understand what A does. The less context required per step, the fewer tokens consumed, the more of the context window is available for the actual task.

This means:
- **Explicit dependencies.** Each function declares exactly what it needs from outside — by name, with types. No globals, no ambient state, no implicit imports.
- **Small units.** A function that fits in a few dozen tokens can be loaded, understood, and modified cheaply. A function that consumes half the context window is a design failure.
- **Clear boundaries.** Inputs, outputs, and side effects are declared. An agent knows exactly what goes in and what comes out without reading the implementation of every dependency.

## The Name

*ilo* is Toki Pona for "tool" ([sona.pona.la/wiki/ilo](https://sona.pona.la/wiki/ilo)).

Toki Pona is a constructed language — a conlang — built around radical minimalism. ~120 words. 14 phonemes. Complex ideas expressed by combining simple terms. It constrains human expression to force clarity of thought.

ilo does the same for machine programmers. A minimal, verified vocabulary. Complex programs built by composing small, self-contained units. The constraint is the feature.

## What ilo Is Not

**Not a framework for building AI agents.** There are plenty of those. ilo is a language for agents to write programs *in*.

**Not optimised for human readability.** Humans can read it — it's not obfuscated — but no decision is made because it "looks cleaner" or "reads more naturally." If a design is uglier but costs fewer total tokens, it wins.

**Not theoretical.** Every principle here addresses measured failure modes in AI-generated code: hallucinated APIs, positional argument swaps, context window exhaustion, wasted retry cycles from vague errors.

## What ilo Is

A **minimal, verified action space** — the smallest set of constructs an agent needs to express computational intent, with everything else stripped away.
