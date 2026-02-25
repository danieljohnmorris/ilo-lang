# Idea 9: Short Names

idea8 syntax + convention of short variable names (1-3 chars). Same language, different style guide.

## Naming Convention

| Long | Short | Rule |
|------|-------|------|
| `order` | `ord` | truncate to 2-3 chars |
| `customers` | `cs` | first + last consonant |
| `data` | `d` | single letter when unambiguous |
| `level` | `lv` | drop vowels |
| `discount` | `dc` | initials or abbreviation |
| `shipped` | `sh` | first 2 chars |
| `final` | `fin` | first 3 chars |
| `spent` | `sp` | first 2 chars |
| `items` | `its` | first 3 chars |

Param names in function signatures stay short too. Field names in constructors (`name:`, `level:`, `oid:`) keep their original names since they define the output schema.

## Syntax

Same as idea8 — see idea8-ultra-dense/SPEC.md for full syntax reference.

Key features:
- `>` return type, no parens: `fn p:n q:n>n`
- 1-char types: `n` `t` `b` `_` `L` `R`
- Positional calls: `charge pid amt`
- Implicit match: `call val;?{!e:...;~v:...}`
- `@` iterate, no `if` keyword, `!`/`~` construct err/ok
- `+` for string concat

## Complete Example

```
notify uid:t msg:t>R _ t;get-user uid;?{!e:!+"Lookup failed: "e;~d:!d.verified{!"Email not verified"};send-email d.email "Notification" msg;?{!e:!+"Send failed: "e;~_:~_}}
```
