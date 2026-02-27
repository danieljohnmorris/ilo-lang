# TODO

## Sigil changes (do first — unblocks other work)

- [x] Decide Err-wrap sigil to replace `!` → chose `^` (caret)
- [x] Reassign `!x` → logical NOT (`UnaryOp::Not`, `OP_NOT` already in AST/VM)
- [x] Update SPEC.md, example `.ilo` files, README with new sigils

## Basics — complete what's already there

### Parser gaps (AST/VM support exists, no parser production)

- [x] List literals `[a, b, c]` — parser production added, connects to existing `Expr::List` and `OP_LISTNEW`
- [x] Unary negation `-x` — `UnaryOp::Negate` in AST, parser now disambiguates: `-x` = negate, `-x y` = subtract
- [x] Logical NOT `!x` — parser production added, connects to existing `UnaryOp::Not` and `OP_NOT`

### Missing fundamental operators

- [x] Logical AND `&a b` — short-circuit jump sequence (JMPF), no new opcode needed
- [x] Logical OR `|a b` — short-circuit jump sequence (JMPT), no new opcode needed
- [x] String comparison `<` `>` `<=` `>=` — lexicographic comparison on text values in VM + interpreter

### Builtins (new opcodes — keep dispatch O(1), JIT-eligible where numeric)

Note: all builtin names are single tokens (no hyphens — manifesto: "every hyphen doubles token cost").

- [x] `len x` — length of string (bytes) or list
- [x] `+=x v` — append single value to list, return new list
- [x] `+a b` — extend to lists: concatenate two lists (already handles `n` add and `t` concat)
- [x] Index access `x.0`, `x.1` — by integer literal (dot notation, consistent with field access)
- [x] `str n` — number to text
- [x] `num t` — text to number (returns `R n t`, Err if unparseable)
- [x] `abs n` — absolute value
- [x] `min a b` — minimum of two numbers
- [x] `max a b` — maximum of two numbers
- [x] `flr n` — floor
- [x] `cel n` — ceil

## Verification

Manifesto principle: "Verification before execution. All calls resolve, all types align, all dependencies exist."

- [x] Type verifier — check all call sites resolve to known functions with correct arity
- [x] Match exhaustiveness — warn when match has no wildcard arm and not all cases covered (see OPEN.md)
- [x] Arity check at call sites — covered by type verifier (static check at all call sites)

## Tooling

- [ ] Pretty-printer / formatter — dense wire format for LLM I/O, expanded form for human review (see OPEN.md: "Hybrid approach")

## Error messages — Phase B (infrastructure + rendering)

Gives spans, structured diagnostics, and dual-mode output (human + machine).

### B1. Span infrastructure
- [ ] Add `Span { start: usize, end: usize }` type to AST module
- [ ] Lexer: attach `Span` to every token (already has byte `position`, extend to start/end)
- [ ] Parser: attach `Span` to every `Expr`, `Stmt`, `Decl`, `Pattern`, `MatchArm` node
- [ ] Source map helper: byte offset → line:col conversion (store original source or line start offsets)

### B2. Diagnostic data model
- [ ] `Diagnostic` struct: severity, code, message, primary span, secondary spans (with labels), suggestion (optional), notes
- [ ] `Severity` enum: Error, Warning, Hint
- [ ] `Suggestion` struct: message, replacement text, span, confidence (MachineApplicable / MaybeIncorrect)
- [ ] Collect diagnostics into a `Vec<Diagnostic>` instead of returning early on first error

### B3. Renderers
- [ ] Human renderer (ANSI): header line, `-->` location, gutter + source lines, labeled underlines (`^^^`), colored by severity
- [ ] JSON renderer: structured output matching the Diagnostic model, one JSON object per diagnostic
- [ ] Auto-detect: TTY → ANSI, piped → JSON. Override with `--json`/`-j`, `--text`/`-t`, `--ansi`/`-a` (mutually exclusive, error if multiple)
- [ ] Respect `NO_COLOR` env var
- [ ] Show full function source in errors (leverage ilo's density — whole function fits in one line)

### B4. Wire up existing errors
- [ ] Lexer errors → Diagnostic with span (already has byte position)
- [ ] Parser errors → Diagnostic with span (currently only token index)
- [ ] Verifier errors → Diagnostic with span (currently no position, just function name)
- [ ] Interpreter runtime errors → Diagnostic with span where possible
- [ ] VM runtime errors → Diagnostic (may need instruction-to-span mapping table from compiler)

## Error messages — Phase C (polish, do after grammar stabilises)

After language features settle. 

### C1. Error recovery ✓
- [x] Parser: continue after errors using panic-mode recovery (sync on `;`, `}`, `>`, next decl keyword)
- [x] Poison AST nodes: mark failed parses as error nodes, suppress cascading errors in verifier
- [x] Report multiple errors per file (cap at ~20 to avoid noise)
- [x] Verifier: analyse all functions even if earlier ones have errors

### C2. Error codes
- [ ] Assign stable codes: `ILO-L___` (lexer), `ILO-P___` (parser), `ILO-T___` (type/verifier), `ILO-R___` (runtime)
- [ ] Error code registry: catalogue of all codes with short description
- [ ] `--explain ILO-T001` flag: print expanded explanation with examples
- [ ] Include code in both human and JSON output

### C3. Suggestions and Fix-Its
- [ ] "Did you mean?" for undefined variables/functions — Damerau-Levenshtein, threshold `max(1, len/3)`, scope-aware
- [ ] Type mismatch suggestions — e.g. "use `num` to convert text to number"
- [ ] Missing pattern arm suggestions — list the uncovered cases
- [ ] Arity mismatch — show expected vs actual signature
- [ ] Cross-language syntax detection — detect `===`, `&&`, `||`, `function`, `def`, `fn` and suggest ilo equivalents

### C4. Runtime source mapping
- [ ] Compiler: emit instruction-to-span table alongside bytecode
- [ ] VM: on error, look up current instruction pointer in span table
- [ ] Interpreter: thread current Stmt/Expr span through evaluation for error context
- [ ] Stack trace with source locations for nested function calls

## Python codegen

- [x] Fix lossy match arm codegen — let bindings in match arms are silently dropped when emitted as ternaries
