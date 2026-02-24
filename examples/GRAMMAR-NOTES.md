# Grammar Notes

Emerging grammar extracted from the five example programs. Not a formal spec — observations on what patterns recur and what the syntax converges toward.

## Top-Level Declarations

Three declaration forms:

```
define function <name> ... end
define type <name> ... end
define tool <name> ... end
```

`define` is always the entry keyword. The second token (`function`, `type`, `tool`) disambiguates. This is constrained — the agent always knows: after `define`, there are exactly 3 valid next tokens.

## Function Structure

```
define function <name>
  requires:          -- optional, dependency edges
    <name> from <module>
  input: <params>    -- required
  output: <type>     -- required
  properties:        -- optional, inline tests
    <call> equals <value>
  body:              -- required
    <statements>
end
```

Section order is fixed: requires → input → output → properties → body. The agent never has to decide where to put a section.

## Type Structure

```
define type <name>
  <field> as <type>
end
```

Flat field list. No methods, no inheritance. Types are data shapes.

## Tool Structure

```
define tool <name>
  description: <text>
  input: <params>
  output: <type>
  timeout: <number>    -- optional
  retry: <number>      -- optional
end
```

Tools declare the contract. The runtime provides the implementation.

## Parameters

```
<name> as <type>
```

Always named, always typed. Multiple params separated by commas:
```
input: price as number, quantity as number
```

## Arguments (call site)

```
function-name arg-name: value, arg-name: value
```

Always named. No positional args at call sites.

## Types

Built-in: `number`, `text`, `bool`, `void`, `nothing`
Parameterised: `list <type>`, `option <type>`, `result <ok-type>, <err-type>`
User-defined: referenced by name (e.g., `customer-record`)

## Identifiers

Lowercase with hyphens: `calculate-total`, `tax-rate`, `order-id`

- No camelCase, no snake_case
- Hyphens are part of the identifier
- Always `[a-z][a-z0-9]*(-[a-z0-9]+)*`

## Operators

All operators are words, prefix notation:

```
add a b            -- a + b
multiply a b       -- a * b
equals a b         -- a == b
greater-or-equal a b  -- a >= b
not condition      -- !condition
concat a b         -- a ++ b / a + b (string)
```

No operator precedence to remember. No symbols to confuse. Each expression is a single function call.

## Statements

| Statement | Form |
|-----------|------|
| Let binding | `let <name> = <expr>` |
| Return | `return <expr>` |
| If | `if <expr> then ... end` or `if <expr> then ... else ... end` |
| Match | `match <expr> on <pattern>: <body> ... end` |
| For-each | `for-each <name> in <expr> do ... end` |
| Log | `log level: <text>, message: <expr>` |

All blocks end with `end`. No indentation sensitivity.

## Expressions

| Expression | Form |
|------------|------|
| Literal | `42`, `"hello"`, `true`, `false`, `nothing` |
| Variable | `name` |
| Field access | `object.field` |
| Function call | `function-name arg: val` |
| Binary op | `op left right` |
| Unary op | `op operand` |
| Record construction | `type-name field: val, field: val` |
| Record update | `record with field: val` |
| Result constructors | `result.ok value`, `result.error value` |

## Error Handling

One mechanism: `result <ok-type>, <err-type>`.

- Construct: `result.ok value` or `result.error message`
- Destructure: `match value on result.ok x: ... result.error e: ... end`
- Shortcut: `result.unwrap value` (only safe after match confirms ok)

No exceptions. No try/catch. No null. Every function that can fail returns `result`.

## Dependency Declaration

```
requires:
  function-name from module-name
```

- `from self` — same file
- `from tools` — external tool
- `from <module>` — another module

Appears before `input` in function declarations. Makes the call graph explicit.

## Comments

```
-- single line comment
```

Two hyphens. No block comments.

## Open Questions

1. **Keyword verbosity**: `define function` (2 tokens) vs `fn` (1 token). Worth ~10 tokens per file.
2. **`body:` section marker**: Could be implicit (everything after `output`/`properties` is body). Saves 1 token per function.
3. **Match syntax**: `match x on pattern: body end` — could `|` arms be terser?
4. **Language-agnostic keywords**: Current syntax is English. Should we explore symbols or constructed words?
5. **Record update**: `record with field: val` — is `with` the right keyword?
6. **`result.unwrap`**: Should this exist, or should every result be matched explicitly?
7. **List expressions in for-each**: `for-each x in list do ... end` as expression — should this return a list implicitly?
