Test whether Haiku can correctly generate ilo code for every syntax construct.

## How it works

1. Spawn **6 haiku agents in parallel**, one per category
2. Each agent reads the ilo spec, generates code for each task, runs it through `ilo`, and reports pass/fail
3. Collect results and display a summary table

## Agent instructions

Each agent gets a batch of tasks. For every task the agent must:

1. Run `~/.cargo/bin/ilo -ai` to read the full language spec
2. Generate ilo code that solves the task (one line per function, no comments)
3. Run it through `~/.cargo/bin/ilo '<code>' <func> <args>` and check stdout matches expected output
4. Report each task as PASS or FAIL with the generated code

**Important rules for agents:**
- Generate code yourself — you ARE the LLM being tested
- Do NOT read example files or look at existing code for answers
- Test by running `ilo` via Bash, check stdout matches expected exactly
- If a test fails, do NOT retry or fix — just report FAIL with the code you generated and the actual output
- Report results in this exact format for each task:
  ```
  [PASS] task_id: <code>
  [FAIL] task_id: <code> | expected=X got=Y
  ```

## Categories and tasks

### Agent 1: Arithmetic & Guards
```
add_mul:     f x:n y:n>n — compute x*y + x (bind-first). Test: f 3 4 → 15
negate:      f x:n>n — negate x using "- 0 x". Test: f 5 → -5
mod_op:      f x:n>n — return mod x 3, bind r then +r 0. Test: f 7 → 1, f 9 → 0
grade:       grd m:n>t — braceless guards: >=m 90 "A", >=m 80 "B", >=m 70 "C", >=m 60 "D", else "F". Test: grd 95 → A, grd 85 → B, grd 50 → F
clamp:       cl v:n lo:n hi:n>n — braceless guard <v lo lo, >v hi hi, else +v 0. Test: cl 5 0 10 → 5, cl -1 0 10 → 0, cl 15 0 10 → 10
sign:        sg n:n>t — braceless: =n 0 "zero", >n 0 "pos", else "neg". Test: sg 0 → zero, sg 5 → pos, sg -3 → neg
eq_prefix:   f a:n b:n>b — test equality with prefix = operator: =a b. Test: f 3 3 → true, f 3 4 → false
```

### Agent 2: Braced Conditionals & Loops
```
count_eq:    cnt xs:L n v:n>n — c=0, @x xs{=x v{c=+c 1}}, +c 0. Test: cnt 1,2,3,2,2 2 → 3, cnt 1,2,3 4 → 0
find_max:    mx xs:L n>n — m=xs.0, @x xs{>x m{m=x}}, +m 0. Test: mx 3,1,4,1,5 → 5
sum_pos:     sp xs:L n>n — s=0, @x xs{>x 0{s=+s x}}, +s 0. Test: sp 1,-2,3,-4,5 → 9, sp -1,-2,-3 → 0
sum_list:    f xs:L n>n — s=0, @x xs{s=+s x}, +s 0. Test: f 1,2,3,4,5 → 15
double_list: f xs:L n>L n — r=[], @x xs{r=+=r *x 2}, return r. Test: f 1,2,3 → [2, 4, 6]
filter_pos:  f xs:L n>L n — r=[], @x xs{>x 0{r=+=r x}}, return r. Test: f 1,-2,3,-4,5 → [1, 3, 5]
```

### Agent 3: Ternary, Match & Prefix Ternary
```
ternary_val:   f x:n>t — return >x 0{"pos"}{"neg"}. Test: f 5 → pos, f -3 → neg
ternary_let:   f x:n>n — v=<x 0{- 0 x}{x}, +v 0 (abs via ternary). Test: f -5 → 5, f 3 → 3
prefix_tern:   f x:n>n — ?=x 0 1 0. Test: f 0 → 1, f 5 → 0
match_lit:     f x:n>t — ?x{1:"one";2:"two";3:"three";_:"other"}. Test: f 1 → one, f 5 → other
match_result:  Two fns. chk x:n>R n t;<x 0 ^"neg";~x. Then f x:n>t;r=chk x;?r{~v:str v;^e:e}. Test: f 5 → 5, f -1 → neg
negated_guard: f x:n>t — !>x 0 "non-positive", else "positive". Test: f 5 → positive, f 0 → non-positive
```

### Agent 4: Range, While, Break, Continue
```
range_sum:    f n:n>n — s=0, @i 0..n{s=+s i}, +s 0. Test: f 5 → 10, f 10 → 45
while_count:  f n:n>n — i=0,s=0, wh <i n{i=+i 1;s=+s i}, +s 0. Test: f 5 → 15, f 1 → 1
range_squares: f n:n>L n — r=[], @i 0..n{r=+=r *i i}, return r. Test: f 4 → [0, 1, 4, 9]
break_loop:   f>n — i=0, wh true{i=+i 1;>=i 5{brk}}, +i 0. Test: f → 5
continue_loop: f xs:L n>n — sum elements <10: s=0, @x xs{>=x 10{cnt};s=+s x}, +s 0. Test: f 1,10,2,20,3 → 6
loop_ret:     f xs:L n th:n>n — @x xs{>=x th{ret x}}, return -1. Test: f 1,5,3,7,2 6 → 7, f 1,2,3 10 → -1
```

### Agent 5: Builtins, Maps & Records
```
list_rev:     f xs:L n>L n — rev xs. Test: f 1,2,3 → [3, 2, 1]
list_sort:    f xs:L n>L n — srt xs. Test: f 3,1,4,1,5 → [1, 1, 3, 4, 5]
list_sum:     f xs:L n>n — r=sum xs, +r 0. Test: f 1,2,3,4 → 10
str_fmt:      f n:n>t — fmt "val={}" n. Test: f 5 → val=5
map_build:    f>n — m=mmap, m=mset m "a" 10, m=mset m "b" 20, mget m "a". Test: f → 10
record_basic: type pt{x:n;y:n} then f a:n b:n>n;p=pt x:a y:b;+p.x p.y. Test: f 3 4 → 7
```

### Agent 6: Recursion, Error Handling & Advanced
```
factorial:    fac n:n>n — guard <=n 1 1, r=fac -n 1, *n r. Test: fac 5 → 120
fibonacci:    fib n:n>n — guard <=n 1 n, a=fib -n 1, b=fib -n 2, +a b. Test: fib 6 → 8, fib 10 → 55
power:        pw b:n e:n>n — guard <=e 0 1, p= -e 1, r=pw b p, *b r. Test: pw 2 3 → 8
result_ret:   f x:n>R n t — guard <=x 0 ^"must be positive", ~*x 2. Test: f 5 → ~10, f -1 → ^must be positive
multi_fn:     dbl x:n>n;*x 2 (newline) f x:n>n;r=dbl x;+r 1. Test: f 3 → 7
min_max_range: f xs:L n>n — mn=xs.0, mx=xs.0, @x xs{<x mn{mn=x};>x mx{mx=x}}, -mx mn. Test: f 3,1,4,1,5 → 4
```

## After all agents complete

Display a summary table:

```
Category                         Pass/Total
─────────────────────────────────────────────
Arithmetic & Guards              X/7
Braced Conditionals & Loops      X/6
Ternary, Match & Prefix          X/6
Range, While, Break, Continue    X/6
Builtins, Maps & Records         X/6
Recursion, Error & Advanced      X/6
─────────────────────────────────────────────
OVERALL                          X/37
```

For any FAILs, show the generated code, expected vs actual output, and a brief analysis of what went wrong (spec gap, parser bug, or genuine comprehension failure).
