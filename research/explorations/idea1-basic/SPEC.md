# Idea 1: ilo (indented syntax)

ilo as an indented, whitespace-significant language inspired by Haskell/Elm.

- `fn name` declares a function, indented body
- `type name` declares a record type with named fields
- `tool name "description"` declares an external tool with timeout/retry
- `@ dep from source` imports dependencies
- Parameters: `name: type` with `->` return type
- `?` lines are inline tests (input == expected)
- `let x = expr` for bindings
- `match value` with indented arms for pattern matching
- `for x in collection` for iteration
- `err "msg"` / `ok value` for result types
- `result T, E` for Result return types
- Comments start with `--`

## Declarations

Three top-level forms:

```
fn <name>
	@ <dep> from <module>
	<params> -> <return-type>
	? <test-call> == <expected>
	<body>

type <name>
	<field>: <type>

tool <name> "<description>"
	<params> -> <return-type>
	timeout: <n>, retry: <n>
```

Functions have: dependencies (`@`), signature (`->`), inline tests (`?`), body. Order is fixed. Last expression is the return value.

Types are flat data shapes. No methods, no inheritance. The type name is the constructor:

```
profile id: "123", name: "Dan", email: "d@x.com"
```

Update with `with`: `order with total: 100`. Access with `.`: `order.total`.

Tools declare external capabilities. The runtime provides the implementation. Tools always return `result` because external calls can fail.

## Types

| Type | Description | Examples |
|------|-------------|----------|
| `number` | Numeric | `42`, `3.14`, `-1` |
| `text` | String | `"hello"` |
| `bool` | Boolean | `true`, `false` |
| `nil` | Absence | `nil` |
| `list T` | Collection | `list number` |
| `option T` | Maybe absent | `option text` |
| `result T, E` | Ok or error | `result profile, text` |

User-defined types are referenced by name after a `type` declaration.

## Signatures and Calls

Signature: `price: number, quantity: number -> number`. Params are `name: type`, comma-separated. `->` separates inputs from output.

Calls always use named args: `total price: 10, quantity: 2, rate: 0.2`

## Operators

Prefix notation, no precedence. Group with parentheses: `+ (* a b) c`

```
+ a b      - a b      * a b      / a b
== a b     != a b     > a b      < a b      >= a b     <= a b
and a b    or a b     not a      concat a b
```

## Statements

| Statement | Form |
|-----------|------|
| Bind | `let x = <expr>` |
| Conditional | `if <cond>` with indented body |
| Pattern match | `match <expr>` with `<pattern>: <body>` arms. `_` = wildcard |
| Iteration | `for x in <collection>` with indented body. Returns a list |
| Logging | `log <level> <expr>` where level is `error`, `info`, `debug` |

Bindings are immutable. All blocks use tab indentation, no closing delimiters. Last expression in a function is the return value — no `return` keyword.

## Error Handling

One mechanism: `result T, E`. No exceptions, no try/catch, no null.

```
ok <value>                    -- construct success
err <message>                 -- construct failure
match result                  -- destructure
	ok x: <use x>
	err e: <handle e>
unwrap result                 -- extract ok (after match confirms)
```

Every function that can fail returns `result`. The caller must handle both cases.

## Dependencies

```
@ get-user from tools         -- external tool
@ classify from self          -- same file
@ validate from address       -- another module
```

Declared before the signature. Makes the call graph explicit.

## Naming

Identifiers: `[a-z][a-z0-9]*(-[a-z0-9]+)*`

- Single word when clear: `fn total`, `type profile`
- Hyphenate when ambiguous: `send-email`, `user-id`
- Never abbreviate: `user-id` not `uid`

## Indentation

Tab-based. One tab = one level. Indentation is significant — determines block structure.

## Comments

```
-- single line comment
```

## Complete Example

```
tool get-user "Retrieve user by ID"
	user-id: text -> result profile, text
	timeout: 5, retry: 2

type profile
	name: text
	email: text
	verified: bool

fn notify
	@ get-user from tools
	user-id: text, message: text -> result nil, text
	let user = get-user user-id: user-id
	match user
		err e: err concat "Lookup failed: " e
		ok data:
			if not data.verified
				err "Email not verified"
			ok nil
```
