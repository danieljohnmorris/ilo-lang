# Idea 7: Dense Wire Format

ilo as a compressed text format optimised for tokeniser efficiency. Spaces removed where tokeniser merges help. Semicolons instead of newlines. No `fn`/`let` keywords — just bare assignments.

- Maximally terse — function names double as declarations
- Positional-style args (space-separated, no commas)
- Braces for blocks instead of indentation
- Looks ugly. Optimised for token count, not readability.

## Declarations

### Functions

One function per line. Name is the declaration — no `fn` keyword:

```
<name>(<param>:<type> <param>:<type> ...)-><return-type>;<body>
```

Parameters are space-separated (no commas), `name:type` with no spaces around colon. `->` separates signature from return type. `;` separates statements in the body.

### Types

```
type <name>{<field>:<type>;<field>:<type>}
```

Braces with `;`-separated fields.

### Tools

```
tool <name>"<description>"(<params>)-><return-type> timeout:<n>,retry:<n>
```

Description string immediately after name (no space needed).

## Types

Same as idea1: `number`, `text`, `bool`, `nil`, `list <type>`, `result <ok-type> <error-type>` (space-separated).

## Calls

Named args in parens, space-separated (no commas):

```
get-user(user-id:uid)
send-email(to:data.email subject:"Notification" body:msg)
```

## Operators

Prefix notation, same as idea1. No spaces required between operator and first operand:

```
*price quantity
+sub tax
+(- order.subtotal disc) ship
>=spent 1000
concat"Lookup failed: "e
not data.verified
```

## Statements

| Statement | Form |
|-----------|------|
| Bind | `<var>=<expr>` (no `let` keyword) |
| Conditional | `if <cond>{<body>}` |
| Match (result) | `match <var>{err <e>:<body>;ok <x>:<body>}` |
| Match (values) | `match <var>{"gold":20;"silver":10;"bronze":5}` |
| Iteration | `for <var> in <list>{<body>}` |

Semicolons separate statements. Braces delimit blocks. `_` wildcard in match arms: `ok _:value`.

## Error Handling

`result T, E` return types. Pattern match on results:

```
match user{err e:err concat"Lookup failed: "e;ok data:use data}
```

For compensate/rollback, call the rollback function inline in the error arm:

```
match charged{err e:release(rid:rid);err concat"Payment failed: "e;ok cid:continue}
```

No special `compensate` keyword — rollback is explicit code.

## Object Construction and Update

Type name as constructor (space-separated fields):

```
summary name:c.name level:level discount:disc
receipt oid:oid cid:cid rid:rid
```

Update with `with`:

```
order with total:final cost:ship
```

Combine with `ok` for result return:

```
ok receipt oid:oid cid:cid rid:rid
```

## Field Access

Dot notation: `data.email`, `c.spent`, `order.addr.country`.

## Complete Example

```
tool get-user"Retrieve user by ID"(user-id:text)->result profile,text timeout:5,retry:2
tool send-email"Send an email"(to:text,subject:text,body:text)->result nil,text timeout:10,retry:1
type profile{id:text;name:text;email:text;verified:bool}
notify(uid:text msg:text)->result nil text;user=get-user(uid:uid);match user{err e:err concat"Lookup failed: "e;ok data:if not data.verified{err"Email not verified"};sent=send-email(to:data.email subject:"Notification" body:msg);match sent{err e:err concat"Send failed: "e;ok _:ok nil}}
```
