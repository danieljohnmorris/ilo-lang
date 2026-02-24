# Example Evaluation

Token counts using cl100k_base (Claude tokeniser family).

## Generation Tokens (code only, no comments)

| Example | ilo | Python | Ratio | Notes |
|---------|-----|--------|-------|-------|
| 01-simple-function | 84 | 73 | 1.15x | ilo includes inline property tests |
| 02-with-dependencies | 137 | 128 | 1.07x | ilo explicit `@` dependency graph |
| 03-data-transform | 234 | 217 | 1.08x | ilo type declarations + properties |
| 04-tool-interaction | 254 | 311 | **0.82x** | ilo beats Python — compact tool interface vs full HTTP impl |
| 05-workflow | 306 | 147 | 2.08x | ilo declares tool interfaces; Python assumes they exist |
| **Total** | **1015** | **876** | **1.16x** | |

### Terse syntax savings (vs verbose v1)

| Change | Tokens saved | Applies to |
|--------|-------------|------------|
| `fn`/`type`/`tool` (drop `define`) | 1 per declaration | All |
| Drop `body:` section marker | 2 per function | All |
| Indentation-based blocks (drop `end`) | 1 per block | All |
| `ok`/`err` (not `result.ok`/`result.error`) | 1 per use | 02, 04, 05 |
| `@` for dependency declarations | 1 per dep | 02, 03, 04, 05 |
| `->` for return type (drop `input:`/`output:`) | 3 per function | All |
| Symbol operators (`*`, `+`, `>=`) | 0 (same token count) | 01, 03 |
| `?` for property tests | 1 per test | 01, 03 |
| `nil` for void/nothing | 0 (same token count) | 04, 05 |
| **Total reduction** | **304 tokens (23%)** | |

## Where ilo Is Still Longer Than Python

The remaining ~16% overhead comes from features that reduce total cost:

1. **Named args at call sites** — `weight: order.total-weight` vs `order.total_weight`. Prevents positional swap errors.
2. **`@` dependency declarations** — explicit graph edges. Python `import` is terser but doesn't declare which functions are used.
3. **`?` property tests** — inline assertions bundled with the function. Python needs a separate test file.
4. **Tool declarations** (05) — ilo declares every tool's interface. Python assumes the agent already knows.

## Where ilo Beats Python

Example 04 (tool interaction) is **0.82x Python** — ilo's `tool` declaration is a compact interface contract while Python needs the full HTTP client implementation with try/catch.

## Total Cost Analysis

Raw generation is only one term:

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

| Factor | ilo advantage | Python cost |
|--------|--------------|-------------|
| **Spec loading** | Spec travels with the program | Agent needs Python in training or context |
| **Context loading** | `@` block tells agent what to load | Must trace imports across files |
| **Error feedback** | Closed world catches hallucinated APIs pre-execution | Runtime error after full execution |
| **Retries: wrong args** | Named args eliminate positional swap errors | ~10% swap rate on 3+ arg functions |
| **Retries: type errors** | Verified before execution | Runtime TypeError |
| **Retries: missing deps** | Verified before execution | ImportError |

### Retry cost estimate

If an agent writing Python swaps positional args once per 10 calls to 3+ arg functions, and each retry costs ~150 tokens (error message + regenerated function):

- Example 01: 2 multi-arg calls → 0.2 expected retries → +30 tokens
- Example 05: 3 tool calls → 0.3 expected retries → +45 tokens

For hallucinated APIs (agent invents a function that doesn't exist):
- Python: fails at runtime → ~200 tokens per retry
- ilo: caught by verifier before execution → ~50 tokens for the error + fix

## Cold-LLM Test

Gave Claude Haiku two ilo examples (01 and 04) with zero prior knowledge. Asked it to write a `validate-email` function.

**Result**: Haiku generated valid ilo. Correctly used `fn`, named args, `ok`/`err`, `?` property tests, `if...then`, and `match`. The only issue: it hallucinated a `contains` function — exactly the kind of error the closed-world verifier would catch before execution.

## Verdict

ilo costs ~1.16x Python to **generate** but carries context that reduces total cost:
1. Named args eliminate positional swap retries
2. Closed world catches hallucinated APIs before execution
3. Explicit dependency graph reduces context loading
4. Inline property tests bundle verification with the function

The generation overhead is ~140 tokens across all five examples. One prevented retry (~150 tokens) pays for it.

## Language-Agnostic Assessment

Current syntax uses short English keywords (`fn`, `type`, `tool`, `let`, `match`, `for`, `if`, `ok`, `err`). Most are 1-2 characters from being language-neutral (`fn` is already opaque to non-English speakers).

Symbols used: `@`, `->`, `?`, `*`, `+`, `-`, `/`, `>=`, `<=`, `==`, `!=`, `:`, `,`, `.`

The remaining English words that carry semantic weight: `match`, `let`, `for`, `if`, `from`, `self`, `true`, `false`, `not`, `and`, `or`, `in`, `with`, `log`.

Recommendation: these are short enough that they function as structural tokens rather than English words. An agent doesn't need to know English to learn that `let` introduces a binding. Revisit if empirical testing shows non-English-trained agents struggle.
