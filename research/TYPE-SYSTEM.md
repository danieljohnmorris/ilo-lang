# Type System Research

Design rationale and trade-offs for expanding ilo's type system (Phase E).

Evaluated against the manifesto: **total tokens from intent to working code**. Types exist to prevent retries — a type error caught at verify time saves an entire generation + execution + error-feedback cycle.

---

## What ilo has today

### 7 type constructors

| Syntax | Kind | Runtime value |
|--------|------|---------------|
| `n` | scalar | `f64` |
| `t` | scalar | `String` |
| `b` | scalar | `bool` |
| `_` | unit | `Nil` |
| `L n` | parameterized | `Vec<Value>` |
| `R n t` | parameterized | `Ok(Value)` / `Err(Value)` |
| `point` | named record | `Record { fields }` |

### What the verifier checks

26 error codes (ILO-T001 through ILO-T026) covering:

- Duplicate type/function definitions (T001, T002)
- Undefined types, variables, functions (T003-T005)
- Arity and argument type mismatches (T006, T007)
- Return type mismatches (T008)
- Operator type errors (T009-T013)
- Record field errors (T015-T022)
- List index errors (T023)
- Match exhaustiveness (T024)
- Auto-unwrap errors (T025, T026)

### Internal representation

AST types (`Type` enum) map to verifier types (`Ty` enum) with one addition: `Ty::Unknown` acts as a wildcard compatible with any type during inference. This lets the verifier proceed past unresolvable types without cascading errors.

### What's monomorphic

Everything. Every function has a fixed, concrete type signature. No type variables, no polymorphism, no generics. This is a deliberate trade-off:

- **Pro:** Verification is simple — direct structural equality. No unification, no constraint solving.
- **Pro:** Error messages are precise — "expected `n`, got `t`" not "failed to unify `a` with `b`".
- **Pro:** Zero runtime cost — types are erased after verification.
- **Con:** Can't write generic `map`, `filter`, `fold` — need per-type variants or untyped builtins.
- **Con:** Can't abstract over record types — no polymorphic tool handlers.

---

## Design principles for Phase E

### 1. Types prevent retries

Every type feature must reduce total token cost. The primary mechanism: catching errors at verify time instead of runtime. A type error caught before execution saves:

```
retry cost = generation tokens + execution tokens + error feedback tokens + re-generation tokens
           ≈ 100-200 tokens wasted
```

A type system that catches 1 more error class per program saves ~50-200 tokens per avoided retry.

### 2. Types cost tokens too

Type annotations are part of the program. Every type feature adds syntax that agents must generate:

```
f x:n>n                   # 3 type tokens
f x:R L n t>R L n t       # 9 type tokens (complex types)
f x:M t R L n t>O n       # 11 type tokens (with maps + optional)
```

A type system that's too expressive costs more tokens in annotations than it saves in prevented retries. The sweet spot: catch common errors with cheap annotations.

### 3. One way to do things

Each type constructor must have a single, obvious syntax. No aliases-by-convention, no implicit coercions, no type-level computation. The agent should never have to choose between equivalent type representations.

### 4. Familiar to LLMs

LLMs are trained on Rust, TypeScript, Python, Go, and Java types. ilo's type syntax should map cleanly to patterns LLMs already know:

| Concept | Rust | TypeScript | ilo |
|---------|------|------------|-----|
| Optional | `Option<T>` | `T \| null` | `O n` |
| Result | `Result<T, E>` | `{ok: T} \| {err: E}` | `R n t` |
| List | `Vec<T>` | `T[]` | `L n` |
| Map | `HashMap<K, V>` | `Record<K, V>` | `M t n` |
| Enum | `enum Status {...}` | `type Status = ...` | `enum status{...}` |

---

## Feature analysis

### E1. Type aliases

**What:** `alias res R n t` — name a complex type without creating a record.

**Token cost:** 3 tokens per alias declaration. 0 tokens per use (replaces the expanded form).

**Token savings:** Every use of `res` instead of `R n t` saves 2 tokens. A program with 5 uses saves 10 tokens minus the 3-token declaration = **7 net tokens saved**.

**Breakeven:** 2 uses of the alias (saves 4, costs 3).

**Implementation cost:** Low. Pure sugar — resolve during declaration collection, expand before body verification. No runtime changes, no new opcodes.

**Risk:** Alias cycles (`alias a b` + `alias b a`). Solved with cycle detection during resolution.

**Verdict:** High value, low cost. Do first.

### E2. Optional type

**What:** `O n` means "number or nil." The verifier forces you to handle nil before using the value.

**Problem it solves:** Currently, nil can appear at runtime in any position. A function returning a record might return nil if a tool fails silently. Field access on nil crashes at runtime with no type-level warning.

**Token cost of nil crash:**
```
runtime error + error message tokens + understanding tokens + fix tokens + retry
≈ 100-200 tokens wasted per crash
```

**Token cost of Optional:**
```
O n in signature: +1 token
?v{~x:use x;_:default}: +5 tokens for unwrap
Total: +6 tokens but prevents ~150-token retry
```

**Net savings:** ~144 tokens when it prevents even 1 nil crash.

**Design decision: `O n` vs `n?`**

| Option | Tokens | Consistency | Familiarity |
|--------|--------|-------------|-------------|
| `O n` | 1 (prefix) | Matches `L n`, `R n t` | Swift `Optional<Int>` |
| `n?` | 1 (postfix) | Breaks prefix pattern | TypeScript, Kotlin |

Recommend `O n` for consistency with other type constructors.

**Interaction with `!`:**
```
f! x    # on R return: propagate Err
f! x    # on O return: propagate nil (return nil from caller)
```

The `!` operator generalizes naturally: unwrap Ok from Result, unwrap Some from Optional. Both propagate the failure case.

**Interaction with match:**
```
?v{~x:use x;_:default}    # reuse ~ for Some, _ for None
```

Reuses existing match arm syntax. `~` means "has a value," `_` means "doesn't."

**Verdict:** High value. The nil crash is a real failure mode in tool-heavy code. Second priority after aliases.

### E3. Sum types / tagged unions

**What:** User-defined enums with variants, each optionally carrying data.

**Problem it solves:** Currently, the only way to represent alternatives is `R ok err` (2 variants) or text matching (unverified). A status field that's "pending," "active," or "closed" must be a text string — the verifier can't check exhaustive matching on text.

**Token cost of text-as-enum:**
```
?status{"pending":...;"active":...;"closed":...}    # no exhaustiveness check
# Agent forgets "closed" arm → runtime fall-through → silent bug → retry
```

**Token cost of enum:**
```
enum status{pending;active;closed}                   # declaration: 5 tokens
?s{pending:...;active:...;closed:...}                # exhaustiveness checked
```

**Key design question: syntax**

Option A — brace-delimited (consistent with records):
```
enum status{pending;active;closed}
enum shape{circle:n;rect:n n}           # variants with payloads
```

Option B — prefix S (consistent with L, R, O):
```
type status S pending active closed
```

Option A is clearer when variants carry data. Recommend Option A.

**Subsumption of Result:**

`R ok err` is a 2-variant enum: `enum R{ok:ok_type;err:err_type}`. Should R become sugar for enum?

- **Yes:** Simplifies the type system — one construct instead of two.
- **No:** R is so common (every tool call) that special syntax pays for itself.
- **Recommendation:** Keep R as special syntax, but compile internally to the same representation.

**Verdict:** Medium priority. Third in order.

### E4. Map type

**What:** `M t n` — dynamic key-value collection. Keys determined at runtime.

**Problem it solves:** Tool responses often contain variable-key objects (e.g., `{"us": 100, "uk": 50}` — counts by country). Currently typed as `t` (raw JSON string), losing all type information.

**Key design questions:**

Map literals — `M{"us":100;"uk":50}` (reuse `M` prefix to avoid ambiguity with record construction).

Map access — `get k m` returns `O v` (key might not exist). Note: `get` conflicts with D1b's HTTP `get`. Resolution options: `at k m` for map access, or type-dispatch (map vs url).

**Verdict:** Medium priority. Fourth in order.

### E5. Generic functions

**What:** Type variables in function signatures. `map f:fn(a>b) xs:L a > L b`.

**Problem it solves:** Without generics, list-processing builtins (`map`, `filter`, `fold`) are either untyped or duplicated per type.

**Token cost of no generics:**
```
s=0;@x xs{s=+s x};s          # 8 tokens every time for a sum
```

**Token cost with generics + fold builtin:**
```
fld + 0 xs                    # 4 tokens — saves 4
```

**Key design decisions:**

1. **Type variable syntax:** Single lowercase letters `a`, `b` in type position.
2. **Function type syntax:** `fn(n n>n)` for function types.
3. **Lambda syntax (prerequisite):** `\x>*x 2` (backslash) or `{x>*x 2}` (braces).
4. **Erasure vs monomorphization:** Erasure — ilo values are already boxed (`enum Value`).

**Prerequisites:** Lambda syntax, function values (`Value::FnRef`/`Value::Closure`), indirect calls (`OP_CALL_INDIRECT`).

**Verdict:** High value but high cost. Fifth priority.

### E6. Traits / interfaces

**What:** Shared behavior across record types.

**Reality check:** Agents generate concrete code for specific tasks. They don't write frameworks or abstract interfaces. Trait polymorphism is an abstraction tool — valuable for human programmers building reusable libraries, rarely needed when an agent generates a fresh program per task.

**Verdict:** Lowest priority. Defer until real use cases emerge. Gates on E5.

---

## Token budget analysis

How much do type annotations cost across a typical ilo program?

**Current (5-function tool orchestration):**
```
5 functions × ~3 params × 1 type token each = 15 type tokens
5 functions × 1 return type each = 5 type tokens
2 type declarations × ~3 fields = 6 type tokens
Total: ~26 type tokens out of ~150 program tokens = 17%
```

**With Phase E additions:**
```
+2 aliases = 6 tokens
+3 optional annotations = 3 tokens
Total: ~35 type tokens out of ~160 program tokens = 22%
```

The type tax goes from 17% to 22%. Acceptable if those extra 9 tokens prevent even one retry cycle (~100-200 tokens).

---

## Comparison with other minimal type systems

### Go (pre-generics)

Go survived 10+ years without generics. Its approach: interfaces for abstraction, concrete types everywhere else. ilo can follow a similar path — keep the type system simple, add generics only when the pain is real.

### Lua

No static types at all. Everything is a table. Works for scripting but causes retry-heavy generation when agents must discover types at runtime through errors.

### TypeScript (gradual typing)

Started untyped, added types gradually. The `any` type is an escape hatch. ilo's `Ty::Unknown` serves a similar role — compatible with everything, suppresses cascading errors.

### Elm

No runtime exceptions. Every failure is modeled in the type system (Result, Maybe). ilo's R and future O follow the same philosophy — make failure explicit and verifiable.

**ilo's position:** Closer to Elm than Go. Explicit failure types, exhaustive matching, verification before execution. But without generics (for now), closer to Go in expressiveness.

---

## Implementation ordering

```
E1 (aliases) ──────── pure sugar, no runtime, quick win
    ↓
E2 (optional) ─────── type-level only, catches nil bugs
    ↓
E3 (sum types) ────── new Decl + Value variant, medium effort
    ↓
E4 (maps) ─────────── new Value + opcodes, medium effort
    ↓
E5 (generics) ─────── lambda syntax, closures, indirect calls — large
    ↓
E6 (traits) ────────── gates on E5, lowest priority
```

Each step is independently valuable. E1-E2 are quick wins with high impact. E3-E4 expand expressiveness. E5-E6 are future territory.
