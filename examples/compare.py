#!/usr/bin/env python3
"""Token count comparison and cold-LLM testing for ilo syntax ideas.

Usage:
    python3 examples/compare.py              # token counts only
    python3 examples/compare.py --test       # token counts + Haiku cold test
    python3 examples/compare.py --test -n 3  # run 3 trials per idea

Requires: pip install tiktoken
For --test: pip install anthropic (and set ANTHROPIC_API_KEY)
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

try:
    import tiktoken
except ImportError:
    print("Install tiktoken: pip install tiktoken")
    sys.exit(1)

ENC = tiktoken.get_encoding("cl100k_base")

EXAMPLES_DIR = Path(__file__).parent

FOLDERS = {
    "python-baseline": [".py"],
    "idea1": [".ilo"],
    "idea1-compact": [".ilo"],
    "idea2-tool-calling": [".json"],
    "idea3-constrained-decoding": [".md"],
    "idea4-ast-bytecode": [".ast"],
    "idea5-workflow-dag": [".yaml"],
    "idea6-mcp-composition": [".json"],
    "idea7-dense-wire": [".ilo"],
}

COMMENT_PREFIXES = {
    ".ilo": "--",
    ".py": "#",
    ".yaml": "#",
}

# Ideas to test with Haiku (must have examples 01 and 04 as training, plus enough syntax)
TESTABLE_IDEAS = ["idea1", "idea1-compact"]

TEST_PROMPT = """You are being given examples of a language called "ilo". Study them, then write a new function.

{examples}

Now write a function called `validate-email` in the same ilo syntax that:
- Takes an email (text) and returns result bool, text
- Depends on a tool called `check-format` that takes (email: text) -> result bool, text
- Calls check-format with the email
- If check-format returns err, return err "Invalid format"
- If check-format returns ok with false, return err "Invalid format"
- If check-format returns ok with true, return ok true

Output ONLY the ilo code. No explanation, no markdown fences."""

# What we check in the output
EXPECTED_FEATURES = {
    "tool_decl": "tool declaration for check-format",
    "fn_decl": "function declaration",
    "dep": "dependency on check-format",
    "named_args": "named arguments at call site",
    "match": "match/pattern matching on result",
    "ok": "ok constructor",
    "err": "err constructor",
    "result_type": "result return type",
}


def strip_comments(text: str, ext: str) -> str:
    prefix = COMMENT_PREFIXES.get(ext)
    lines = text.splitlines()
    kept = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if prefix and stripped.startswith(prefix):
            continue
        kept.append(line)
    return "\n".join(kept)


def count_tokens(text: str) -> int:
    return len(ENC.encode(text))


def count_folder(folder: Path, extensions: list[str]) -> dict[str, int]:
    results = {}
    for ext in extensions:
        for f in sorted(folder.glob(f"*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if not cleaned.strip():
                continue
            results[f.name] = count_tokens(cleaned)
    return results


def print_token_counts():
    print("=" * 70)
    print("ilo token comparison (cl100k_base)")
    print("=" * 70)

    py_folder = EXAMPLES_DIR / "python-baseline"
    py_counts = count_folder(py_folder, [".py"])
    py_total = sum(py_counts.values())

    print(f"\n{'python-baseline':30s}  {'tokens':>7s}  {'vs python':>10s}")
    print("-" * 52)
    for name, tokens in sorted(py_counts.items()):
        print(f"  {name:28s}  {tokens:7d}")
    print(f"  {'TOTAL':28s}  {py_total:7d}  {'1.00x':>10s}")

    for folder_name, exts in FOLDERS.items():
        if folder_name == "python-baseline":
            continue
        folder = EXAMPLES_DIR / folder_name
        if not folder.exists():
            continue
        counts = count_folder(folder, exts)
        if not counts:
            continue
        total = sum(counts.values())
        ratio = total / py_total if py_total else 0

        print(f"\n{folder_name:30s}  {'tokens':>7s}  {'vs python':>10s}")
        print("-" * 52)
        for name, tokens in sorted(counts.items()):
            print(f"  {name:28s}  {tokens:7d}")
        print(f"  {'TOTAL':28s}  {total:7d}  {ratio:.2f}x")

    print()


def load_examples(idea: str) -> str:
    """Load examples 01 and 04 from an idea folder as training context."""
    folder = EXAMPLES_DIR / idea
    exts = FOLDERS.get(idea, [".ilo"])
    parts = []
    for ext in exts:
        for num in ["01", "04"]:
            matches = sorted(folder.glob(f"{num}-*{ext}"))
            for f in matches:
                raw = f.read_text()
                cleaned = strip_comments(raw, ext)
                if cleaned.strip():
                    parts.append(f"Example ({f.name}):\n{cleaned}")
    return "\n\n".join(parts)


def check_output(output: str, idea: str) -> dict[str, bool]:
    """Check which expected features appear in the model output."""
    text = output.lower()
    is_compact = "compact" in idea

    results = {}
    results["tool_decl"] = "tool " in text and "check-format" in text
    results["fn_decl"] = "fn " in text or "fn validate" in text
    results["dep"] = "@check-format" in text or "@ check-format" in text
    results["named_args"] = "email:" in text
    results["match"] = "match " in text or "match{" in text
    results["ok"] = "ok " in text or "ok(" in text
    results["err"] = "err " in text or "err(" in text
    results["result_type"] = "result " in text and ("bool" in text)
    return results


def run_test(idea: str, client, trial: int) -> dict:
    """Run a single cold-LLM test for an idea. Returns results dict."""
    examples_text = load_examples(idea)
    prompt = TEST_PROMPT.format(examples=examples_text)

    prompt_tokens = count_tokens(prompt)

    start = time.time()
    response = client.messages.create(
        model="claude-haiku-4-5-20251001",
        max_tokens=500,
        messages=[{"role": "user", "content": prompt}],
    )
    elapsed = time.time() - start

    output = response.content[0].text
    output_tokens = count_tokens(output)
    features = check_output(output, idea)
    score = sum(features.values())
    total = len(features)

    return {
        "idea": idea,
        "trial": trial,
        "output": output,
        "prompt_tokens": prompt_tokens,
        "output_tokens": output_tokens,
        "elapsed_s": round(elapsed, 2),
        "features": features,
        "score": f"{score}/{total}",
    }


def run_tests(n_trials: int):
    """Run cold-LLM tests across testable ideas."""
    try:
        import anthropic
    except ImportError:
        print("Install anthropic: pip install anthropic")
        sys.exit(1)

    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print("Set ANTHROPIC_API_KEY environment variable")
        sys.exit(1)

    client = anthropic.Anthropic(api_key=api_key)

    print("=" * 70)
    print(f"Cold-LLM test (claude-haiku-4-5, {n_trials} trial(s) per idea)")
    print("=" * 70)

    all_results = []

    for idea in TESTABLE_IDEAS:
        print(f"\n--- {idea} ---")
        idea_scores = []

        for trial in range(1, n_trials + 1):
            result = run_test(idea, client, trial)
            all_results.append(result)
            idea_scores.append(result)

            features = result["features"]
            passed = [k for k, v in features.items() if v]
            failed = [k for k, v in features.items() if not v]

            print(f"\n  Trial {trial}: {result['score']} features | "
                  f"{result['output_tokens']} tokens | {result['elapsed_s']}s")
            if failed:
                print(f"    missing: {', '.join(failed)}")
            print(f"    output: {result['output'][:120]}...")

        # Summary for this idea
        scores = [sum(r["features"].values()) for r in idea_scores]
        avg = sum(scores) / len(scores)
        total = len(EXPECTED_FEATURES)
        avg_tokens = sum(r["output_tokens"] for r in idea_scores) / len(idea_scores)
        print(f"\n  Average: {avg:.1f}/{total} features, {avg_tokens:.0f} output tokens")

    # Cross-idea summary
    print(f"\n{'=' * 70}")
    print("Summary")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':20s}  {'Avg Score':>10s}  {'Avg Tokens':>11s}  {'Avg Time':>9s}")
    print(f"  {'-' * 54}")
    for idea in TESTABLE_IDEAS:
        idea_results = [r for r in all_results if r["idea"] == idea]
        total = len(EXPECTED_FEATURES)
        avg_score = sum(sum(r["features"].values()) for r in idea_results) / len(idea_results)
        avg_tokens = sum(r["output_tokens"] for r in idea_results) / len(idea_results)
        avg_time = sum(r["elapsed_s"] for r in idea_results) / len(idea_results)
        print(f"  {idea:20s}  {avg_score:.1f}/{total:d}      {avg_tokens:>7.0f}      {avg_time:.2f}s")

    # Save raw results
    results_path = EXAMPLES_DIR / "test-results.json"
    with open(results_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n  Raw results saved to {results_path}")


def main():
    parser = argparse.ArgumentParser(description="ilo token comparison and cold-LLM testing")
    parser.add_argument("--test", action="store_true", help="Run cold-LLM tests with Haiku")
    parser.add_argument("-n", type=int, default=3, help="Number of trials per idea (default: 3)")
    args = parser.parse_args()

    print_token_counts()

    if args.test:
        run_tests(args.n_trials if hasattr(args, "n_trials") else args.n)


if __name__ == "__main__":
    main()
