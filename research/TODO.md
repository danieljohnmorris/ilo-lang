# TODO

Everything through Phase E is complete. This file tracks remaining work.

## Open items

### Agent / tool integration

- [ ] Tool graph — which tools depend on which types, what produces what
- [ ] "Typed shell" mode — interactive tool composition with type-guided completion

### Tooling

- [ ] LSP / language server — completions, diagnostics, hover for editor integration
- [ ] REPL — interactive evaluation for exploration and debugging
- [ ] Playground — web-based editor with live evaluation (WASM target)

### Codegen targets

- [ ] JavaScript / TypeScript emit — like Python codegen but for JS ecosystem
- [ ] WASM emit — compile to WebAssembly for browser/edge execution

### Program structure

- [ ] Multi-file programs / module system (programs are small by design — may never need this)
- [ ] Imports — `use "other.ilo"` to compose programs from multiple files

---

## Completed (summary)

| Phase | Feature |
|-------|---------|
| Basics | List literals, unary ops, logical AND/OR, string comparison, all builtins |
| Verification | Type verifier, match exhaustiveness, arity checks |
| B: Errors | Spans, Diagnostic model, ANSI/JSON renderers, error codes |
| C: Polish | Error recovery, suggestions/fix-its, runtime source mapping, stack traces |
| D1: Tools | HTTP `get`/`$`, auto-unwrap `!`, ToolProvider, HttpProvider, StubProvider, Value↔JSON |
| D2: MCP | MCP stdio client, auto-discover tools, inject into AST |
| D3: Discovery | `ilo tools`, progressive disclosure, `--human`/`--ilo`/`--json` output |
| D4: Agent loop | `ilo serv` / `ilo repl -j`, JSON protocol, phase-structured errors |
| E: Types | `O T` optional, `S a b c` sum, `M k v` map + 7 builtins, type variables |
| Hardening | Reserved keywords: `if` `return` `let` `fn` `def` `var` `const` |
| Control flow | Type pattern matching `?x{n v:...; t v:...}` |
| Codegen | Python emit, formatter (`--explain`), dense wire format |
