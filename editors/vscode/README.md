# ilo Language Support for VS Code

Syntax highlighting and LSP client integration for the [ilo programming language](https://ilo-lang.ai).

## Features

- **Syntax highlighting** via TextMate grammar covering:
  - Comments (`--`)
  - String literals with escape sequences
  - Number literals (integers, floats, negatives)
  - Boolean and nil constants
  - Type declarations (`type Name{...}`)
  - Tool declarations (`tool name"desc" params>ret`)
  - Function definitions
  - Control flow keywords (`wh`, `brk`, `cnt`, `ret`, `@`)
  - Result operators (`~` ok, `^` err)
  - All operators including `>=`, `<=`, `!=`, `+=`, `>>`, `??`
  - Built-in functions (`len`, `str`, `map`, `flt`, etc.)
  - Type constructors (`L`, `R`, `F`, `O`, `M`, `S`) and primitives (`n`, `t`, `b`)
  - Reserved words highlighted as invalid

- **LSP integration** — connects to `ilo lsp` for diagnostics, hover, and completions (requires ilo >= 0.10.0 with LSP support)

## Requirements

- `ilo` installed and on `PATH` (for LSP features)
- Install ilo: `cargo install ilo` or `npx ilo-lang`

## File Extension

`.ilo` files are automatically recognised.

## Building

```bash
npm install
npm run compile
```

## Packaging

```bash
npm install -g @vscode/vsce
vsce package
```
