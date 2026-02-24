# ilo Language Spec

For the human-readable reference with explanations, see [docs/reference.md](docs/reference.md).

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

## Types

Built-in: `number`, `text`, `bool`, `nil`
Parameterised: `list T`, `option T`, `result T, E`
User-defined: by name after `type` declaration

## Signatures

```
x: number, y: number -> number
```

Params are `name: type`, comma-separated. `->` separates inputs from output.

## Calls

Always named args:

```
total price: 10, quantity: 2, rate: 0.2
```

## Operators

Prefix, no precedence:

```
+ a b      - a b      * a b      / a b
== a b     != a b     > a b      < a b     >= a b     <= a b
and a b    or a b     not a      concat a b
```

Group with parentheses: `+ (* a b) c`

## Statements

```
let x = <expr>
if <cond>
	<body>
match <expr>
	<pattern>: <body>
for x in <collection>
	<body>
log <level> <expr>
```

Last expression in a function is the return value. All blocks use tab indentation.

## Results

```
ok <value>
err <message>
```

Destructure with match:

```
match result
	ok x: <use x>
	err e: <handle e>
```

`unwrap` extracts after match confirms ok.

## Dependencies

```
@ get-user from tools
@ classify from self
@ validate from address
```

`from self` = same file. `from tools` = external tool. `from <module>` = another module.

## Properties (inline tests)

```
? price: 10, quantity: 2 == 20
```

## Records

Construct: `profile name: "Dan", email: "d@x.com"`
Update: `order with total: 100`
Access: `order.total`

## Naming

Identifiers: `[a-z][a-z0-9]*(-[a-z0-9]+)*`

Prefer single words (`total`, `validate`, `profile`). Use hyphens only when a single word would be ambiguous (`user-id`, `send-email`). Single words = 1 token. Hyphens = 2 tokens.

## Comments

```
-- comment
```
