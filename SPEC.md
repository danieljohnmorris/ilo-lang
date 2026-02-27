# ilo Language Spec

ilo is a token-minimal language for AI agents. Every design choice is evaluated against total token cost: generation + retries + context loading.

---

## Functions

```
<name> <param>:<type> ...><return-type>;<body>
```

- No parens around params — `>` separates params from return type
- `;` separates statements — no newlines required
- Last expression is the return value (no `return` keyword)
- Zero-arg call: `make-id()`

```
tot p:n q:n r:n>n;s=*p q;t=*s r;+s t
```

---

## Types

| Syntax | Meaning |
|--------|---------|
| `n` | number (f64) |
| `t` | text (string) |
| `b` | bool |
| `_` | nil |
| `L n` | list of number |
| `R n t` | result: ok=number, err=text |
| `order` | named type |

---

## Naming

Short names everywhere. 1–3 chars.

| Long | Short | Rule |
|------|-------|------|
| `order` | `ord` | truncate |
| `customers` | `cs` | consonants |
| `data` | `d` | single letter |
| `level` | `lv` | drop vowels |
| `discount` | `dc` | initials |
| `final` | `fin` | first 3 |
| `items` | `its` | first 3 |

Function names follow the same rules. Field names in constructors and external tool names keep their full form — they define the public interface.

---

## Operators

Prefix notation.

### Binary

| Op | Meaning | Types |
|----|---------|-------|
| `+a b` | add / concat / list concat | `n`, `t`, `L` |
| `+=a v` | append to list | `L` |
| `-a b` | subtract | `n` |
| `*a b` | multiply | `n` |
| `/a b` | divide | `n` |
| `=a b` | equal | any |
| `!=a b` | not equal | any |
| `>a b` | greater than | `n`, `t` |
| `<a b` | less than | `n`, `t` |
| `>=a b` | greater or equal | `n`, `t` |
| `<=a b` | less or equal | `n`, `t` |
| `&a b` | logical AND (short-circuit) | any (truthy) |
| `\|a b` | logical OR (short-circuit) | any (truthy) |

### Unary

| Op | Meaning | Types |
|----|---------|-------|
| `-x` | negate | `n` |
| `!x` | logical NOT | any (truthy) |

Nesting is unambiguous — no parentheses needed:

```
+*a b c     -- (a * b) + c
*a +b c     -- a * (b + c)
>=+x y 100  -- (x + y) >= 100
-*a b *c d  -- (a * b) - (c * d)
```

Each nested operator saves 2 tokens (no `(` `)` needed). Flat expressions like `+a b` save 1 char vs `a + b`. Across 25 expression patterns, prefix notation saves **22% tokens** and **42% characters** vs infix. See [research/explorations/prefix-vs-infix/](research/explorations/prefix-vs-infix/) for the full benchmark.

Disambiguation: `-` followed by one atom is unary negate, followed by two atoms is binary subtract.

### Operands

Operator operands are **atoms** (literals, refs, field access) or **nested prefix operators**. Function calls are NOT operands — bind call results to a variable first:

```
-- DON'T: *n fac p  →  parses as Multiply(n, fac) with p dangling
-- DO:    r=fac p;*n r
```

---

## Builtins

Called like functions, compiled to dedicated opcodes.

| Call | Meaning | Returns |
|------|---------|---------|
| `len x` | length of string (bytes) or list (elements) | `n` |
| `str n` | number to text (integers format without `.0`) | `t` |
| `num t` | text to number (Err if unparseable) | `R n t` |
| `abs n` | absolute value | `n` |
| `min a b` | minimum of two numbers | `n` |
| `max a b` | maximum of two numbers | `n` |
| `flr n` | floor (round toward negative infinity) | `n` |
| `cel n` | ceiling (round toward positive infinity) | `n` |

---

## Lists

```
xs=[1, 2, 3]
empty=[]
```

Comma-separated expressions in brackets. Trailing comma allowed. Use with `@` to iterate:

```
@x xs{+x 1}
```

Index by integer literal (dot notation):
```
xs.0     # first element
xs.2     # third element
```

**CLI list arguments:** Pass lists from the command line with commas (brackets also accepted):
```
ilo 'f xs:L n>n;len xs' 1,2,3       → 3
ilo 'f xs:L n>n;xs.0' 10,20,30      → 10
```

---

## Statements

| Form | Meaning |
|------|---------|
| `x=expr` | bind |
| `cond{body}` | guard: return body if cond true |
| `!cond{body}` | guard: return body if cond false |
| `?x{arms}` | match named value |
| `?{arms}` | match last result |
| `@v list{body}` | iterate list |
| `~expr` | return ok |
| `^expr` | return err |

---

## Match Arms

| Pattern | Meaning |
|---------|---------|
| `"gold":body` | literal text |
| `42:body` | literal number |
| `~v:body` | ok — bind inner value to `v` |
| `^e:body` | err — bind inner value to `e` |
| `_:body` | wildcard |

Arms separated by `;`. First match wins.

```
cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze"
```

```
?r{^e:^+"failed: "e;~v:v}
```

---

## Calls

Positional args, space-separated, no parens:

```
get-user uid
send-email d.email "Notification" msg
charge pid amt
```

### Call Arguments

Call arguments can be atoms or prefix expressions:

```
fac -n 1       -- Call(fac, [Subtract(n, 1)])
fac +a b       -- Call(fac, [Add(a, b)])
g +a b c       -- Call(g, [Add(a,b), c])  — 2 args
fac p           -- Call(fac, [Ref(p)])
```

Use parentheses when you need a full expression (including another call) as an argument:

```
f (g x)        -- Call(f, [Call(g, [x])])
```

---

## Records

Define:
```
type point{x:n;y:n}
```

Construct (type name as constructor):
```
p=point x:10 y:20
```

Access:
```
p.x
ord.addr.country
```

Update:
```
ord with total:fin cost:sh
```

---

## Tools (external calls)

```
tool <name>"<description>" <params>><return-type> timeout:<n>,retry:<n>
```

```
tool get-user"Retrieve user by ID" uid:t>R profile t timeout:5,retry:2
```

---

## Error Handling

`R ok err` return type. Call then match:

```
get-user uid;?{^e:^+"Lookup failed: "e;~d:use d}
```

Compensate/rollback inline:

```
charge pid amt;?{^e:release rid;^+"Payment failed: "e;~cid:continue}
```

---

## Patterns (for LLM generators)

### Bind-first pattern

Always bind complex expressions to variables before using them in operators. Operators only accept atoms and nested operators as operands — not function calls.

```
-- DON'T: *n fac -n 1     (fac is an operand of *, not a call)
-- DO:    r=fac -n 1;*n r  (bind call result, then use in operator)
```

### Recursion template

```
<name> <params>><return>;<guard>;...;<recursive-calls>;combine
```

1. **Guard**: base case returns early — `<=n 1{1}`
2. **Bind**: bind recursive call results — `r=fac -n 1`
3. **Combine**: use bound results in final expression — `*n r`

### Factorial

```
fac n:n>n;<=n 1{1};r=fac -n 1;*n r
```

- `<=n 1{1}` — guard: if n <= 1, return 1
- `r=fac -n 1` — recursive call with prefix subtract as argument
- `*n r` — multiply n by result

### Fibonacci

```
fib n:n>n;<=n 1{n};a=fib -n 1;b=fib -n 2;+a b
```

- `<=n 1{n}` — base case: return n for 0 and 1
- `a=fib -n 1;b=fib -n 2` — two recursive calls, each with prefix arg
- `+a b` — add results

### Multi-statement bodies

Semicolons separate statements. Last expression is the return value.

```
f x:n>n;a=*x 2;b=+a 1;*b b    -- (x*2 + 1)^2
```

### DO / DON'T

```
-- DON'T: fac n:n>n;<=n 1{1};*n fac -n 1
--   ↑ *n sees fac as an atom operand, not a call

-- DO:    fac n:n>n;<=n 1{1};r=fac -n 1;*n r
--   ↑ bind-first: call result goes into r, then *n r works

-- DON'T: +fac -n 1 fac -n 2
--   ↑ + takes two operands; fac is just an atom ref

-- DO:    a=fac -n 1;b=fac -n 2;+a b
--   ↑ bind both calls, then combine
```

---

## Complete Example

```
tool get-user"Retrieve user by ID" uid:t>R profile t timeout:5,retry:2
tool send-email"Send an email" to:t subject:t body:t>R _ t timeout:10,retry:1
type profile{id:t;name:t;email:t;verified:b}
ntf uid:t msg:t>R _ t;get-user uid;?{^e:^+"Lookup failed: "e;~d:!d.verified{^"Email not verified"};send-email d.email "Notification" msg;?{^e:^+"Send failed: "e;~_:~_}}
```

### Recursive Example

Factorial and Fibonacci as standalone functions:

```
fac n:n>n;<=n 1{1};r=fac -n 1;*n r
```

```
fib n:n>n;<=n 1{n};a=fib -n 1;b=fib -n 2;+a b
```
