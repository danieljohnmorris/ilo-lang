# Idea 5: Workflow DAG

ilo as a YAML workflow definition, like AWS Step Functions or Temporal.

- Named steps make the DAG explicit
- `$` references for data flow between steps
- `catch` blocks for error handling
- `compensate` for rollback
- YAML is well-tokenised (agents generate lots of YAML)
- The runtime walks the DAG, calling tools at each step
- Verification: all tool names checked, all references resolved

## Top-Level Structure

```yaml
name: <function-name>
input:
  <param>: <type>
output: <type>

steps:
  <step-name>:
    <step-definition>
```

Multiple workflows in one file separated by `---`.

## Types

`number`, `text`, `bool`, `nil`, `list <type>`, `result <ok-type>, <error-type>` (comma-separated for result).

Examples: `result nil, text`, `result order, text`, `list customer`.

## References

- `$.field` — input parameter (JSONPath-style)
- `$.order.addr.country` — nested input field access
- `${step-name}` — reference a prior step's result by name
- `${step-name.field}` — field access on a prior step's result
- `${error}` — the error value inside `catch` blocks
- `${c.field}` — field access on a loop variable

## Step Types

### Expression

```yaml
calc:
  expr: "$.price * $.quantity"
```

Inline math in quoted strings. References use `$` notation.

### Tool call

```yaml
fetch:
  call: get-user
  args:
    user-id: $.user-id
```

### Tool call with error handling

```yaml
fetch:
  call: get-user
  args:
    user-id: $.user-id
  catch:
    return:
      error: "User lookup failed: ${error}"
```

### Tool call with compensate

```yaml
charge:
  call: charge
  args:
    payment-id: $.payment-id
    amount: $.amount
  catch:
    compensate:
      - call: release
        args:
          reservation-id: ${reserve}
    return:
      error: "Payment failed: ${error}"
```

### Conditional

```yaml
check:
  if:
    not: ${fetch.verified}
  then:
    return:
      error: "Email not verified"
```

### Switch (multi-arm conditional)

```yaml
classify:
  switch:
    - if: "$.spent >= 1000"
      return: "gold"
    - if: "$.spent >= 500"
      return: "silver"
    - default:
      return: "bronze"
```

### Return

```yaml
done:
  return:
    ok: null

fail:
  return:
    error: "Something went wrong"
```

### Match (discrete values)

```yaml
discount:
  match: ${level}
  cases:
    gold: 20
    silver: 10
    bronze: 5
```

### For loop

```yaml
process:
  for: c
  in: $.customers
  yield:
    level:
      call: classify
      args:
        spent: ${c.spent}
    discount:
      match: ${level}
      cases:
        gold: 20
        silver: 10
        bronze: 5
    result:
      name: ${c.name}
      level: ${level}
      discount: ${discount}
```

The last sub-step in `yield` (without `call`/`expr`) is the yielded object.

### Object merge

```yaml
done:
  return:
    ok:
      merge: $.order
      set:
        total: ${total}
        cost: ${ship}
```

## Complete Example

```yaml
name: notify
input:
  user-id: text
  message: text
output: result nil, text

steps:
  fetch:
    call: get-user
    args:
      user-id: $.user-id
    catch:
      return:
        error: "User lookup failed: ${error}"

  check:
    if:
      not: ${fetch.verified}
    then:
      return:
        error: "Email not verified"

  send:
    call: send-email
    args:
      to: ${fetch.email}
      subject: "Notification"
      body: $.message
    catch:
      return:
        error: "Send failed: ${error}"

  done:
    return:
      ok: null
```
