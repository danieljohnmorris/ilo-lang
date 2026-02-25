# Idea 2: Extended Tool Calling

ilo as a sequence of tool calls with flow control, built on top of the function-calling JSON format agents already generate.

- `$references` for data flow between steps
- `on-error` for error handling
- `compensate` for rollback on failure
- Runtime is a step executor

Agents already generate JSON for tool calls. This extends that with sequencing and error handling. No new syntax to learn — just JSON with conventions.

## Top-Level Structure

Each function is a JSON object:

```json
{
  "function": "<name>",
  "input": {"<param>": "<type>", ...},
  "output": "<type>",
  "deps": ["<dep-name>", ...],
  "steps": [...]
}
```

Multiple functions in a file are sequential JSON objects (not wrapped in an array).

## Types

`"number"`, `"text"`, `"bool"`, `"nil"`, `"list <type>"`, `"result <ok-type> <error-type>"` (space-separated).

Examples: `"result nil text"`, `"result order text"`, `"list customer"`.

## References

- `"$input.<field>"` — access an input parameter
- `"$<var>"` — reference a bound variable
- `"$<var>.<field>"` — field access on a variable
- `"$error"` — the error value inside `on-error` handlers

## Steps

### Let binding with expression

```json
{"let": "<var>", "expr": {"*": ["$a", "$b"]}}
```

Operators as JSON keys: `"+"`, `"-"`, `"*"`, `"/"`, `">="`, `"not"`.

### Let binding with tool call

```json
{"let": "<var>", "call": "<tool>", "args": {"<param>": "<ref>"}}
```

### Tool call with error handling

```json
{
  "let": "<var>",
  "call": "<tool>",
  "args": {"<param>": "<ref>"},
  "on-error": {"return": {"error": "Message: $error"}}
}
```

`$error` is interpolated in the error message string.

### Tool call without binding

```json
{"call": "<tool>", "args": {...}, "on-error": {...}}
```

### Conditional

```json
{"if": {"not": "$user.verified"}, "return": {"error": "Email not verified"}}
```

`if` steps always pair with `return` for early exit.

### Return

```json
{"return": {"ok": <value>}}
{"return": {"error": "<message>"}}
{"return": {"ok": null}}
```

### For loop

```json
{"for": "<var>", "in": "$<list>", "do": [
  ...steps...,
  {"yield": {<object fields>}}
]}
```

### Match (discrete values)

```json
{"let": "<var>", "match": "$<expr>", "cases": {"gold": 20, "silver": 10, "bronze": 5}}
```

## Error Handling

Per-step `on-error` with optional `compensate` for rollback:

```json
{
  "let": "charged",
  "call": "charge",
  "args": {"payment-id": "$input.payment-id", "amount": "$input.amount"},
  "on-error": {
    "compensate": [{"call": "release", "args": {"reservation-id": "$rid"}}],
    "return": {"error": "Payment failed: $error"}
  }
}
```

## Object Merge

```json
{"$merge": ["$input.order", {"total": "$final", "cost": "$ship"}]}
```

## Complete Example

```json
{
  "function": "notify",
  "input": {"user-id": "text", "message": "text"},
  "output": "result nil text",
  "steps": [
    {
      "let": "user",
      "call": "get-user",
      "args": {"user-id": "$input.user-id"},
      "on-error": {"return": {"error": "User lookup failed: $error"}}
    },
    {
      "if": {"not": "$user.verified"},
      "return": {"error": "Email not verified"}
    },
    {
      "call": "send-email",
      "args": {
        "to": "$user.email",
        "subject": "Notification",
        "body": "$input.message"
      },
      "on-error": {"return": {"error": "Send failed: $error"}}
    },
    {"return": {"ok": null}}
  ]
}
```
