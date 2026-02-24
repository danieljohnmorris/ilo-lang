# idea1 Evaluation

Token counts for all ideas: run `python3 examples/compare.py` from the repo root.

## Where ilo Beats Python

Tool interaction and error handling. ilo's `tool` declaration is a compact interface contract while Python needs the full HTTP client with try/catch. The `match`/`ok`/`err` pattern is terser than Python's `isinstance`/`.value`/`.error`.

## Where Python Wins

Simple math. The BPE tokeniser is trained on Python, so `sub = price * quantity` tokenises more efficiently than `let sub = * price quantity`. Novel syntax can never fully overcome tokeniser bias.

## Total Cost Analysis

Raw generation is only one term:

```
Total cost = spec loading + generation + context loading + error feedback + retries
```

| Factor | ilo advantage | Python cost |
|--------|--------------|-------------|
| Spec loading | Spec travels with the program | Agent needs Python in training or context |
| Context loading | `@` block tells agent what to load | Must trace imports across files |
| Error feedback | Closed world catches hallucinated APIs pre-execution | Runtime error after full execution |
| Retries: wrong args | Named args eliminate positional swap errors | ~10% swap rate on 3+ arg functions |
| Retries: type errors | Verified before execution | Runtime TypeError |
| Retries: missing deps | Verified before execution | ImportError |

## Cold-LLM Test

Gave Claude Haiku two ilo examples (01 and 04) with zero prior knowledge. Asked it to write a `validate-email` function.

**Result**: Haiku generated valid ilo. Correctly used `fn`, named args, `ok`/`err`, `?` property tests, `if`, and `match`. The only issue: it hallucinated a `contains` function — exactly the kind of error the closed-world verifier would catch.

## Language-Agnostic Assessment

Current syntax uses short English keywords (`fn`, `type`, `tool`, `let`, `match`, `for`, `if`, `ok`, `err`). Most function as structural tokens rather than English words. An agent learns that `let` introduces a binding from examples, not from understanding English.

Symbols used: `@`, `->`, `?`, `*`, `+`, `-`, `/`, `>=`, `<=`, `==`, `!=`, `:`, `,`, `.`
