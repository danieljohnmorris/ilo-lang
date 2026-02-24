# Grammar Notes

Emerging grammar extracted from the five example programs.

## Top-Level Declarations

```
fn <name> ...
type <name> ...
tool <name> <description> ...
```

No `define` prefix. The keyword itself (`fn`, `type`, `tool`) starts the declaration. Indentation-based — no closing delimiter.

## Function Structure

```
fn <name>
	@ <dep> from <module>     -- optional, dependency edges
	<params> -> <return-type>  -- signature
	? <call> == <expected>     -- optional, property tests
	<body>                     -- statements
```

Section order is implicit: `@` deps first, then signature (`->`), then `?` tests, then body. No section markers needed.

## Type Structure

```
type <name>
	<field>: <type>
```

Flat field list. No methods, no inheritance. Types are data shapes.

## Tool Structure

```
tool <name> <description>
	<params> -> <return-type>
	timeout: <n>, retry: <n>   -- optional
```

Description is inline after the name. Tools declare the contract; the runtime provides the implementation.

## Parameters and Signatures

```
price: number, quantity: number -> number
```

`<name>: <type>` for each param, comma-separated. `->` separates inputs from output type. No `input:`/`output:` markers.

## Arguments (call site)

```
calculate-total price: 10, quantity: 2
```

Always named. No positional args.

## Types

Built-in: `number`, `text`, `bool`, `nil`
Parameterised: `list <type>`, `option <type>`, `result <ok-type>, <err-type>`
User-defined: referenced by name (e.g., `customer-record`)

## Identifiers

`[a-z][a-z0-9]*(-[a-z0-9]+)*` — lowercase with hyphens.

**Prefer single words.** Common English words (`price`, `quantity`, `user`, `email`, `data`, `total`, `tax`) are 1 token across all major LLM tokenisers. Hyphenated compounds (`tax-rate`, `fetch-user`, `shipping-address`) are always 2 tokens — the hyphen forces a split. Every hyphen doubles the cost of a name.

Guidelines:
- Use single words where unambiguous: `total` not `calculate-total`, `addr` not `shipping-address`
- Use hyphens only when a single word would be ambiguous in context
- Function names can be single words if the signature disambiguates: `fn total` with `price: number, quantity: number -> number` is clear
- Type names should be single words where possible: `user` not `user-data`, `item` not `item-line`

## Operators

Symbol operators, prefix notation:

```
+ a b       -- addition
- a b       -- subtraction
* a b       -- multiplication
/ a b       -- division
== a b      -- equality
!= a b      -- inequality
>= a b      -- greater or equal
<= a b      -- less or equal
> a b       -- greater than
< a b       -- less than
not x       -- logical not
and a b     -- logical and
or a b      -- logical or
concat a b  -- string concatenation
```

## Statements

| Statement | Form |
|-----------|------|
| Let binding | `let <name> = <expr>` |
| If | `if <expr>` (body indented) |
| Match | `match <expr>` with indented `<pattern>: <body>` arms |
| For | `for <name> in <expr>` (body indented) |
| Log | `log <level> <expr>` |

Implicit return: last expression in a function is the return value. No `return` keyword needed.

All blocks use indentation — no `end`, no `}`.

## Result Type and Error Handling

Construct:
```
ok value
err message
```

Destructure:
```
match result-value
	ok data: ...
	err e: ...
```

Shortcut (after match confirms success):
```
let value = unwrap result-value
```

No exceptions. No try/catch. No null. Every function that can fail returns `result`.

## Dependency Declaration

```
@ function-name from module-name
```

- `from self` — same file
- `from tools` — external tool
- `from <module>` — another module

`@` is the dependency marker. Appears before the signature.

## Property Tests

```
? function-name arg: val, arg: val == expected
```

`?` introduces an inline test. Lives between signature and body.

## Record Construction and Update

```
-- Construct
loyalty-summary
	customer-name: c.name,
	level: level

-- Update
order with total: new-total
```

Type name IS the constructor. `with` creates a copy with fields changed.

## Comments

```
-- single line comment
```

## Indentation

Tab-based. One tab = one indent level. Indentation is significant (determines block structure). No closing delimiters.

## Open Questions

1. **Match exhaustiveness**: should the verifier require all patterns to be covered?
2. **`unwrap` safety**: should `unwrap` be allowed, or must every result be matched?
3. **Multi-line args**: should continuation lines be indented, or use explicit line continuation?
4. **`for` as expression**: does `for` always return a list? Or is it a statement?
5. **`with` record update**: should this be deeper (nested field updates)?
6. **`concat`**: only remaining word-operator. Should string concatenation use a symbol?
