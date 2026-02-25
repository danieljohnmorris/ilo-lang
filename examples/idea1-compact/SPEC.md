# Idea 1 Compact: ilo (single-line syntax)

ilo compact is the same language as idea1 but compressed onto single lines for token efficiency.

- `fn name(params)->return_type` declares a function, body follows after `;`
- `type name{fields}` declares a record type with `;`-separated fields
- `tool name"description"(params)->return_type timeout:N,retry:N`
- `@dep from source` imports, comma-separated in parens
- Semicolons replace newlines; braces replace indentation
- `?input==expected` for inline tests
- `match value{arm:expr;arm:expr}` for pattern matching
- `for x in collection{body}` for iteration
- `err"msg"` / `ok value` for results (spaces optional)
- No `let` keyword — bare `name=expr` for bindings
- Comments start with `--`

## Declarations

Three top-level forms, one per line:

```
fn name(@dep from source)(params)->return_type;body
type name{field:type;field:type}
tool name"description"(params)->return_type timeout:N,retry:N
```

Functions: dependencies in first parens, signature in second parens, `->` return type, then `;` and body. One function per line.

Types: braces with `;`-separated fields.

Tools: description string immediately after name (no space needed), params in parens, config after return type.

## Types

Same as idea1: `number`, `text`, `bool`, `nil`, `list T`, `option T`, `result T, E`.

User-defined types referenced by name after `type` declaration.

## Signatures and Calls

Signature: `(price:number,quantity:number,rate:number)->number`. Params are `name:type` (no spaces around colon), comma-separated, in parentheses.

Calls use named args in parens: `total(price:10 quantity:2 rate:0.2)`. Args are space-separated inside parens (no commas in calls).

## Operators

Prefix notation, same as idea1: `*price quantity`, `+sub tax`, `+(- order.subtotal disc) ship`

No spaces required between operator and first operand: `concat"Failed: "e`

## Statements

| Statement | Form |
|-----------|------|
| Bind | `name=expr` (no `let` keyword) |
| Conditional | `if cond{body}` with braces |
| Pattern match | `match expr{arm:body;arm:body}` with `;`-separated arms in braces |
| Iteration | `for x in collection{body}` with braces |

Semicolons separate statements within a line. Braces replace indentation for blocks.

## Error Handling

Same as idea1 but compressed:

```
ok value
err"message"
match result{err e:handle;ok x:use}
unwrap result
```

`result T, E` return types. Caller must handle both cases via `match`.

## Object Construction and Update

Type name as constructor: `summary name:c.name,level:level,discount:disc`

Update with `with`: `order with total:final,cost:ship`

Field access with `.`: `data.email`, `c.spent`

## Dependencies

In first parens of function declaration:

```
fn notify(@get-user from tools,@send-email from tools)(user-id:text,message:text)->result nil,text
```

## Complete Example

```
tool get-user"Retrieve user by ID"(user-id:text)->result profile,text timeout:5,retry:2
tool send-email"Send an email"(to:text,subject:text,body:text)->result nil,text timeout:10,retry:1
type profile{id:text;name:text;email:text;verified:bool}
fn notify(@get-user from tools,@send-email from tools)(user-id:text,message:text)->result nil,text;user=get-user user-id:user-id;match user{err e:log error concat"Failed to fetch user: "e;err concat"User lookup failed: "e;ok data:if not data.verified{err"Email not verified"};sent=send-email to:data.email,subject:"Notification",body:message;match sent{err e:log error concat"Email failed: "e;err concat"Send failed: "e;ok _:log info concat"Notified user "user-id;ok nil}}
```
