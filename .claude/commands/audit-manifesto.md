Compare the ilo manifesto against the actual codebase and site documentation for consistency.

## What to check

Read these three sources:
1. **Manifesto**: `/Users/dan/code/ilo-lang/ilo/MANIFESTO.md`
2. **Code**: The ilo codebase at `/Users/dan/code/ilo-lang/ilo/src/` — focus on `ast/mod.rs` (types), `parser/mod.rs` (syntax), `lexer/mod.rs` (tokens/keywords), `interpreter/mod.rs` (runtime), `vm/mod.rs` (VM), `main.rs` (CLI flags), `codegen/` (emitters)
3. **Site docs**: `/Users/dan/code/ilo-lang/site/src/content/docs/docs/` — all guide and reference pages

Also read:
- `SPEC.md` for the canonical language specification
- `README.md` for the project overview

## How to audit

Go through the manifesto **claim by claim**. For each substantive claim:

1. **Verify against code** — Is the claim implemented? Is it accurate?
2. **Verify against site docs** — Do the docs describe the same thing the manifesto claims?
3. **Check for contradictions** — Does the manifesto say X but the code/docs say Y?
4. **Check for gaps** — Are there features in code/docs not reflected in the manifesto?

## What to report

For each issue found, report:
- **Location**: Manifesto line/section, and the code/doc file that contradicts or is missing
- **Type**: `outdated` (was true, no longer), `aspirational` (not yet implemented), `inaccurate` (never true), `gap` (missing from manifesto)
- **Severity**: `high` (misleading), `medium` (incomplete), `low` (nitpick)
- **Suggested fix**: What to change, and where

## Output format

```
## Manifesto Audit

### Issues Found
[For each issue:]
- **[Section]** [Type/Severity] — description
  Manifesto: [quote]
  Reality: [what code/docs actually show]
  Fix: [suggested change]

### Verified Claims
[List of claims that checked out correctly]

### Summary
[Overall health: how aligned is the manifesto with reality?]
```

After presenting findings, ask whether to apply fixes.
