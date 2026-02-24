# ilo Language Reference

A human-readable guide to the ilo programming language. For the compact agent-loadable spec, see [SPEC.md](../SPEC.md).

## Overview

ilo is a programming language designed for AI agents, not humans. Every design decision optimises for **total tokens from intent to working code** — including generation, retries, error feedback, and context loading.

The language is:
- **Small** — few keywords, one way to do things
- **Explicit** — dependencies, types, and errors are always declared
- **Verifiable** — programs are checked before execution

## Program Structure

An ilo program is a list of declarations. There are three kinds:

### Functions

```
fn total
	price: number, quantity: number, rate: number -> number
	? price: 10, quantity: 2, rate: 0.2 == 24
	let sub = * price quantity
	let tax = * sub rate
	+ sub tax
```

A function has:
1. **Name** — `fn total`
2. **Dependencies** (optional) — `@ validate from address` declares that this function calls `validate`, found in the `address` module
3. **Signature** — `price: number, quantity: number -> number`. Named parameters with types, `->` return type
4. **Properties** (optional) — `? price: 10, quantity: 2 == 20` are inline tests. The function is called with those args and the result is compared
5. **Body** — statements and expressions. The last expression is the return value (no `return` keyword)

### Types

```
type profile
	id: text
	name: text
	email: text
	verified: bool
```

Types are flat data shapes — a list of named, typed fields. No methods, no inheritance.

To construct a value of a type, use the type name as a constructor:

```
profile id: "123", name: "Dan", email: "d@x.com", verified: true
```

To update a field:

```
order with total: 100
```

### Tools

```
tool get-user "Retrieve user by ID"
	user-id: text -> result profile, text
	timeout: 5, retry: 2
```

Tools declare external capabilities (APIs, databases, file systems). The declaration is the **contract** — name, description, input/output types, timeout, retry policy. The runtime provides the implementation.

Tools always return `result` because external calls can fail.

## Type System

### Built-in types

| Type | Description | Examples |
|------|-------------|----------|
| `number` | Numeric value | `42`, `3.14`, `-1` |
| `text` | String | `"hello"`, `""` |
| `bool` | Boolean | `true`, `false` |
| `nil` | Absence of value | `nil` |

### Parameterised types

| Type | Description |
|------|-------------|
| `list T` | Ordered collection |
| `option T` | Value that may be absent |
| `result T, E` | Success (`ok T`) or failure (`err E`) |

### User-defined types

Declared with `type`, referenced by name. See [Types](#types) above.

## Operators

All operators use prefix notation — the operator comes first, then the operands. No operator precedence to remember.

### Arithmetic
```
+ a b       -- addition
- a b       -- subtraction
* a b       -- multiplication
/ a b       -- division
```

### Comparison
```
== a b      -- equality
!= a b      -- inequality
> a b       -- greater than
< a b       -- less than
>= a b      -- greater or equal
<= a b      -- less or equal
```

### Logical
```
and a b     -- logical AND
or a b      -- logical OR
not a       -- logical NOT
```

### String
```
concat a b  -- string concatenation
```

### Grouping

Use parentheses to nest expressions:

```
+ (* price quantity) (* price rate)
```

## Statements

### Let binding

```
let tax = * sub rate
```

Binds a name to a value. Immutable — once bound, a name cannot be reassigned.

### If

```
if not data.verified
	err "User email not verified"
```

Condition is an expression. Body is indented. No `else` keyword — use `match` for multi-branch logic.

### Match

```
match level
	"gold": 20
	"silver": 10
	"bronze": 5
	_: 0
```

Pattern matching. Each arm is `<pattern>: <body>`. `_` is the wildcard (matches anything).

For results:

```
match user
	ok data:
		log info data.name
	err e:
		log error e
```

### For

```
for c in customers
	summary name: c.name, level: classify spent: c.spent
```

Iterates over a list. Returns a new list (expression, not statement).

### Log

```
log error concat "Failed: " e
log info "Success"
```

First argument is level (`error`, `info`, `debug`), second is the message expression.

## Error Handling

ilo has one error mechanism: `result T, E`.

- **Construct**: `ok value` or `err message`
- **Destructure**: `match` with `ok` and `err` arms
- **Shortcut**: `unwrap` extracts the ok value (only safe after a match confirms success)

There are no exceptions, no try/catch, no null. Every function that can fail returns `result`, and the caller must handle both cases.

```
let user = get-user user-id: uid
match user
	err e: err concat "Lookup failed: " e
	ok data:
		-- use data here
```

## Dependencies

Every function declares what it calls:

```
fn notify
	@ get-user from tools
	@ send-email from tools
	@ validate from address
	@ classify from self
```

| Source | Meaning |
|--------|---------|
| `from self` | Same file |
| `from tools` | External tool (declared with `tool`) |
| `from <module>` | Another module |

Dependencies make the call graph explicit. An agent can see what a function needs without reading its body.

## Naming Conventions

Identifiers are lowercase with optional hyphens: `[a-z][a-z0-9]*(-[a-z0-9]+)*`

**Prefer single words** where unambiguous. Across all major LLM tokenisers (OpenAI cl100k, o200k; Anthropic), single English words are 1 token. Hyphens force a token split — `user-id` is 2 tokens, `user` is 1.

Use hyphens when a single word would be ambiguous:
- `send-email` (not `send` — send what?)
- `user-id` (not `uid` — abbreviations lose semantic information)
- `order-id` (not `id` — which id?)

Don't use hyphens when context disambiguates:
- `fn total` with `price: number, quantity: number -> number` — clearly calculates a total
- `type profile` with user fields — clearly a user profile
- `fn classify` with `spent: number -> text` — clearly classifies loyalty

## Indentation

Tab-based. One tab = one level. Indentation is significant — it determines block structure. No closing delimiters (`end`, `}`).

## Comments

```
-- This is a comment
```

Double hyphen, single line only. No block comments.
