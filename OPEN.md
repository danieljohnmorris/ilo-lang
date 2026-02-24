# Open Questions

Unresolved design questions. Nothing here is decided — these are options being explored.

## Execution Model

The Rust implementation needs to be one of these.

**Option A: Graph engine (verify → execute)**
The program is a graph of nodes (functions, types, tools). The runtime holds the graph, validates new nodes as they're added, and executes by traversing edges. No compile step — each node is verified and live immediately.

- Aligns with graph-native principle
- Incremental: add one node, verify it, it's available
- Cost of a mistake is one node, not the whole program

**Option B: Tool orchestration engine**
The runtime is a workflow engine. ilo programs are DAGs of tool calls — fetch data, transform, call APIs, handle errors. The runtime executes the DAG, calling real external services. General computation (loops, math) is supported but the primary purpose is chaining tool calls.

**Option C: Transpilation**
ilo verifies the program (types, deps, closed world) then compiles to Python/JS/WASM for execution. Verification happens in ilo, execution happens in a mature runtime. Leverages existing ecosystems but error messages may leak from the target language.

## The Agent's Workflow

How does an agent actually use ilo vs what it does today?

Today (Python):
```
1. Agent gets task
2. Generates Python, guessing at APIs
3. Runtime error → retry (one error at a time, ~200 tokens each)
4. Eventually works (maybe)
```

With ilo:
```
1. Agent gets task + the ilo graph (all available tools, types, functions)
2. Agent writes ilo against a known world
3. Verifier catches ALL errors before execution (~50 tokens total)
4. Executes. Works first time.
```

The key difference: the agent receives the world as a loadable graph. It's not guessing. And the verifier catches everything before execution, all at once.

## Use Cases (Not Yet Prioritised)

1. **Tool orchestration** — chain API calls with verified types and explicit error handling. The most immediate use case.
2. **Sandboxed execution** — the closed world is a security boundary. The agent can only use what's declared.
3. **Multi-agent composition** — Agent A writes a function, Agent B extends it. The graph makes dependencies explicit.
4. **Persistent programs** — the graph grows incrementally across sessions. Each node is self-contained.

## Syntax Questions

### `let` keyword
`let` costs 1 token per binding (~15-20 per program). Python manages without it — `x = expr` is unambiguous in statement position. Does the disambiguation earn its keep, or should bindings be `x = expr`?

### `concat` operator
Only remaining word-operator. Should string concatenation use a symbol? `++`? `~`?

### Match exhaustiveness
Should the verifier require all patterns to be covered?

### `unwrap` safety
Should `unwrap` be allowed, or must every result be matched explicitly?

### `for` as expression
Does `for` always return a list? Or can it be a statement (side-effects only)?
