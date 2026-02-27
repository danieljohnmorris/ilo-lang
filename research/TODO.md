# TODO

## Basics — complete what's already there

### Parser gaps (AST/VM support exists, no parser production)

- [ ] List literals `[a, b, c]` — `Expr::List` and `OP_LISTNEW` exist, parser has no `[` production
- [ ] Unary negation `-x` — `UnaryOp::Negate` in AST, but `-` always parsed as binary subtract
- [ ] Logical NOT `!x` — `UnaryOp::Not` in AST, `OP_NOT` in VM, no parser production
  - Note: `!x` currently means Err-wrap (`Expr::Err`) — need new sigil for Err-wrap before this lands

### Missing fundamental operators

- [ ] Logical AND `a & b` — compile to short-circuit jump sequence (JMPF), no new opcode needed
- [ ] Logical OR `a | b` — compile to short-circuit jump sequence (JMPT), no new opcode needed
- [ ] String comparison `<` `>` `<=` `>=` — extend existing comparison ops to handle text

### Builtins (new opcodes — keep dispatch O(1), JIT-eligible where numeric)

- [ ] `len x` — length of string (bytes) or list
- [ ] `str n` — number to text
- [ ] `num t` — text to number (returns `R n t`, Err if unparseable)
- [ ] `abs n` — absolute value
- [ ] `min a b` — minimum of two numbers
- [ ] `max a b` — maximum of two numbers
- [ ] `flr n` — floor
- [ ] `cel n` — ceil

## Python codegen

- [ ] Fix lossy match arm codegen — let bindings in match arms are silently dropped when emitted as ternaries
