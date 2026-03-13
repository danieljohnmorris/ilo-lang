# ilo

*A programming language AI agents write, not humans. Named from [Toki Pona](https://sona.pona.la/wiki/ilo) for "tool".*

[![CI](https://github.com/ilo-lang/ilo/actions/workflows/rust.yml/badge.svg)](https://github.com/ilo-lang/ilo/actions/workflows/rust.yml)  [![codecov](https://codecov.io/gh/ilo-lang/ilo/branch/main/graph/badge.svg)](https://codecov.io/gh/ilo-lang/ilo)  [![crates.io](https://img.shields.io/crates/v/ilo)](https://crates.io/crates/ilo)  [![npm](https://img.shields.io/npm/v/ilo-lang)](https://www.npmjs.com/package/ilo-lang)  [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

```
Python                                    ilo
─────                                     ───
def total(price, quantity, rate):          tot p:n q:n r:n>n;s=*p q;t=*s r;+s t
    sub = price * quantity
    tax = sub * rate
    return sub + tax

4 lines, 30 tokens, 90 chars              1 line, 10 tokens, 20 chars
```

**0.33× the tokens. 0.22× the characters. Same semantics. Type-verified before execution.**

## Why

AI agents pay three costs per program: generation tokens, error feedback, retries. ilo cuts all three:

- **Shorter programs** - prefix notation eliminates parentheses; positional args eliminate boilerplate
- **Verified first** - type errors caught before execution; agents get `ILO-T004` not a stack trace
- **Compact errors** - one token, not a paragraph; agents correct faster, fewer retries

## Install

<details open>
<summary>macOS / Linux</summary>

```bash
curl -fsSL https://raw.githubusercontent.com/ilo-lang/ilo/main/install.sh | sh
```

</details>

<details>
<summary>Windows (PowerShell)</summary>

```powershell
Invoke-WebRequest -Uri https://github.com/ilo-lang/ilo/releases/latest/download/ilo-x86_64-pc-windows-msvc.exe -OutFile ilo.exe
```

</details>

<details>
<summary>npm (any platform with Node 20+)</summary>

```bash
npm i -g ilo-lang

# or run without installing
npx ilo-lang 'dbl x:n>n;*x 2' 5
```

> WASM mode: interpreter only. HTTP builtins (`get`, `$`, `post`) require the native binary.

</details>

<details>
<summary>Rust</summary>

```bash
cargo install ilo
```

</details>

<details>
<summary>Agent-specific install</summary>

| Agent | Install |
|-------|---------|
| **Claude Code** | `/plugin marketplace add ilo-lang/ilo` then `/plugin install ilo-lang/ilo` |
| **Claude Cowork** | Browse Plugins → Add marketplace → `ilo-lang/ilo` → install |
| **Other agents** | Copy `skills/ilo/` into your agent's skills directory |

</details>

**[All install methods →](https://ilo-lang.ai/docs/installation/)**

## Quick start

```bash
# Inline
ilo 'dbl x:n>n;*x 2' 5                    # → 10

# From file
ilo program.ilo functionName arg1 arg2
```

**[Tutorial: Write your first program →](https://ilo-lang.ai/docs/first-program/)**

## What it looks like

**Guards** - flat, no nesting:
```
cls sp:n>t;>=sp 1000 "gold";>=sp 500 "silver";"bronze"
```

**Pipes** - left-to-right composition:
```
run x:n>n;x>>dbl>>inc
```

**Data pipeline** - fetch, parse, filter, sum:
```
fetch url:t>R ? t;r=($!url);rdb! r "json"
proc rows:L ?>n;clean=flt pos rows;sum clean
pos x:?>b;>x 0
```

**Auto-unwrap `!`** - eliminates Result matching:
```bash
ilo 'inner x:n>R n t;~x  outer x:n>R n t;~(inner! x)' 42  # → 42
```

## Teaching agents

ilo ships as an [Agent Skill](https://agentskills.io). Install the plugin and the agent learns ilo automatically.

For manual context loading:
```bash
ilo -ai              # compact spec for LLM system prompts
ilo help lang        # full spec
```

## Key docs

| | |
|---|---|
| **[Introduction](https://ilo-lang.ai/docs/introduction/)** | What ilo is and why |
| **[Installation](https://ilo-lang.ai/docs/installation/)** | All install methods |
| **[Tutorial](https://ilo-lang.ai/docs/first-program/)** | Write your first program |
| **[Types & Functions](https://ilo-lang.ai/docs/guide/types-and-functions/)** | Core language guide |
| **[Prefix Notation](https://ilo-lang.ai/docs/guide/prefix-notation/)** | Why prefix saves tokens |
| **[Guards](https://ilo-lang.ai/docs/guide/guards/)** | Pattern matching without if/else |
| **[Pipes](https://ilo-lang.ai/docs/guide/pipes/)** | Function composition |
| **[Collections](https://ilo-lang.ai/docs/guide/collections/)** | Lists and higher-order functions |
| **[Error Handling](https://ilo-lang.ai/docs/guide/error-handling/)** | Result types and auto-unwrap |
| **[Data & I/O](https://ilo-lang.ai/docs/guide/data-io/)** | HTTP, files, JSON, env |
| **[MCP Integration](https://ilo-lang.ai/docs/integrations/mcp/)** | Connect MCP servers |
| **[CLI Reference](https://ilo-lang.ai/docs/reference/cli/)** | Flags, REPL, output modes |
| **[Builtins](https://ilo-lang.ai/docs/reference/builtins/)** | All built-in functions |
| **[Error Codes](https://ilo-lang.ai/docs/reference/error-codes/)** | ILO-XXXX reference |
| **[SPEC.md](SPEC.md)** | Full language specification |
| **[examples/](examples/)** | Runnable examples (also test suite) |

## Benchmarks

Per-call time (ns) across 8 micro-benchmarks. Lower is better. [Full results →](https://ilo-lang.ai/docs/reference/benchmarks/)

| Language | numeric | string | record | mixed | guards | recurse | foreach | while | pipe | file | api |
|----------|--------:|--------:|--------:|--------:|--------:|--------:|--------:|--------:|--------:|--------:|--------:|
| Rust (native) | 494ns | 294ns | 1ns | 10.2us | 1.6us | 217ns | 46ns | n/a | 382ns | 9.4us | 172.6us |
| Go | 707ns | 4.2us | 66ns | 6.3us | 688ns | 471ns | 458ns | 109ns | 116ns | 18.6us | 210.0us |
| C# (.NET) | 5.6us | 2.3us | 570ns | 30.0us | 7.0us | 300ns | 1.5us | 555ns | 857ns | 22.1us | 239.1us |
| Kotlin (JVM) | 494ns | 2.1us | 277ns | 7.7us | 1.0us | 184ns | 1.2us | 165ns | 226ns | 17.7us | n/a |
| LuaJIT | 425ns | 1.1us | 151ns | 9.7us | 2.9us | 711ns | 1.2us | 135ns | 205ns | 14.3us | 46.7us |
| Node/V8 | 543ns | 431ns | 380ns | 5.4us | 1.0us | 482ns | 564ns | 116ns | 242ns | 13.9us | 294.1us |
| TypeScript | 442ns | 373ns | 233ns | 5.4us | 1.0us | 381ns | 408ns | 75ns | 164ns | 12.3us | 298.7us |
| ilo AOT | 5.8us | 4.3us | 1.5us | 48.2us | 7.3us | 971ns | 13.2us | 1.1us | 721ns | 18.8us | 2.5ms |
| ilo JIT | 3.7us | 744ns | 633ns | 40.7us | 5.5us | 503ns | 10.7us | 406ns | 505ns | 17.5us | 1.8ms |
| ilo VM | 12.2us | 2.9us | 3.1us | 30.0us | 49.8us | 5.0us | 2.5us | 1.2us | 6.9us | 16.6us | 2.5ms |
| ilo Interpreter | 91.3us | 16.1us | 54.7us | 1.4ms | 940.5us | 133.9us | 76.5us | 10.4us | 149.3us | 31.4us | 2.6ms |
| Lua | 4.0us | 5.2us | 7.8us | 49.4us | 27.1us | 2.7us | 3.8us | 927ns | 4.2us | 15.2us | n/a |
| Ruby | 23.4us | 8.9us | 10.5us | n/a | 61.9us | 4.7us | 4.2us | 2.5us | 8.0us | n/a | n/a |
| PHP | 6.6us | 1.3us | 4.1us | 8.3us | 25.4us | 4.3us | 996ns | 691ns | 6.4us | 14.4us | 192.2us |
| Python 3 | 41.5us | 2.5us | 15.7us | 43.6us | 86.9us | 9.5us | 2.3us | 3.6us | 16.7us | 24.1us | 2.5us |
| PyPy 3 | 1.3us | 803ns | 440ns | 21.2us | 4.4us | 1.1us | 589ns | 276ns | 454ns | 22.4us | 731ns |

*10000 iterations, Darwin arm64, 2026-03-13*

## Community

- **[ilo-lang.ai](https://ilo-lang.ai)** - docs, playground, and examples
- **[r/ilolang](https://www.reddit.com/r/ilolang/)** - discussion and updates
- **[hello@ilo-lang.ai](mailto:hello@ilo-lang.ai)** - get in touch

## Principles

1. **Token-conservative** - every choice evaluated against total token cost
2. **Constrained** - small vocabulary, one way to do things, fewer wrong choices
3. **Verified** - types checked before execution, all errors reported at once
4. **Language-agnostic** - structural tokens (`@`, `>`, `?`, `^`, `~`, `!`, `$`) over English words

See the [manifesto](https://ilo-lang.ai/docs/manifesto/) for full rationale.
