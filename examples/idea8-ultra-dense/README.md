# Idea 8: Ultra-Dense

Pushes idea7's dense wire format even further — every keyword and type compressed to minimum characters.

Key changes: `>` return type, `n/t/b/_` types, `L`/`R` composite types, `?`/`!`/`~` match/err/ok, `@` iteration, drop `if` keyword, `+` for string concat, `!` for negation.

Target: < 0.35x tokens vs Python baseline.
