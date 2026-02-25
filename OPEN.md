# Open Questions

Unresolved design questions and lessons from syntax exploration. For design rationale, see [MANIFESTO.md](MANIFESTO.md). For syntax variants, see [examples/](examples/).

## Lessons From Syntax Experiments

### What saves tokens

Positional arguments are the single biggest token saver. `reserve(items:items)` → `reserve items` eliminates parens, colons, and repeated names. Most call sites become `verb arg arg`.

Implicit last-result matching saves both tokens and variable names. `x=call(...);match x{err e:...}` → `call arg;?{!e:...}` — no intermediate binding needed.

Single-char operators (`?`/`!`/`~`/`@`/`>`) replace keywords (`match`/`err`/`ok`/`for`/`->`) but save fewer tokens than expected — the tokenizer already encodes common English words as single tokens. The savings are mainly in characters.

### What doesn't save tokens

Short variable names (`ord` instead of `order`, `dc` instead of `discount`) save characters but not tokens. Common English words are already single tokens in cl100k_base. Unusual abbreviations sometimes split into multiple tokens, costing more. This is why idea8 and idea9 have nearly identical token counts (285 vs 287) despite idea9 being 114 chars shorter.

### Key tradeoff: tokens vs characters

Tokens and characters optimise differently. idea4-ast-bytecode is 0.67x tokens but 0.33x chars. idea8-ultra-dense is 0.33x tokens and 0.25x chars. The best formats score well on both, but the techniques that help each metric are different.

### Spec quality matters for generation

LLM generation accuracy depends heavily on spec clarity. Adding operator examples (showing `<`, `>`, `/` usage) and explicit comparison operator docs raised scores from 8/10 to 10/10. The spec is part of the prompt — it needs to be unambiguous.

## Execution Model

**Option A: Graph engine (verify → execute)**
The program is a graph of nodes (functions, types, tools). The runtime validates new nodes and executes by traversing edges. No compile step — each node is verified and live immediately.

**Option B: Tool orchestration engine**
The runtime is a workflow engine. ilo programs are DAGs of tool calls. The runtime executes the DAG, calling real external services.

**Option C: Transpilation**
ilo verifies the program then compiles to Python/JS/WASM for execution. Verification in ilo, execution in a mature runtime.

## Graph Loading Problem

"Agent gets the world upfront" has a cost: the world must be loaded into context. 500 tools and 200 types = thousands of tokens of spec before the agent writes a line.

**Option 1: Full graph** — load everything. Only works for small projects.

**Option 2: Subgraph by task** — something decides which slice is relevant. Question: who decides?

**Option 3: Query on demand** — agent starts with nothing, queries the runtime for what it needs. Total context cost: 2 tool signatures instead of 500.

**Option 4: Progressive disclosure** — load tool names first (cheap), load full signatures on demand.

## ilo as a Typed Shell

Not just a language — a **typed shell** for agents. Like bash discovers executables on `$PATH`, ilo discovers typed tools from configured sources and lets agents compose them with verified types and error handling.

The runtime's job: discover → present → verify → execute.

## Syntax Questions (Resolved by Experiments)

These were open questions that the syntax experiments have now answered:

- **`let` keyword** — dropped entirely in idea7+. `x=expr` is unambiguous. Saves ~15 tokens per program.
- **`concat` operator** — `+` doubles as string concat in idea8+. One fewer keyword.
- **`for` syntax** — `@` in idea8+. Always produces a list. Statement-form iteration wasn't needed.
- **Named vs positional args** — positional wins for token efficiency. Named args at call sites were the biggest token cost in idea1.

## Still Open

### Which syntax to build?

idea8-ultra-dense has the best token efficiency at 10/10 accuracy. But is it too dense for debugging? Error messages pointing at `?{!e:!+"Failed: "e;~d:...}` may be hard to read. The runtime/tooling could help — pretty-printing for human review while keeping the dense wire format for LLM I/O.

### Hybrid approach?

Could the runtime accept multiple syntax levels — dense wire format for LLM generation, expanded form for human review — with lossless conversion between them? Same AST, different serialisations.

### Match exhaustiveness

Should the verifier require all patterns to be covered? The experiments don't test this since there's no verifier yet.

### Compensation patterns

The workflow examples show inline compensation (`charge pid amt;?{!e:release rid;!+"Payment failed"...}`). Should compensation be a first-class concept, or is inline error handling sufficient?
