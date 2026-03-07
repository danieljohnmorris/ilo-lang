# TODO

## Tooling

- [ ] LSP / language server — completions, diagnostics, hover for editor integration
- [ ] REPL — interactive evaluation for exploration and debugging
- [ ] Playground — web-based editor with live evaluation (WASM target)

## Codegen targets

- [ ] JavaScript / TypeScript emit — like Python codegen but for JS ecosystem
- [ ] WASM emit — compile to WebAssembly for browser/edge execution

## Program structure

- [ ] Namespacing — prevent name collisions when merging many declaration graphs (low priority)

## Language hardening (from hands-on exploration)

Discovered during a Claude Code session using ilo as a bash/python replacement. See conversation for full context.

### Bugs
- [x] **Bool-variable guard elision** — And/Or short-circuit was clobbering left operand register. Fixed by allocating a fresh result register. (#92)

### Syntax (accept familiar forms, hint toward canonical)
- [x] **`==` as sugar for `=`** — `==` lexes as `Eq` token, same as `=`. `==a b` works as equality. Removed from cross-language warnings. Note: `e==c n` parses as assignment `e = c(n)` — use `e= ==c n` for assign+equality.
- [ ] **Full infix support** — support `a + b` alongside `+a b` everywhere. Desugars to prefix AST. Prefix stays canonical, formatter emits prefix in `--dense`. Hints suggest prefix form with token savings. Lets LLMs write familiar infix on first try, learn prefix over time via hints. Parsing: use Pratt parsing / precedence climbing. Function application (space) binds tighter than operators (`f a + b` = `(f a) + b`). Parens override precedence as usual. Design challenge: disambiguating prefix vs infix when operator follows an atom — solve during implementation.

### Diagnostics
- [x] **`//` warning inside string literals** — cross-language warning now strips string contents before pattern matching. URLs in strings no longer trigger false positives.
- [x] **Multi-function boundary diagnostic** — already fixed by `is_fn_decl_start()` in `can_start_operand()` (commit 2b9ff66). Parser detects `Ident >` (zero-param) and `Ident Ident :` (parameterized) boundaries, preventing greedy arg consumption. All valid multi-function programs parse correctly; no additional diagnostic needed.
- [x] **Guard-in-loop lint** — verifier emits ILO-W001 when a guard without else appears inside `@`/`wh`/range loops. Suggests ternary `{then}{else}` or `brk`/`cnt` for loop control.

### DX
- [x] **Idiomatic hints on successful runs** — scans source for non-canonical forms (e.g. `==` → `=`) and emits hints to stderr after execution. TTY → plain text, JSON mode → `{"hints":[...]}`. Disable with `--no-hints` / `-nh`.

### Nice-to-have
- [x] **Modulo builtin** — `mod a b` returns remainder. Implemented across verifier, interpreter, and VM with division-by-zero check.

### Testing
- [ ] **Parser coverage 85% → 90%+** — lowest coverage module. The multi-function boundary and `==` lexing issues suggest more edge case tests are needed.

---

## Completed

### Performance
- [x] Interpreter flat-scope rewrite — `Vec<(String, Value)>` + `scope_marks: Vec<usize>` replaces `Vec<HashMap>`

### Agent / tool integration
- [x] Tool graph — `ilo tools --graph`: type-level composition map showing which tools can feed each other
- [x] D1: ToolProvider, HttpProvider, StubProvider, Value↔JSON
- [x] D2: MCP stdio client, auto-discover tools, inject into AST
- [x] D3: `ilo tools` — list/discover with `--human`/`--ilo`/`--json` output
- [x] D4: `ilo serv` — JSON stdio agent loop with phase-structured errors

### Program structure
- [x] Imports — `use "other.ilo"` (all) and `use "other.ilo" [name1 name2]` (scoped)

### Language hardening
- [x] Reserve keywords at lexer level — `if`, `return`, `let`, `fn`, `def`, `var`, `const`

### Type system
- [x] Optional type — `O T` nullable values
- [x] Sum types — `S a b c` closed sets of variants
- [x] Map type — `M k v` key-value collections + 7 builtins (mmap, mget, mset, mhas, mkeys, mvals, mdel)
- [x] Type variables — single-letter type params for generic functions

### Control structures
- [x] Pattern matching on type — `?x{n v:...; t v:...}`
- [x] While loop `wh cond{body}`
- [x] Break/continue `brk`/`cnt`
- [x] Range iteration `@i 0..n{body}`
- [x] Early return `ret expr`
- [x] Pipe operator `>>` for chaining calls
- [x] Nil-coalesce `??`, safe field navigation `.?`
- [x] Destructuring bind `{a;b}=expr`

### VM / performance
- [x] Bump arena for records — arena-allocated structs, promote to heap on escape
- [x] JIT inlining — arithmetic, comparisons, branching, field access, alloc
- [x] No-Vec OP_CALL — push args directly onto stack, 1.6x faster function calls

### Builtins
- [x] `env` — read environment variables (`env "PATH"` → `R t t`)
- [x] `get`/`$` — HTTP GET returning `R t t`
- [x] `rd`, `rdl`, `wr`, `wrl` — file I/O (read/write, string and lines variants)
- [x] `rd path fmt` — format override (`"csv"`, `"tsv"`, `"json"`, `"raw"`); auto-detects from extension when 1-arg
- [x] `rdb s fmt` — parse string/buffer in given format (for HTTP responses, env vars, etc.)
- [x] String escape sequences — `\n`, `\t`, `\r`, `\"`, `\\` in string literals
- [x] `prnt` — print + passthrough (like Rust `dbg!`)
- [x] `len`, `str`, `num`, `abs`, `min`, `max`, `flr`, `cel`, `rnd`, `now`
- [x] `cat`, `has`, `hd`, `tl`, `rev`, `srt`, `srt fn xs`, `slc`, `spl`
- [x] `map`, `flt`, `fld` — higher-order functions
- [x] `jpth`, `jdmp`, `jpar` — JSON path/dump/parse
- [x] `trm s` — trim whitespace from string ends
- [x] `unq xs` — deduplicate list or text chars, preserve order
- [x] `fmt "template {}" args…` — `{}` positional interpolation
- [x] `grp fn xs` — group by key function, returns map of key → list
- [x] `flat xs` — flatten nested lists one level
- [x] `sum xs` / `avg xs` — basic numeric aggregation
- [x] `rgx pat s` — regex match/extract
- [x] Structured CSV/TSV/JSON output via `wr path data "csv"`

### Error infrastructure
- [x] Spans, Diagnostic model, ANSI/JSON renderers, error codes (ILO-L/P/T/R)
- [x] Error recovery — multiple errors per file, poison nodes
- [x] Error codes + `--explain ILO-T001`
- [x] Suggestions/fix-its — did-you-mean, type coercion hints, cross-language syntax detection
- [x] Runtime source mapping — spans and call stacks on runtime errors

### Basics
- [x] List literals, unary ops, logical AND/OR/NOT, string comparison
- [x] All comparison operators extend to text (lexicographic)
- [x] Type verifier, match exhaustiveness, arity checks at all call sites
- [x] Python codegen, `--explain` formatter
- [x] Type aliases `alias name type`
