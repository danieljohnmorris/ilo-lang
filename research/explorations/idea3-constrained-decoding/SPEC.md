# Idea 3: Constrained Decoding

ilo as a grammar fed to a constrained decoder. The agent generates tokens, but at each step the runtime masks invalid next-tokens. The agent literally cannot write an invalid program.

The runtime provides a JSON Schema. The constrained decoder (Outlines/Guidance/LMQL) uses it to mask invalid tokens at each generation step. Programs are minified JSON — one line per function.

## Top-Level Structure

Each function is a single-line JSON object:

```json
{"fn":"<name>","in":{...},"out":"<type>","deps":[...],"body":[...]}
```

Keys are abbreviated: `"fn"` not `"function"`, `"in"` not `"input"`, `"out"` not `"output"`, `"ret"` not `"return"`.

Multiple functions are sequential single-line JSON objects.

## Types

`"num"`, `"text"`, `"bool"`, `"nil"`, `"list <type>"`, `"result <ok-type> <error-type>"` (space-separated).

Note: `"num"` not `"number"` (abbreviated for token efficiency).

## References

Bare variable names without sigil — no `$` prefix:

- `"user-id"` — input parameter (same name as declared in `"in"`)
- `"user"` — bound variable
- `"user.verified"` — field access with dot notation
- `"c.spent"` — field access on loop variable

## Body Statements

### Let binding with expression

```json
{"let":"<var>","op":"<operator>","a":"<ref>","b":"<ref>"}
```

Operators: `"*"`, `"+"`, `"-"`. Nested expressions inline:

```json
{"let":"final","op":"+","a":{"op":"-","a":"order.subtotal","b":"disc"},"b":"ship"}
```

### Let binding with tool call

```json
{"let":"<var>","call":"<tool>","args":{...}}
```

### Tool call with error handling

```json
{"let":"<var>","call":"<tool>","args":{...},"err":"Message: ${err}"}
```

`${err}` is interpolated in the error message string.

### Tool call with compensate

```json
{"let":"cid","call":"charge","args":{...},"err":"Payment failed: ${err}","compensate":[{"call":"release","args":{...}}]}
```

### Conditional

```json
{"if":{"not":"user.verified"},"ret":{"err":"Email not verified"}}
{"if":{">=":["score",500]},"ret":"silver"}
```

### Return

```json
{"ret":{"ok":<value>}}
{"ret":{"err":"<message>"}}
{"ret":{"ok":null}}
```

### For loop

```json
{"for":"<var>","in":"<list-ref>","yield":[...steps...,{"obj":{...}}]}
```

### Match (discrete values)

```json
{"let":"<var>","match":"<ref>","cases":{"gold":20,"silver":10,"bronze":5}}
```

### Object construction

```json
{"obj":{"name":"c.name","level":"level","discount":"disc"}}
```

Used as the last step in a `yield` block to build the output object.

### Object merge

```json
{"ret":{"ok":{"merge":"order","set":{"total":"final","cost":"ship"}}}}
```

## Complete Example

```json
{"fn":"notify","in":{"user-id":"text","message":"text"},"out":"result nil text","body":[{"let":"user","call":"get-user","args":{"user-id":"user-id"},"err":"User lookup failed: ${err}"},{"if":{"not":"user.verified"},"ret":{"err":"Email not verified"}},{"call":"send-email","args":{"to":"user.email","subject":"Notification","body":"message"},"err":"Send failed: ${err}"},{"ret":{"ok":null}}]}
```
