# Approach 2: Constrained Decoding

ilo isn't a language — it's a grammar fed to a constrained decoder.
The agent generates tokens, but at each step the runtime masks invalid
next-tokens. The agent literally cannot write an invalid program.

## How it works

The runtime provides a grammar (JSON Schema, CFG, or regex):

```json
{
  "type": "program",
  "steps": {
    "type": "array",
    "items": {
      "oneOf": [
        {"$ref": "#/defs/tool-call"},
        {"$ref": "#/defs/branch"},
        {"$ref": "#/defs/return"}
      ]
    }
  },
  "defs": {
    "tool-call": {
      "properties": {
        "call": {"enum": ["get-user", "send-email"]},
        "args": {}
      }
    }
  }
}
```

The constrained decoder (Outlines/Guidance) uses this schema to mask
invalid tokens at each generation step. The agent can only produce:

```json
{"steps":[{"call":"get-user","args":{"user-id":"$input.user-id"}},{"if":{"not":"$1.verified"},"return":{"error":"Email not verified"}},{"call":"send-email","args":{"to":"$1.email","subject":"Notification","body":"$input.message"}},{"return":{"ok":null}}]}
```

## Token count

The generated output is JSON — the tokeniser handles it efficiently.
But the schema itself costs tokens to load. For a system with 50 tools,
the schema could be thousands of tokens.

## What ilo becomes

Not a language. A schema generator. Given a set of tools and types,
ilo generates the JSON Schema that constrains the agent's output.
The "language" is the schema. The "runtime" is the constrained decoder.

## Tradeoffs

- Pro: impossible to generate invalid programs (zero retries)
- Pro: agents already know JSON
- Con: schema must be loaded into context (expensive for large systems)
- Con: constrained decoding is slower per token
- Con: no human readability at all
- Con: flow control in JSON is awkward
