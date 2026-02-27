# TODO

## Sigil changes (do first — unblocks other work)

- [ ] Decide Err-wrap sigil to replace `!` (candidates: `\x`, `^x`)
- [ ] Reassign `!x` → logical NOT (`UnaryOp::Not`, `OP_NOT` already in AST/VM)
- [ ] Update SPEC.md, example `.ilo` files, README with new sigils

## Basics — complete what's already there

### Parser gaps (AST/VM support exists, no parser production)

- [x] List literals `[a, b, c]` — parser production added, connects to existing `Expr::List` and `OP_LISTNEW`
- [x] Unary negation `-x` — `UnaryOp::Negate` in AST, parser now disambiguates: `-x` = negate, `-x y` = subtract
- [ ] Logical NOT `!x` — blocked on sigil change above

### Missing fundamental operators

- [x] Logical AND `&a b` — short-circuit jump sequence (JMPF), no new opcode needed
- [x] Logical OR `|a b` — short-circuit jump sequence (JMPT), no new opcode needed
- [x] String comparison `<` `>` `<=` `>=` — lexicographic comparison on text values in VM + interpreter

### Builtins (new opcodes — keep dispatch O(1), JIT-eligible where numeric)

Note: all builtin names are single tokens (no hyphens — manifesto: "every hyphen doubles token cost").

- [ ] `len x` — length of string (bytes) or list
- [ ] `+=x v` — append single value to list, return new list
- [ ] `+a b` — extend to lists: concatenate two lists (already handles `n` add and `t` concat)
- [ ] Index access `x.0`, `x.1` — by integer literal (dot notation, consistent with field access)
- [ ] `str n` — number to text
- [ ] `num t` — text to number (returns `R n t`, Err if unparseable)
- [ ] `abs n` — absolute value
- [ ] `min a b` — minimum of two numbers
- [ ] `max a b` — maximum of two numbers
- [ ] `flr n` — floor
- [ ] `cel n` — ceil

## Verification

Manifesto principle: "Verification before execution. All calls resolve, all types align, all dependencies exist."

- [ ] Type verifier — check all call sites resolve to known functions with correct arity
- [ ] Match exhaustiveness — warn when match has no wildcard arm and not all cases covered (see OPEN.md)
- [ ] Arity check at call sites — currently only checked at runtime

## Tooling

- [ ] Pretty-printer / formatter — dense wire format for LLM I/O, expanded form for human review (see OPEN.md: "Hybrid approach")
- [ ] Useful error messages — current errors point at raw bytecode; source positions needed

## Python codegen

- [ ] Fix lossy match arm codegen — let bindings in match arms are silently dropped when emitted as ternaries
