# Idea 6: MCP Composition Layer

ilo as a thin layer on top of Model Context Protocol. Tools are already MCP servers. ilo adds composition, data flow, and error handling.

- Tools referenced by `mcp://` URIs
- MCP already defines tool discovery, typed schemas, and invocation
- ilo adds: sequencing (chain calls with data flow), error handling (`err` on each step), verification (all URIs resolved before execution, all types checked)
- `compensate` for rollback on multi-step failures
- The agent discovers tools via MCP, composes them via ilo

## Top-Level Structure

Each function is a JSON object:

```json
{
  "meta": {
    "tools": ["mcp://<server>/<tool>", ...],
    "input": {"<param>": "<type>", ...},
    "output": "<type>"
  },
  "steps": [...]
}
```

`"meta"` separates metadata from execution steps. `"tools"` declares MCP tool dependencies by URI.

Multiple functions in a file are sequential JSON objects.

## Types

`"number"`, `"text"`, `"bool"`, `"nil"`, `"list <type>"`, `"result <ok-type> <error-type>"` (space-separated).

## References

- `"$.field"` — input parameter
- `"$.order.addr.country"` — nested input field access
- `"$var"` — bound variable (simple reference)
- `"${var.field}"` — field access with interpolation

## Tool URIs

- `mcp://<server>/<tool>` — MCP server/tool path
- `mcp://self/<tool>` — reference a function in the same composition

## Steps

### Let binding with expression

```json
{"let": "<var>", "expr": {"*": ["$.price", "$.quantity"]}}
```

Operators as JSON keys: `"+"`, `"-"`, `"*"`, `"/"`, `">="`, `"not"`.

### Let binding with tool call

```json
{"let": "<var>", "call": "mcp://<server>/<tool>", "with": {"<param>": "<ref>"}}
```

Note: `"with"` not `"args"` (aligns with MCP protocol).

### Tool call with error handling

```json
{"let": "<var>", "call": "mcp://users/get-user", "with": {"user-id": "$.user-id"},
 "err": "User lookup failed: ${err}"}
```

`${err}` is interpolated in the error message string.

### Tool call with compensate

```json
{"let": "charged", "call": "mcp://payments/charge",
 "with": {"payment-id": "$.payment-id", "amount": "$.amount"},
 "err": "Payment failed: ${err}",
 "compensate": [{"call": "mcp://inventory/release", "with": {"reservation-id": "$rid"}}]}
```

### Assert (guard)

```json
{"assert": "${user.verified}", "err": "Email not verified"}
```

Fails with error if value is falsy. Unique to this format.

### Conditional return

```json
{"if": {">=": ["$.spent", 1000]}, "ok": "gold"}
```

`"if"` paired with `"ok"` for conditional early return.

### Return

```json
{"ok": <value>}
{"ok": null}
{"ok": {"order-id": "$oid", "charge-id": "$cid"}}
```

Note: bare `"ok"` at step level (not wrapped in `"return"`).

### For loop

```json
{"for": "<var>", "in": "$.customers", "yield": [
  {"let": "level", "call": "mcp://self/classify", "with": {"spent": "${c.spent}"}},
  {"obj": {"name": "${c.name}", "level": "$level", "discount": "$disc"}}
]}
```

### Match (discrete values)

```json
{"let": "disc", "match": "$level", "cases": {"gold": 20, "silver": 10, "bronze": 5}}
```

### Object construction

```json
{"obj": {"name": "${c.name}", "level": "$level", "discount": "$disc"}}
```

### Object merge

```json
{"ok": {"merge": "$.order", "set": {"total": "$final", "cost": "$ship"}}}
```

## Complete Example

```json
{
  "meta": {
    "tools": ["mcp://users/get-user", "mcp://email/send-email"],
    "input": {"user-id": "text", "message": "text"},
    "output": "result nil text"
  },
  "steps": [
    {"let": "user", "call": "mcp://users/get-user", "with": {"user-id": "$.user-id"},
     "err": "User lookup failed: ${err}"},
    {"assert": "${user.verified}", "err": "Email not verified"},
    {"call": "mcp://email/send-email",
     "with": {"to": "${user.email}", "subject": "Notification", "body": "$.message"},
     "err": "Send failed: ${err}"},
    {"ok": null}
  ]
}
```
