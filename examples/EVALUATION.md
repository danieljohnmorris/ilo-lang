# Example Evaluation

Token counts using cl100k_base (Claude tokeniser family).

## Raw Generation Tokens (code only, no comments)

| Example | ilo | Python | Ratio | Notes |
|---------|-----|--------|-------|-------|
| 01-simple-function | 104 | 73 | 1.42x | ilo includes inline property tests |
| 02-with-dependencies | 161 | 128 | 1.26x | ilo explicit `requires` block |
| 03-data-transform | 304 | 217 | 1.40x | ilo type declarations + properties |
| 04-tool-interaction | 359 | 313 | 1.15x | Closest — Python hides tool contract in impl |
| 05-workflow | 391 | 147 | 2.66x | ilo declares tool interfaces; Python assumes they exist |
| **Total** | **1319** | **878** | **1.50x** | |

## Analysis: Why ilo Is Longer

ilo programs carry context that Python pushes elsewhere:

1. **Tool declarations** (04, 05): ilo declares every external tool inline — name, types, timeout, retry. Python assumes the agent already knows the HTTP API.
2. **Inline property tests** (01, 03): ilo bundles examples with the function. Python needs a separate test file.
3. **Explicit dependency graph** (02, 05): `requires` block adds tokens but makes edges visible without parsing the body.
4. **Word operators** (`multiply`, `add` vs `*`, `+`): ~2x per expression. But symbols are ambiguous across contexts (`*` = multiply, pointer, glob, emphasis).
5. **Block delimiters** (`define function ... end`): More tokens than Python's `def ... :`+ indentation.

## The Real Metric: Total Tokens

Raw generation is only one term:

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

| Factor | ilo advantage | Python cost |
|--------|--------------|-------------|
| **Spec loading** | Spec can travel with the program (self-contained) | Agent must have Python knowledge in training or context |
| **Context loading** | `requires` block tells agent what to load | Agent must trace imports across files |
| **Error feedback** | Closed world catches hallucinated APIs before execution | Runtime error after full execution cycle |
| **Retries: wrong args** | Named args eliminate positional swap errors | Positional args cause ~10% swap rate on 3+ arg functions |
| **Retries: type errors** | Verified before execution | Runtime TypeError after execution |
| **Retries: missing deps** | Verified before execution | ImportError after execution |

### Retry cost estimate

If an agent writing Python swaps positional args once per 10 calls to 3+ arg functions, and each retry costs ~150 tokens (error message + regenerated function):

- Example 01: 2 multi-arg calls → 0.2 expected retries → +30 tokens
- Example 05: 3 tool calls → 0.3 expected retries → +45 tokens

For hallucinated APIs (agent invents a function that doesn't exist):
- Python: fails at runtime → ~200 tokens per retry
- ilo: caught by verifier before execution → ~50 tokens for the error + fix

## Verdict

ilo costs ~1.5x more tokens to **generate** but aims to cost less in **total** by:
1. Eliminating retry categories entirely (closed world, named args, pre-verification)
2. Reducing context loading (self-contained units, explicit dependency graph)
3. Bundling the spec with the program (no external knowledge required)

The generation overhead is the price of carrying context. Whether it pays for itself depends on the retry rate — which is an empirical question for the next phase.

## Syntax Observations

Things that work well:
- Hyphenated identifiers (`calculate-total`, `tax-rate`) — single token sequences, no camelCase splits
- Named args everywhere — verbose but eliminates a class of errors
- `result` as the only error mechanism — forces explicit handling
- `requires` block — graph edges are visible at a glance
- `define tool` — tools are first-class, not hidden in implementation

Things to reconsider:
- `define function` is 2 tokens where `fn` is 1. Consider shorter keywords.
- `body:` section marker adds tokens with no information (everything after `output` is body)
- `match ... on ... end` — 3 tokens of structure per match. Could `|` pattern arms be terser?
- `result.ok` / `result.error` — verbose compared to `ok`/`err`
- Word operators (`multiply`, `add`) — correct for disambiguation but expensive in arithmetic-heavy code

## Language-Agnostic Assessment

Current syntax uses English keywords (`define`, `function`, `input`, `output`, `body`, `end`, `match`, `return`, etc.). This violates principle 4 (language-agnostic).

Options:
1. **Keep English** — pragmatic. All major LLMs are heavily English-trained. The "agnostic" benefit is theoretical.
2. **Use symbols** — `→` for output, `::` for type annotation, `|` for match arms. Removes English but adds Unicode complexity.
3. **Constructed tokens** — short, meaningless strings (`fn`, `io`, `tp`) that are equally foreign to all languages.
4. **Hybrid** — structural markers as symbols, domain terms as English words.

Recommendation: revisit after empirical testing. If English keywords don't measurably increase retries for non-English-trained agents, keep them. Principle 1 (token-conservative) trumps principle 4 if they conflict.
