# Approach 3: AST-Level Generation

Skip text entirely. The agent emits AST node IDs, not text tokens.
Each generation step picks a node type from the valid set.

## How it works

The runtime defines an AST:

```
NodeTypes:
  0 = tool-call
  1 = branch
  2 = return-ok
  3 = return-err
  4 = ref (reference a previous step's output)
  5 = literal

ToolIDs:
  0 = get-user
  1 = send-email

FieldIDs:
  0 = user-id
  1 = email
  2 = verified
  3 = subject
  4 = body
  5 = message
```

The agent generates a sequence of integers:

```
0 0 [0:input.0]     → tool-call get-user {user-id: input.user-id}
1 [!4.2] 3 "..."    → if not step0.verified, return-err "..."
0 1 [1:4.1 3:"..." 4:input.5]  → tool-call send-email {to: step0.email, ...}
2                    → return-ok
```

## What ilo becomes

A bytecode format. The "language" is an instruction set. The runtime
is a VM that executes the bytecode.

## Token count

Minimal — integers are 1 token each. A full program might be 15-20
tokens. But the instruction set must be loaded into context, and the
mapping from intent to bytecode is harder for the agent to learn.

## Tradeoffs

- Pro: maximally token-efficient generation
- Pro: impossible to generate invalid bytecode (with constrained decoding)
- Con: agents aren't trained on bytecode — high learning cost
- Con: error messages are hard ("invalid opcode at position 7")
- Con: no human readability
- Con: the instruction set IS the spec — changes break everything
