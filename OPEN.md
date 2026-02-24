# Open Questions

Unresolved design questions. Nothing here is decided — these are options being explored. For the current language spec, see [SPEC.md](SPEC.md). For design rationale, see [MANIFESTO.md](MANIFESTO.md).

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

## Graph Loading Problem

"Agent gets the world upfront" has a cost: the world must be loaded into context. 500 tools and 200 types = thousands of tokens of spec before the agent writes a line. The context window cost could dwarf the savings from fewer retries.

### How does the agent get just enough of the graph?

**Option 1: Full graph** — load everything. Only works for small projects. Simple but doesn't scale.

**Option 2: Subgraph by task** — something decides which slice of the graph is relevant to the task and loads only that. Question: who decides? The orchestrator? A routing agent? Keyword matching?

**Option 3: Query on demand** — agent starts with nothing, asks the runtime questions:
```
agent: "what tools can send email?"
runtime: send-email(to: text, subject: text, body: text -> result nil, text)
agent: "what types exist for users?"
runtime: profile(id: text, name: text, email: text, verified: bool)
```
Agent builds the `@` dependency block from query results, not from pre-loaded context.

**Option 4: Progressive disclosure** — load tool/function names first (cheap — just a list of single-token names), load full signatures on demand when the agent decides to use one.

Option 3 is the most interesting. The workflow becomes:
```
1. Agent gets task: "notify user X about their order"
2. Agent queries: "what tools handle users?" → get-user
3. Agent queries: "what tools send messages?" → send-email
4. Agent now has exactly the two tool signatures it needs
5. Writes ilo, verify, execute
```

Total context cost: 2 tool signatures instead of 500.

## Interop With The Real World

ilo programs don't run in isolation. Real systems have Python services, REST APIs, databases, operating systems. The `tool` declaration is the bridge — but who writes them?

### Tool discovery

**Manual** — human writes tool declarations. Accurate but doesn't scale. Defeats the purpose if the goal is autonomous agents.

**Auto-generated from specs** — OpenAPI/Swagger → tool declarations. Database schema → type declarations. CLI `--help` → tool declarations. The runtime reads existing specs and generates ilo-compatible declarations automatically.

**Runtime introspection** — the runtime connects to configured sources (API endpoints, databases, local services) and discovers what's available. Like `$PATH` for bash — the runtime knows what tools exist because it can see them.

### ilo as a typed shell

This reframes what ilo is: not just a language, but a **typed shell** for agents. The way bash discovers executables on `$PATH` and lets you pipe them together, ilo discovers typed tools from configured sources and lets agents compose them with verified types and error handling.

The runtime's job:
1. **Discover** — connect to sources, introspect available tools, generate typed declarations
2. **Present** — respond to agent queries about available tools
3. **Verify** — check the agent's program against the known world
4. **Execute** — run the verified program, calling real external services

The agent never guesses at APIs. It browses a typed catalogue.

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
