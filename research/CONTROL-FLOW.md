# Control Flow Research

How terse languages handle control flow — patterns, trade-offs, and what ilo should steal.

Evaluated against the manifesto: **total tokens from intent to working code**. A feature is worth adding only if it reduces total token cost across generation + retries + context loading + error feedback.

---

## What ilo has today

| Form | Tokens | What it does |
|------|--------|--------------|
| `cond{body}` | 1 sigil | Guard: return body if cond true |
| `!cond{body}` | 1 sigil | Guard: return body if cond false |
| `?x{arms}` | 1 sigil | Match named value |
| `?{arms}` | 1 sigil | Match last result (implicit) |
| `@v list{body}` | 1 sigil | Iterate list, accumulate |
| `~expr` | 1 sigil | Return Ok |
| `^expr` | 1 sigil | Return Err |
| `func! args` | 1 sigil | Call + auto-unwrap Result |
| `&a b` | 1 sigil | Short-circuit AND |
| `\|a b` | 1 sigil | Short-circuit OR |

This is already terse. The question is: what common patterns still cost too many tokens?

---

## Lessons from other languages

### Perl — postfix conditionals and implicit variables

**Postfix if/unless:**
```perl
print "yes" if $x > 10;      # condition after action
die "bad"  unless $valid;     # negated postfix
```

Token savings: eliminates braces entirely for single-expression bodies. The condition is a modifier, not a block.

**ilo equivalent today:**
```
>x 10{"yes"}     # 1 guard — already terse
```

**Verdict:** ilo guards are already 1 sigil + braces. Postfix conditionals would save the `{}` (2 tokens) but add parsing ambiguity. Not worth it — the guard form is already minimal.

**`$_` implicit variable (topic variable):**
```perl
for (@items) { print $_ }     # $_ is the implicit loop variable
grep { $_ > 5 } @items;       # $_ used in filter predicate
map { $_ * 2 } @items;        # $_ used in transform
```

Token savings: eliminates naming the loop variable when it's only used once.

**ilo equivalent today:**
```
@x xs{+x 1}     # must name x even for single use
```

**Verdict:** High value for ilo. A topic variable (e.g., `_` or implicit) in `@` loops and future `map`/`flt` would save 1 token per loop. But conflicts with `_` as nil/wildcard. See F3 below.

**`//` defined-or operator:**
```perl
my $name = $input // "default";   # use $input unless undef
```

Token savings: 1 operator vs a full match with nil arm. Equivalent to Kotlin's `?:`, Ruby's `||`, Swift's `??`.

**ilo equivalent today:**
```
?x{_:"default";~v:v}     # 7+ tokens for a nil-or-default
```

**Verdict:** Very high value. See F5 (nil-coalescing).

---

### Ruby — chaining, safe navigation, compact blocks

**Safe navigation `&.`:**
```ruby
user&.address&.city     # returns nil if any link is nil
```

Token savings: eliminates nested nil checks. Each `&.` replaces a full `if x.nil?` guard.

**ilo equivalent today:**
```
# Must guard at each step:
?u{_:_;~u:?u.addr{_:_;~a:a.city}}    # deeply nested, many tokens
```

**Verdict:** Very high value for tool-heavy code where results may be nil. See F6 (safe navigation).

**`||=` assign-if-nil:**
```ruby
@cache ||= expensive_call    # only compute if nil
```

**ilo equivalent today:** No direct equivalent — requires guard + let.

**Verdict:** Low priority. Agents generate fresh code, rarely need memoization.

**Method chaining with blocks:**
```ruby
items.select { |x| x > 5 }.map { |x| x * 2 }.sum
```

Token savings: eliminates intermediate variables entirely. Each step flows into the next.

**ilo equivalent today:**
```
a=@x items{>x 5{x}};b=@x a{*x 2};sum b    # 3 statements, 2 intermediate vars
```

**Verdict:** High value — but requires generics (E5) and lambdas first. The pipe operator (F8) partially addresses this.

---

### Bash — pipes and short-circuit control flow

**Pipes `|`:**
```bash
cat file | grep error | wc -l
```

The pipe model: each stage's output feeds the next stage's input. No intermediate variables. Linear, left-to-right data flow.

**`&&`/`||` as control flow:**
```bash
test -f file && echo "exists" || echo "missing"
```

Token savings: ternary-like branching in 1 line, no `if`/`then`/`fi`.

**ilo equivalent today:**
```
# Already has & and | operators, but they return values, not control flow
# Guard chaining works similarly:
=f file{"exists"};"missing"    # but guards return early, breaking the chain
```

**Verdict:** ilo's `&`/`|` already short-circuit. The missing piece is using them as *expression-level* ternary. See F4 (ternary expression).

---

### APL/J/K — implicit mapping, reduction operators

**Each operator (K):**
```k
2*                     / multiply each by 2 — applied to whole list implicitly
+/                     / reduce with addition
```

**Reduce `/`:**
```apl
+/ 1 2 3 4             ⍝ → 10 (sum)
×/ 1 2 3 4             ⍝ → 24 (product)
```

Token savings: 1 character for reduce. No loop, no accumulator, no lambda.

**Scan `\`:**
```apl
+\ 1 2 3 4             ⍝ → 1 3 6 10 (running sum)
```

**ilo equivalent today:**
```
# No reduce — must use @:
s=0;@x xs{s=+s x};s    # 8 tokens for a sum
```

**Verdict:** Very high value. Reduce is the most common list operation after iteration. A 1-token reduce operator would save 6+ tokens per use. But needs generics (E5). See F9 (reduce operator).

---

### Haskell — composition, guards in function heads, `$` application

**Guards in function definitions:**
```haskell
classify spend
  | spend >= 1000 = "gold"
  | spend >= 500  = "silver"
  | otherwise     = "bronze"
```

**ilo equivalent today:**
```
cls sp:n>t;>=sp 1000{"gold"};>=sp 500{"silver"};"bronze"
```

**Verdict:** ilo's guard syntax already mirrors Haskell guards closely. Already good.

**`$` application (eliminate parens):**
```haskell
f $ g $ h x          -- f(g(h(x)))
```

**`.` composition:**
```haskell
(f . g . h) x        -- same thing, reusable
```

**ilo equivalent today:**
```
a=h x;b=g a;f b      # 3 binds — verbose
```

**Verdict:** Composition/application operators would help but conflict with prefix notation. With prefix, `f (g x)` is already the natural nesting form. The bind-first pattern is verbose but clear. Pipe (F8) is the pragmatic answer.

---

### Elixir — pipe operator `|>` and `with` for happy paths

**Pipe `|>`:**
```elixir
"hello" |> String.upcase() |> String.reverse()
```

Token savings: eliminates intermediate variables for linear chains. Each step flows left-to-right.

**`with` for happy-path chaining:**
```elixir
with {:ok, user} <- get_user(id),
     {:ok, profile} <- get_profile(user),
     {:ok, _} <- send_email(profile) do
  :ok
else
  {:error, reason} -> {:error, reason}
end
```

Token savings: chains multiple fallible operations with a single error clause. Equivalent to Rust's `?` but with explicit else.

**ilo equivalent today:**
```
# Auto-unwrap ! already handles this:
u=get-user! id;p=get-profile! u;send-email! p
```

**Verdict:** ilo's `!` operator already does what Elixir's `with` does — chain fallible calls with automatic error propagation. This is a win ilo already has.

The pipe operator itself is still valuable for non-Result chains. See F8.

---

### Rust — `?` operator, method chaining, `if let`

**`?` operator:**
```rust
let user = get_user(id)?;
let profile = get_profile(&user)?;
```

**ilo equivalent: `!` — already implemented.** Same semantics.

**Iterator chains:**
```rust
items.iter().filter(|x| x > 5).map(|x| x * 2).sum()
```

Token savings: no intermediate variables, lazy evaluation, composable.

**`if let` (partial match):**
```rust
if let Some(x) = maybe_value {
    use(x);
}
```

Token savings: combine match + guard into one form. Only handles one variant.

**ilo equivalent today:**
```
?v{~x:use x}    # match with one arm — similar but always needs ?{}
```

**Verdict:** `if let` maps to single-arm match. ilo's `?v{~x:use x}` is already close. Not worth a new construct.

---

### Awk — implicit loops and pattern-action

**Pattern-action model:**
```awk
/error/ { count++ }         # for every line matching /error/, increment
NR > 10 { print $0 }       # for lines after 10, print
END { print count }         # after all input, print count
```

Token savings: the loop is implicit — awk iterates over input automatically. Pattern-action pairs are concise conditional guards applied per-record.

**Verdict:** Interesting model for tool-output processing, but too domain-specific for a general language. The `@` loop + guard pattern covers this.

---

### Forth/Factor — stack combinators

**Stack-based control:**
```forth
: abs dup 0 < if negate then ;
```

**Factor combinators:**
```factor
{ 1 2 3 } [ 2 * ] map         ! quotation (anonymous function) as argument
5 [ even? ] [ 2 / ] [ 1 + ] if  ! conditional with quotations
```

Token savings: no variable names — values flow through the stack. Combinators (`bi`, `tri`, `cleave`) apply multiple operations to the same value without naming it.

**Verdict:** Stack-based is fundamentally different from ilo's named-variable model. However, the *idea* of applying multiple operations to the same value without re-naming is valuable. Destructuring (F7) and topic variables address this partially.

---

## Synthesis: what ilo should add

Ranked by **token savings × frequency of use**:

| Priority | Feature | Token savings | Frequency | Source |
|----------|---------|---------------|-----------|--------|
| **1** | Ternary expression | 3-5 per use | Very high | Bash, Perl, Ruby, C |
| **2** | Nil-coalescing `??` | 5-7 per use | High (tool results) | Perl `//`, Ruby `\|\|`, Kotlin `?:` |
| **3** | Safe navigation `.?` | 5-10 per chain | High (nested records) | Ruby `&.`, Kotlin `?.`, TS `?.` |
| **4** | Pipe operator | 2-3 per step | Medium | Elixir `\|>`, Bash `\|`, F# |
| **5** | Early return | 2-4 per use | Medium | Rust, most languages |
| **6** | While loop | N/A (new capability) | Low-medium | Perl, Ruby, Bash |
| **7** | Destructuring bind | 2-3 per record | Medium | Rust, JS, Elixir |
| **8** | Range iteration | 3-5 per range loop | Medium | Ruby `0..n`, Python `range()` |
| **9** | Break/continue | 2-3 per use | Low | Most languages |
| **10** | Reduce operator | 5-7 per use | Medium | APL `/`, K, Haskell `fold` |
| **11** | Guard else | 1-2 per use | Low | Haskell, Elixir |
| **12** | Type pattern match | N/A (new capability) | Low | Haskell, Scala |

---

## Proposed syntax for each feature

### F1. Ternary expression — `cond?then:else`

**Problem:** Guard returns from the function. There's no expression-level conditional that doesn't return.

Current workaround:
```
# Use match as expression:
r=?{=x 1:a;_:b}    # 8 tokens
```

**Proposed:** Reuse existing `?` with inline syntax:
```
r==x 1?a:b          # 5 tokens — saves 3
```

Or with prefix condition:
```
r=?=x 1{a}{b}       # ternary with two brace bodies
```

**Inspiration:** C/JS ternary, Perl `$x ? "a" : "b"`, Ruby `x > 0 ? "pos" : "neg"`.

### F2. While loop — `wh cond{body}`

**Problem:** `@` only iterates lists. Polling, convergence, and stateful loops need while.

```
wh >x 0{x=-x 1}     # while x > 0, decrement
```

**2 tokens** for the `wh` keyword + condition + body. Consistent with guard syntax (`cond{body}`).

### F3. Range iteration — `@i 0..n{body}`

**Problem:** Index loops require constructing a list first.

```
@i 0..10{*i i}       # i = 0, 1, 2, ..., 9
@i 0..len xs{xs.i}   # index-based iteration
```

The `..` operator produces a lazy range, no list allocation. Only valid in `@` context.

**Inspiration:** Ruby `0..9`, Rust `0..10`, Python `range(10)`, Kotlin `0 until 10`.

### F4. Nil-coalescing — `a??b`

**Problem:** Handling optional/nil values requires a full match.

Current:
```
?v{_:"default";~x:x}    # 7 tokens
```

Proposed:
```
v??"default"              # 2 tokens — saves 5
```

Evaluates left side. If nil, evaluates right side. Otherwise returns left side.

**Inspiration:** Perl `//`, C# `??`, Kotlin `?:`, Swift `??`, JS `??`.

### F5. Safe navigation — `.?`

**Problem:** Chained field access on possibly-nil values requires nested matches.

Current:
```
?u{_:_;~u:?u.addr{_:_;~a:a.city}}    # deeply nested
```

Proposed:
```
u.?addr.?city            # 3 tokens — returns nil if any step is nil
```

Short-circuits at first nil, returns nil. No match needed.

**Inspiration:** Ruby `&.`, Kotlin `?.`, TypeScript `?.`, C# `?.`.

### F6. Early return — `ret expr`

**Problem:** Can only return from guards or as the last expression. Complex functions sometimes need to exit from the middle.

```
f x:n>n;v=compute x;ret v;cleanup    # cleanup never runs — ret exits
```

**3 tokens** (keyword + space + expr). Rare in ilo style but needed for complex tool orchestration.

### F7. Destructuring bind — `{a;b}=expr`

**Problem:** Extracting multiple fields requires separate statements.

Current:
```
r=get-user! id;n=r.name;e=r.email    # 3 statements, 9 tokens
```

Proposed:
```
{n;e}=get-user! id                   # 1 statement — fields by name
```

Binds `n` to `result.n` and `e` to `result.e` using field name matching. Short field names (ilo convention) make this natural.

**Inspiration:** JS `const {name, email} = obj`, Rust `let Point {x, y} = p`, Elixir `%{name: n} = user`.

### F8. Pipe operator — `>>`

**Problem:** Linear chains of calls require intermediate variables.

Current:
```
a=f x;b=g a;h b       # 3 binds
```

Proposed:
```
f x>>g>>h              # 0 binds, left-to-right flow
```

`>>` passes the result of the left side as the **last** argument of the right side. 2 chars, 1 token.

Why `>>` not `|>`:
- `|` is already logical OR
- `>>` is visually directional (data flows right)
- 1 token in most LLM tokenizers
- Familiar from Haskell (`>>` is monadic sequencing)

**Tension with prefix notation:** Pipe is inherently infix/postfix — it reverses the prefix order. This is fine for *call chains* (which are already left-to-right in ilo: `a=f x;b=g a`). The pipe just drops the bind.

**Inspiration:** Elixir `|>`, F# `|>`, Bash `|`, Haskell `>>`, Unix pipes.

### F9. Break/continue — `brk` / `cnt`

**Problem:** No way to exit a loop early or skip an iteration.

```
@x xs{=x 0{cnt};>x 100{brk};process x}
```

`brk` exits the loop immediately, returning the last accumulated value.
`cnt` skips to the next iteration.

**Inspiration:** C `break`/`continue`, Rust `break`/`continue`, Perl `last`/`next`.

### F10. Guard else — `cond{then}{else}`

**Problem:** Guards return from the function. Sometimes you want if/else *within* a function without returning.

Current workaround:
```
?{=x 1:a;_:b}       # match as if/else — works but clunky
```

Proposed:
```
=x 1{a}{b}           # guard with else block — doesn't return from function
```

Two adjacent brace bodies: first if true, second if false. Same syntax as guard but with an else block.

**Verdict:** Lower priority — match already covers this. But 2 fewer tokens than the match form.

### F11. Reduce operator

Needs generics (E5). Proposed as a builtin `fld` rather than a syntax operator.

```
fld + 0 xs           # fold with +, init 0 — sum
fld * 1 xs           # fold with *, init 1 — product
```

**Inspiration:** APL `+/`, Haskell `foldl`, K `/`, Elixir `Enum.reduce`.

### F12. Type pattern matching

Extends `?` to match on runtime type.

```
?x{n v:*v 2; t v:+v "!"; _:"unknown"}
```

Needed when tools return `t` (raw JSON) as escape hatch and the program must dispatch on actual shape.

---

## Implementation ordering

```
F4 (ternary) ─────────┐
F5 (nil-coalesce) ─────┤
F6 (safe navigation) ──┤── no dependencies, pure syntax + verifier
F1 (early return) ─────┤
F10 (guard else) ──────┘

F2 (while) ────────────┐
F3 (range) ────────────┤── new VM loops
F9 (break/continue) ───┘

F7 (destructuring) ────── needs record type awareness
F8 (pipe) ─────────────── needs call rewriting
F12 (type match) ──────── needs runtime type tags

F11 (reduce) ──────────── gates on E5 (generics)
```

The first group (expression-level features) are highest value and lowest implementation cost. They add no new opcodes — just parser productions and verifier rules.
