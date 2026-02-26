# Approach 3: AST-Level Generation

Skip text entirely. The agent emits AST node IDs, not text tokens.
Each generation step picks a node type from the valid set.

## How it works

The runtime defines registries that map integer IDs to names. The agent generates instructions using integer IDs. Registry comments at the top of each program document the mappings.

## File Structure

```
; fn <name>(<params>) -> <return-type>
; Vars: 0=<name> 1=<name> ...
; Tools: 0=<name> 1=<name> ...
; Fields: 0=<name> 1=<name> ...
<instructions>
```

Registry comments (`;` prefix) map IDs to human-readable names. The VM uses integer IDs only.

## Value Expressions

Values appear as operands in instructions:

| Form | Meaning | Example |
|------|---------|---------|
| `REF <var-id>` | Reference a variable slot | `REF 0` |
| `LIT 0 "<string>"` | String literal | `LIT 0 "hello"` |
| `LIT 1 <number>` | Number literal | `LIT 1 42` |
| `LIT 2 nil` | Nil literal | `LIT 2 nil` |
| `FIELD <var-id> <field-id>` | Field access | `FIELD 0 2` |

## Instructions

### LET — bind a value to a variable slot

```
LET <dst-var-id> <expr>
```

Expression can be a CALL, arithmetic, CONCAT, OBJ, etc.

### CALL — invoke a tool

```
CALL <tool-id> <arg-count> <field-id> <value> ...
```

Example: `CALL 0 1 0 REF 0` = call tool 0, 1 arg, field 0 = variable 0.

### RET, RET_OK, RET_ERR — return a value

```
RET <expr>          -- return raw value
RET_OK <expr>       -- return ok(value)
RET_ERR <expr>      -- return err(value)
```

### IF — conditional branch

```
IF <condition> <then-stmt-count>
  <then-body instructions...>
```

The count tells the VM how many instructions follow in the branch.

### MATCH — pattern match on result type

```
MATCH <var-id> <arm-count>
  ERR <bind-id> <stmt-count>
    <err-body instructions...>
  OK <bind-id> <stmt-count>
    <ok-body instructions...>
```

### FOR — iterate over a list

```
FOR <iteration-var-id> <list-ref> <stmt-count>
  <body instructions...>
```

### MATCH_MAP — discrete value mapping

```
MATCH_MAP <var-ref> <case-count> <key1> <val1> <key2> <val2> ...
```

Example: `MATCH_MAP REF 2 3 "gold" 20 "silver" 10 "bronze" 5`

### OBJ — construct an object

```
OBJ <field-count> <field-id1> <val1> <field-id2> <val2> ...
```

### MERGE — update an object with new fields

```
MERGE <obj-ref> <field-id> <val> ...
```

### Arithmetic and Logic

```
ADD <a> <b>
SUB <a> <b>
MUL <a> <b>
GTE <a> <b>
NOT <a>
CONCAT <a> <b>
```

## Error Handling

Done via `MATCH` with `ERR` and `OK` arms. For compensate/rollback, the ERR arm calls the rollback tool before returning error. No special `compensate` instruction.

## Complete Example

```
; fn notify(user-id: text, message: text) -> result nil, text
; Vars: 0=user-id 1=message 2=user 3=sent
; Tools: 0=get-user 1=send-email
; Fields: 0=user-id 1=email 2=verified 3=subject 4=body
LET 2 CALL 0 1 0 REF 0
MATCH 2 2
  ERR 4 1
    RET_ERR CONCAT LIT 0 "User lookup failed: " REF 4
  OK 5 4
    IF NOT FIELD 5 2 1
      RET_ERR LIT 0 "Email not verified"
    LET 3 CALL 1 3 1 FIELD 5 1 3 LIT 0 "Notification" 4 REF 1
    MATCH 3 2
      ERR 6 1
        RET_ERR CONCAT LIT 0 "Send failed: " REF 6
      OK 7 1
        RET_OK LIT 2 nil
```
