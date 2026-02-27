#!/usr/bin/env python3
"""Prefix vs infix notation: token and character comparison.

Measures actual savings from ilo's prefix notation against equivalent
infix (Python-style) expressions using the cl100k_base tokenizer.

Usage:
    python3 research/explorations/prefix-vs-infix/bench.py

Requires: pip install tiktoken
"""

import sys

try:
    import tiktoken
except ImportError:
    print("Install tiktoken: pip install tiktoken")
    sys.exit(1)

ENC = tiktoken.get_encoding("cl100k_base")


def tokens(text: str) -> int:
    return len(ENC.encode(text))


def token_list(text: str) -> list[str]:
    return [ENC.decode([t]) for t in ENC.encode(text)]


# ── Expression pairs: (description, infix, prefix) ──────────────────

PAIRS = [
    # Simple binary
    ("add", "a + b", "+a b"),
    ("subtract", "a - b", "-a b"),
    ("multiply", "a * b", "*a b"),
    ("divide", "a / b", "/a b"),
    ("equal", "a == b", "=a b"),
    ("not equal", "a != b", "!=a b"),
    ("greater", "a > b", ">a b"),
    ("less", "a < b", "<a b"),
    ("greater eq", "a >= b", ">=a b"),
    ("less eq", "a <= b", "<=a b"),

    # Nested (2 levels)
    ("(a*b)+c", "(a * b) + c", "+*a b c"),
    ("a*(b+c)", "a * (b + c)", "*a +b c"),
    ("(a+b)>=100", "(a + b) >= 100", ">=+a b 100"),
    ("(a*b)-(c*d)", "(a * b) - (c * d)", "-*a b *c d"),

    # Nested (3 levels)
    ("((a+b)*c)>=100", "((a + b) * c) >= 100", ">=*+a b c 100"),
    ("(a*(b+c))-d", "(a * (b + c)) - d", "-*a +b c d"),

    # Logical
    ("and", "a and b", "&a b"),
    ("or", "a or b", "|a b"),
    ("not", "not x", "!x"),
    ("not(a==b)", "not (a == b)", "!=a b"),
    ("x>=0 and x<=100", "x >= 0 and x <= 100", "&>=x 0 <=x 100"),

    # Real-world expressions from ilo programs
    ("total fn", "sub = price * qty; tax = sub * rate; return sub + tax",
     "s=*p q;t=*s r;+s t"),
    ("guard", "if score >= 1000: return \"gold\"",
     ">=sp 1000{\"gold\"}"),
    ("guard+not", "if not d.verified: return err(\"not verified\")",
     "!d.verified{^\"not verified\"}"),
    ("chained guards",
     "if score < 500: return err(\"too low\")\nratio = debt / income\nif ratio > 0.4: return err(\"too high\")",
     "<sc 500{^\"too low\"};r=/dbt inc;>r 0.4{^\"too high\"}"),
]

# ── Run comparison ───────────────────────────────────────────────────


def main():
    print("=" * 78)
    print("Prefix vs Infix: Token & Character Comparison (cl100k_base)")
    print("=" * 78)

    total_infix_tok = 0
    total_prefix_tok = 0
    total_infix_chr = 0
    total_prefix_chr = 0

    print(f"\n  {'Pattern':<22s}  {'Infix':>6s}  {'Prefix':>6s}  {'Saved':>6s}  "
          f"{'Infix':>6s}  {'Prefix':>6s}  {'Saved':>6s}")
    print(f"  {'':22s}  {'tok':>6s}  {'tok':>6s}  {'tok':>6s}  "
          f"{'chr':>6s}  {'chr':>6s}  {'chr':>6s}")
    print(f"  {'-' * 72}")

    for desc, infix, prefix in PAIRS:
        i_tok = tokens(infix)
        p_tok = tokens(prefix)
        i_chr = len(infix)
        p_chr = len(prefix)
        tok_saved = i_tok - p_tok
        chr_saved = i_chr - p_chr

        total_infix_tok += i_tok
        total_prefix_tok += p_tok
        total_infix_chr += i_chr
        total_prefix_chr += p_chr

        tok_sign = f"+{tok_saved}" if tok_saved > 0 else str(tok_saved)
        chr_sign = f"+{chr_saved}" if chr_saved > 0 else str(chr_saved)

        print(f"  {desc:<22s}  {i_tok:>6d}  {p_tok:>6d}  {tok_sign:>6s}  "
              f"{i_chr:>6d}  {p_chr:>6d}  {chr_sign:>6s}")

    print(f"  {'-' * 72}")
    tok_saved = total_infix_tok - total_prefix_tok
    chr_saved = total_infix_chr - total_prefix_chr
    tok_pct = (1 - total_prefix_tok / total_infix_tok) * 100 if total_infix_tok else 0
    chr_pct = (1 - total_prefix_chr / total_infix_chr) * 100 if total_infix_chr else 0

    print(f"  {'TOTAL':<22s}  {total_infix_tok:>6d}  {total_prefix_tok:>6d}  "
          f"+{tok_saved:>5d}  {total_infix_chr:>6d}  {total_prefix_chr:>6d}  +{chr_saved:>5d}")
    print(f"  {'SAVINGS':<22s}  {tok_pct:>13.1f}%  {'':>6s}  {chr_pct:>13.1f}%")

    # Token breakdown for interesting cases
    print(f"\n\n{'=' * 78}")
    print("Token breakdown (how the tokenizer sees each expression)")
    print(f"{'=' * 78}\n")

    highlights = [
        ("(a*b)+c", "(a * b) + c", "+*a b c"),
        ("((a+b)*c)>=100", "((a + b) * c) >= 100", ">=*+a b c 100"),
        ("x>=0 and x<=100", "x >= 0 and x <= 100", "&>=x 0 <=x 100"),
        ("total fn", "sub = price * qty; tax = sub * rate; return sub + tax",
         "s=*p q;t=*s r;+s t"),
        ("guard+not", "if not d.verified: return err(\"not verified\")",
         "!d.verified{^\"not verified\"}"),
    ]

    for desc, infix, prefix in highlights:
        i_toks = token_list(infix)
        p_toks = token_list(prefix)
        print(f"  {desc}:")
        print(f"    infix:  {infix!r}")
        print(f"            {i_toks}  ({len(i_toks)} tokens)")
        print(f"    prefix: {prefix!r}")
        print(f"            {p_toks}  ({len(p_toks)} tokens)")
        print()


if __name__ == "__main__":
    main()
