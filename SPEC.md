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

Disambiguation: `-` followed by one atom is unary negate, followed by two atoms is binary subtract.

---

## Builtins

Called like functions, compiled to dedicated opcodes.

| Call | Meaning | Returns |
|------|---------|---------|
| `len x` | length of string (bytes) or list (elements) | `n` |
| `str n` | number to text (integers format without `.0`) | `t` |
| `num t` | text to number (Err if unparseable) | `R n t` |
| `abs n` | absolute value | `n` |

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
| `!expr` | return err |

---

## Match Arms

| Pattern | Meaning |
|---------|---------|
| `"gold":body` | literal text |
| `42:body` | literal number |
| `~v:body` | ok — bind inner value to `v` |
| `!e:body` | err — bind inner value to `e` |
| `_:body` | wildcard |

Arms separated by `;`. First match wins.

```
cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze"
```

```
?r{!e:!+"failed: "e;~v:v}
```

---

## Calls

Positional args, space-separated, no parens:

```
get-user uid
send-email d.email "Notification" msg
charge pid amt
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
get-user uid;?{!e:!+"Lookup failed: "e;~d:use d}
```

Compensate/rollback inline:

```
charge pid amt;?{!e:release rid;!+"Payment failed: "e;~cid:continue}
```

---

## Complete Example

```
tool get-user"Retrieve user by ID" uid:t>R profile t timeout:5,retry:2
tool send-email"Send an email" to:t subject:t body:t>R _ t timeout:10,retry:1
type profile{id:t;name:t;email:t;verified:b}
ntf uid:t msg:t>R _ t;get-user uid;?{!e:!+"Lookup failed: "e;~d:!d.verified{!"Email not verified"};send-email d.email "Notification" msg;?{!e:!+"Send failed: "e;~_:~_}}
```
