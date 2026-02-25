# Idea 8: Ultra-Dense Format

ilo pushed to minimum characters AND tokens. Every keyword shortened to 1 char. Positional args instead of named. Implicit last-result matching.

## Changes from idea7

| idea7 | idea8 | Meaning |
|-------|-------|---------|
| `->` | `>` | return type separator |
| `number` / `text` / `bool` | `n` / `t` / `b` | 1-char types |
| `nil` | `_` | nil type |
| `list T` | `L T` | list type |
| `result T E` | `R T E` | result type |
| `call(name:val name:val)` | `call val val` | positional args (drop names, parens) |
| `fn(p:type p:type)>` | `fn p:type p:type>` | no parens in declarations |
| `x=call(...);match x{err e:;ok v:}` | `call val;?{!e:;~v:}` | implicit last-result match |
| `match x{"a":1;"b":2}` | `?x{"a":1;"b":2}` | value match |
| `for c in list` | `@c list` | iteration |
| `if <cond>{...}` | `<cond>{...}` | conditional (drop `if`) |
| `not x` | `!x` | negation guard |
| `concat"a"b` | `+"a"b` | string concat |
| `err <expr>` | `!<expr>` | construct error |
| `ok <expr>` | `~<expr>` | construct ok |

## Declarations

### Functions

```
<name> <param>:<type> ...><return-type>;<body>
```

No parens around params — `>` marks where params end and return type begins.

### Types

```
type <name>{<field>:<type>;<field>:<type>}
```

### Tools

```
tool <name>"<description>" <params>><return-type> timeout:<n>,retry:<n>
```

## Types

| Type | Syntax |
|------|--------|
| Number | `n` |
| Text | `t` |
| Bool | `b` |
| Nil | `_` |
| List | `L <type>` |
| Result | `R <ok-type> <err-type>` |
| Named | just the name, e.g. `order` |

## Calls

Positional args, space-separated, no parens, no names:

```
get-user uid
send-email data.email "Notification" msg
reserve items
charge pid amt
```

The callee's param names define the order. Caller just passes values in position.

## Operators

Prefix notation. `+` doubles as string concat. All comparison operators: `>`, `<`, `>=`, `<=`, `=`, `!=`.

```
*p q
+s t
+"Lookup failed: "e
>=spent 1000
<score 500
>ratio 0.4
/ debt income
```

## Statements

| Statement | Form |
|-----------|------|
| Bind | `<var>=<expr>` |
| Conditional | `<cond>{<body>}` (no `if` keyword) |
| Negation guard | `!<expr>{<body>}` |
| Match last result | `?{!<e>:<body>;~<v>:<body>}` |
| Match named | `?<var>{!<e>:<body>;~<v>:<body>}` |
| Match values | `?<var>{"gold":20;"silver":10}` |
| Iteration | `@<var> <list>{<body>}` |
| Return error | `!<expr>` |
| Return ok | `~<expr>` |

`?` with no argument matches the last expression's result. `?x` matches a named variable.

## Error Handling

`R T E` return types. Call then match:

```
get-user uid;?{!e:!+"Lookup failed: "e;~data:use data}
```

Compensate/rollback inline:

```
charge pid amt;?{!e:release rid;!+"Payment failed: "e;~cid:continue}
```

## Object Construction and Update

Type name as constructor:

```
summary name:c.name level:level discount:disc
receipt oid:oid cid:cid rid:rid
```

Update with `with`:

```
order with total:final cost:ship
```

## Field Access

Dot notation: `d.email`, `c.spent`, `ord.addr.country`.

## Naming Style

Prefer 1-3 character variable names. Shorter names don't save tokens (the tokeniser already treats common words as single tokens) but reduce character count and reinforce the dense aesthetic.

| Long | Short | Technique |
|------|-------|-----------|
| `order` | `ord` | truncate to 3 |
| `customers` | `cs` | first + last consonant |
| `data` | `d` | single letter |
| `level` | `lv` | drop vowels |
| `discount` | `dc` | initials |
| `shipped` | `sh` | first 2 |
| `final` | `fin` | first 3 |
| `items` | `its` | first 3 |
| `spent` | `sp` | first 2 |

Function names and field names in constructors keep their full form (they define the public interface).

## Complete Example

```
tool get-user"Retrieve user by ID" uid:t>R profile t timeout:5,retry:2
tool send-email"Send an email" to:t subject:t body:t>R _ t timeout:10,retry:1
type profile{id:t;name:t;email:t;verified:b}
notify uid:t msg:t>R _ t;get-user uid;?{!e:!+"Lookup failed: "e;~data:!data.verified{!"Email not verified"};send-email data.email "Notification" msg;?{!e:!+"Send failed: "e;~_:~_}}
```
