#!/usr/bin/env python3
"""Token count comparison and cold-LLM testing for ilo syntax ideas.

Usage:
    python3 examples/compare.py              # token counts only
    python3 examples/compare.py --test       # full test (spec + all examples)
    python3 examples/compare.py --test -n 3  # 3 trials each

Requires: pip install tiktoken
For tests: pip install anthropic (and set ANTHROPIC_API_KEY)
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
    "idea3-constrained-decoding": [".json"],
    "idea4-ast-bytecode": [".ast"],
    "idea5-workflow-dag": [".yaml"],
    "idea6-mcp-composition": [".json"],
    "idea7-dense-wire": [".ilo"],
    "idea8-ultra-dense": [".ilo"],
    "idea9-ultra-dense-short": [".ilo"],
}

COMMENT_PREFIXES = {
    ".ilo": "--",
    ".py": "#",
    ".yaml": "#",
    ".ast": ";",
}

IDEAS = [
    "idea1",
    "idea1-compact",
    "idea2-tool-calling",
    "idea3-constrained-decoding",
    "idea4-ast-bytecode",
    "idea5-workflow-dag",
    "idea6-mcp-composition",
    "idea7-dense-wire",
    "idea8-ultra-dense",
    "idea9-ultra-dense-short",
]

FULL_PROMPT = """You are being given the specification and examples of a programming format. Study them carefully, then write a new program in the SAME format.

## Language Specification

{rules}

## Examples

{examples}

Task:
{task}

Output ONLY the code in the same format as the examples above. No explanation, no markdown fences."""

TASKS = {
    "workflow": {
        "desc": """Write a program called `transfer` that moves money between accounts:
- Takes: from-account (text), to-account (text), amount (number)
- Returns: result with a receipt or error text
- Step 1: Call tool `withdraw` with from-account and amount. If it fails, return the error.
- Step 2: Call tool `deposit` with to-account and amount. If it fails, call `refund` with from-account and amount to compensate, then return the error.
- Step 3: Return success with a receipt containing from-account, to-account, amount, and both transaction IDs from steps 1 and 2.""",
    },
    "data_pipeline": {
        "desc": """Write a program called `enrich` that processes a list of orders:
- Takes: orders (a list of order records, each with: id, customer-id, total)
- Returns: a list of enriched orders
- For each order: call tool `lookup-customer` with the customer-id to get customer data (name, email, tier)
- If lookup fails, skip that order
- Calculate a discount: 20% for "gold" tier, 10% for "silver", 0% otherwise
- Return enriched orders with customer name, email, original total, discount amount, and final total""",
    },
    "decision_logic": {
        "desc": """Write a program called `approve` that decides whether to approve a loan:
- Takes: income (number), debt (number), score (number), amount (number)
- Returns: result with approval details or rejection reason
- Rule 1: If score < 500, reject with "Credit score too low"
- Rule 2: Calculate debt-to-income ratio (debt / income). If > 0.4, reject with "Debt ratio too high"
- Rule 3: Calculate max-loan as income * 5. If amount > max-loan, reject with "Amount exceeds limit"
- Rule 4: If all pass, return approved with: amount, rate (3.5 if score > 750, 5.0 if score > 650, 7.5 otherwise), and monthly payment (amount * rate / 100 / 12)""",
    },
    "api_orchestration": {
        "desc": """Write a program called `deploy` that deploys a service:
- Takes: service (text), version (text), environment (text)
- Returns: result with deployment status or error
- Step 1: Call tool `health-check` with service and environment. If unhealthy, return error "Service unhealthy".
- Step 2: Call tool `create-snapshot` with service and environment (backup before deploy). Store the snapshot-id.
- Step 3: Call tool `roll-out` with service, version, and environment. If it fails, call `restore-snapshot` with the snapshot-id to rollback, then return the error.
- Step 4: Call `health-check` again. If unhealthy after deploy, call `restore-snapshot` with snapshot-id, return error "Deploy failed health check".
- Step 5: Return success with service, version, environment, and snapshot-id.""",
    },
}


def check_workflow(text: str) -> dict[str, bool]:
    return {
        "named_transfer": "transfer" in text or "xfr" in text or "trf" in text or "tfr" in text,
        "three_inputs": (
            ("from" in text or "source" in text) and
            ("to" in text or "dest" in text) and
            "amount" in text
        ),
        "calls_withdraw": "withdraw" in text,
        "calls_deposit": "deposit" in text,
        "handles_withdraw_err": (
            "withdraw" in text and ("err" in text or "error" in text or "catch" in text or "fail" in text or "!e" in text or "?{" in text)
        ),
        "handles_deposit_err": (
            "deposit" in text and ("err" in text or "error" in text or "catch" in text or "fail" in text or "!e" in text or "?{" in text)
        ),
        "compensates_refund": "refund" in text,
        "refund_after_deposit": (
            "refund" in text and "deposit" in text and
            text.index("refund") > text.index("deposit")
        ) if "deposit" in text and "refund" in text else False,
        "returns_receipt": (
            ("receipt" in text or "ok" in text or "return" in text or "~" in text) and
            ("from" in text or "source" in text)
        ),
        "receipt_has_both_ids": (
            sum(1 for w in ["wid", "withdraw", "w-id", "txn1", "tx1"]
                if w in text) >= 1 and
            sum(1 for d in ["did", "deposit", "d-id", "txn2", "tx2"]
                if d in text) >= 1
        ),
    }


def check_data_pipeline(text: str) -> dict[str, bool]:
    return {
        "named_enrich": "enrich" in text or "enr" in text,
        "takes_orders": "order" in text or "ord" in text,
        "iterates": (
            "for" in text or "map" in text or "each" in text or
            "yield" in text or "items" in text or "@" in text
        ),
        "calls_lookup": "lookup" in text and "customer" in text,
        "handles_lookup_fail": (
            "lookup" in text and
            ("err" in text or "error" in text or "catch" in text or "skip" in text or "fail" in text or "!_" in text or "?{" in text)
        ),
        "tier_check": "gold" in text and "silver" in text,
        "calculates_discount": (
            ("20" in text or "0.2" in text) and
            ("10" in text or "0.1" in text)
        ),
        "returns_list": (
            "list" in text or "array" in text or
            "yield" in text or "append" in text or "for" in text or
            "@" in text or "l " in text or "l_" in text
        ),
        "includes_final_total": "final" in text or "total" in text,
        "includes_customer_data": "name" in text and "email" in text,
    }


def check_decision_logic(text: str) -> dict[str, bool]:
    return {
        "named_approve": "approve" in text or "aprv" in text or "appr" in text,
        "four_inputs": (
            ("income" in text or "inc" in text) and ("debt" in text or "dbt" in text) and
            ("score" in text or "sc:" in text) and ("amount" in text or "amt" in text)
        ),
        "credit_check": "500" in text and ("score" in text or "credit" in text or "sc" in text),
        "debt_ratio": (
            ("ratio" in text or ("debt" in text and "income" in text) or ("dbt" in text and "inc" in text)) and
            ("0.4" in text or "40" in text)
        ),
        "max_loan": (
            ("income" in text or "inc" in text) and ("5" in text or "max" in text or "limit" in text or "exceed" in text or "mx" in text)
        ),
        "three_rejections": (
            text.count("too low") + text.count("too high") + text.count("exceed") +
            text.count("reject") + text.count("err") + text.count("error") +
            text.count('!"')
        ) >= 3,
        "rate_tiers": "750" in text and "650" in text,
        "rate_values": (
            ("3.5" in text or "3.50" in text) and
            ("5.0" in text or "5.00" in text) and
            ("7.5" in text or "7.50" in text)
        ),
        "monthly_calc": ("12" in text or "1200" in text) and ("100" in text or "month" in text or "payment" in text or "pmt" in text or "1200" in text),
        "returns_approved": "approv" in text or "ok" in text or "success" in text or "~" in text,
    }


def check_api_orchestration(text: str) -> dict[str, bool]:
    return {
        "named_deploy": "deploy" in text,
        "three_inputs": ("service" in text or "svc" in text) and ("version" in text or "ver" in text) and ("environment" in text or "env" in text),
        "calls_health": "health" in text,
        "calls_snapshot": "snapshot" in text,
        "calls_rollout": "roll" in text,
        "stores_snapshot_id": "snapshot" in text and ("id" in text or "let" in text or "=" in text),
        "handles_rollout_fail": (
            "roll" in text and
            ("err" in text or "error" in text or "catch" in text or "fail" in text or "!e" in text or "?{" in text)
        ),
        "rollback_on_fail": (
            "restore" in text and "snapshot" in text
        ),
        "post_deploy_health": (
            text.count("health") >= 2
        ),
        "returns_success": (
            ("ok" in text or "return" in text or "success" in text or "~" in text) and
            ("version" in text or "ver" in text)
        ),
    }


TASK_CHECKERS = {
    "workflow": check_workflow,
    "data_pipeline": check_data_pipeline,
    "decision_logic": check_decision_logic,
    "api_orchestration": check_api_orchestration,
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


def count_folder(folder: Path, extensions: list[str]) -> dict[str, tuple[int, int]]:
    results = {}
    for ext in extensions:
        for f in sorted(folder.glob(f"*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if not cleaned.strip():
                continue
            results[f.name] = (count_tokens(cleaned), len(cleaned))
    return results


def print_token_counts():
    print("=" * 70)
    print("Token and character comparison vs Python (cl100k_base)")
    print("=" * 70)

    tok_totals = {}
    char_totals = {}
    for folder_name, exts in FOLDERS.items():
        folder = EXAMPLES_DIR / folder_name
        if not folder.exists():
            continue
        counts = count_folder(folder, exts)
        if counts:
            tok_totals[folder_name] = sum(t for t, c in counts.values())
            char_totals[folder_name] = sum(c for t, c in counts.values())

    py_tok = tok_totals.get("python-baseline", 1)
    py_char = char_totals.get("python-baseline", 1)

    print(f"\n  {'Idea':30s}  {'Tokens':>7s}  {'vs Py':>6s}  {'Chars':>7s}  {'vs Py':>6s}")
    print(f"  {'-' * 60}")
    for folder_name in FOLDERS:
        if folder_name not in tok_totals:
            continue
        tok = tok_totals[folder_name]
        chars = char_totals[folder_name]
        tok_r = tok / py_tok if py_tok else 0
        char_r = chars / py_char if py_char else 0
        marker = "" if folder_name != "python-baseline" else "  *"
        print(f"  {folder_name:30s}  {tok:7d}  {tok_r:>5.2f}x  {chars:7d}  {char_r:>5.2f}x{marker}")

    print()


def load_all_examples(idea: str) -> str:
    """Load all examples from an idea folder."""
    folder = EXAMPLES_DIR / idea
    exts = FOLDERS.get(idea, [".ilo"])
    parts = []
    for ext in exts:
        for f in sorted(folder.glob(f"*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if cleaned.strip():
                parts.append(f"Example ({f.name}):\n{cleaned}")
    return "\n\n".join(parts)


def load_rules(idea: str) -> str:
    """Load the SPEC.md from an idea folder."""
    spec = EXAMPLES_DIR / idea / "SPEC.md"
    if spec.exists():
        return spec.read_text()
    return ""


def get_client():
    """Get an Anthropic client, or exit with an error."""
    try:
        import anthropic
    except ImportError:
        print("Install anthropic: pip install anthropic")
        sys.exit(1)

    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print("Set ANTHROPIC_API_KEY environment variable")
        sys.exit(1)

    return anthropic.Anthropic(api_key=api_key)


def call_haiku(client, prompt: str, max_tokens: int = 1000) -> tuple[str, float]:
    """Call Haiku with retry. Returns (output_text, elapsed_seconds)."""
    for attempt in range(5):
        try:
            start = time.time()
            response = client.messages.create(
                model="claude-haiku-4-5-20251001",
                max_tokens=max_tokens,
                messages=[{"role": "user", "content": prompt}],
            )
            elapsed = time.time() - start
            return response.content[0].text, elapsed
        except Exception as e:
            if attempt < 4:
                wait = 2 ** attempt
                print(f"    retry in {wait}s: {type(e).__name__}")
                time.sleep(wait)
            else:
                raise


def run_tests(n_trials: int, only_ideas: list[str] | None = None):
    """Run full tests — spec + all examples, the realistic usage scenario."""
    client = get_client()

    ideas = [i for i in IDEAS if i in only_ideas] if only_ideas else IDEAS
    task_names = list(TASKS.keys())
    total_tests = len(ideas) * len(task_names) * n_trials
    print("=" * 70)
    print(f"Full test: spec + all examples (claude-haiku-4-5, {n_trials} trial(s), {len(task_names)} tasks)")
    print(f"Ideas: {', '.join(ideas)}")
    print(f"Total API calls: {total_tests}")
    print("=" * 70)

    all_results = []

    for idea in ideas:
        rules = load_rules(idea)
        examples_text = load_all_examples(idea)

        print(f"\n{'=' * 50}")
        print(f"  {idea}")
        print(f"{'=' * 50}")

        for task_name in task_names:
            print(f"\n  [{task_name}]")
            task_desc = TASKS[task_name]["desc"]
            task_results = []

            for trial in range(1, n_trials + 1):
                prompt = FULL_PROMPT.format(
                    rules=rules, examples=examples_text, task=task_desc,
                )
                prompt_tokens = count_tokens(prompt)
                output, elapsed = call_haiku(client, prompt)
                output_tokens = count_tokens(output)

                checker = TASK_CHECKERS[task_name]
                features = checker(output.lower())
                score = sum(features.values())
                total = len(features)

                result = {
                    "idea": idea,
                    "task": task_name,
                    "output": output,
                    "prompt_tokens": prompt_tokens,
                    "output_tokens": output_tokens,
                    "elapsed_s": round(elapsed, 2),
                    "features": features,
                    "score": f"{score}/{total}",
                }
                all_results.append(result)
                task_results.append(result)

                failed = [k for k, v in features.items() if not v]
                label = f"    T{trial}: {score}/{total} | {output_tokens}tok"
                if failed:
                    label += f" | miss: {', '.join(failed)}"
                print(label)

            scores = [sum(r["features"].values()) for r in task_results]
            avg = sum(scores) / len(scores)
            avg_tokens = sum(r["output_tokens"] for r in task_results) / len(task_results)
            print(f"    avg: {avg:.1f}/10 | {avg_tokens:.0f}tok")

    # Summary
    print(f"\n{'=' * 70}")
    print("Summary")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}  {'Time':>7s}")
    print(f"  {'-' * 57}")
    for idea in IDEAS:
        idea_results = [r for r in all_results if r["idea"] == idea]
        if not idea_results:
            continue
        avg_score = sum(sum(r["features"].values()) for r in idea_results) / len(idea_results)
        avg_tokens = sum(r["output_tokens"] for r in idea_results) / len(idea_results)
        avg_time = sum(r["elapsed_s"] for r in idea_results) / len(idea_results)
        print(f"  {idea:30s}  {avg_score:.1f}/10  {avg_tokens:>7.0f}  {avg_time:>6.2f}s")

    # Per-task breakdown
    print(f"\n  Per-task scores (avg across trials):")
    print(f"\n  {'Idea':30s}", end="")
    for t in task_names:
        print(f"  {t[:10]:>10s}", end="")
    print()
    print(f"  {'-' * (30 + 12 * len(task_names))}")
    for idea in IDEAS:
        print(f"  {idea:30s}", end="")
        for task_name in task_names:
            task_results = [r for r in all_results if r["idea"] == idea and r["task"] == task_name]
            if task_results:
                avg = sum(sum(r["features"].values()) for r in task_results) / len(task_results)
                print(f"  {avg:>9.1f}", end="")
            else:
                print(f"  {'—':>9s}", end="")
        print()

    results_path = EXAMPLES_DIR / "full-results.json"
    with open(results_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n  Raw results saved to {results_path}")


def write_summary():
    """Write a consolidated summary: one row per idea with tokens + test scores."""
    lines = []
    w = lines.append

    tok_totals = {}
    char_totals = {}
    for folder_name, exts in FOLDERS.items():
        folder = EXAMPLES_DIR / folder_name
        if not folder.exists():
            continue
        counts = count_folder(folder, exts)
        if counts:
            tok_totals[folder_name] = sum(t for t, c in counts.values())
            char_totals[folder_name] = sum(c for t, c in counts.values())

    py_tok = tok_totals.get("python-baseline", 1)
    py_char = char_totals.get("python-baseline", 1)

    def load_results(filename):
        path = EXAMPLES_DIR / filename
        if path.exists():
            data = json.loads(path.read_text())
            return data if data else []
        return []

    full_results = load_results("full-results.json")

    # Re-score with current checkers
    for r in full_results:
        if r.get("task") in TASK_CHECKERS:
            r["features"] = TASK_CHECKERS[r["task"]](r["output"].lower())

    def avg_score(results, idea):
        r = [x for x in results if x["idea"] == idea]
        if not r:
            return None
        return sum(sum(x["features"].values()) for x in r) / len(r)

    def avg_out_tokens(results, idea):
        r = [x for x in results if x["idea"] == idea]
        if not r:
            return None
        return sum(x["output_tokens"] for x in r) / len(r)

    w("ilo syntax comparison")
    w("=" * 90)
    w("")
    w(f"  {'Idea':<28s}  {'Tokens':>6s}  {'vs Py':>6s}  {'Chars':>6s}  {'vs Py':>6s}  {'Score':>6s}  {'Out tok':>7s}")
    w(f"  {'-' * 74}")

    all_ideas = list(FOLDERS.keys())
    for idea in all_ideas:
        tok = tok_totals.get(idea)
        if tok is None:
            continue

        chars = char_totals.get(idea, 0)
        tok_r = f"{tok / py_tok:.2f}x" if py_tok else "—"
        char_r = f"{chars / py_char:.2f}x" if py_char else "—"
        baseline = "  *" if idea == "python-baseline" else ""

        full = avg_score(full_results, idea)
        out = avg_out_tokens(full_results, idea)

        full_s = f"{full:.1f}" if full is not None else "—"
        out_s = f"{out:.0f}" if out is not None else "—"

        w(f"  {idea:<28s}  {tok:>6d}  {tok_r:>6s}  {chars:>6d}  {char_r:>6s}  {full_s:>6s}  {out_s:>7s}{baseline}")

    w("")
    w("  Tokens  = total tokens across 5 examples (cl100k_base, comments stripped)")
    w("  Chars   = total characters")
    w("  Score   = LLM generation accuracy /10 (spec + all examples, claude-haiku-4-5)")
    w("  Out tok = avg output tokens generated")
    w("  * = baseline")

    if full_results:
        task_names = list(TASKS.keys())
        w("")
        w("")
        w("Per-task breakdown (Full test)")
        w("=" * 90)
        w("")
        w(f"  {'Idea':<28s}  {'workflow':>10s}  {'data_pipe':>10s}  {'decision':>10s}  {'api_orch':>10s}")
        w(f"  {'-' * 72}")
        for idea in all_ideas:
            if idea == "python-baseline":
                continue
            idea_r = [r for r in full_results if r["idea"] == idea]
            if not idea_r:
                continue
            row = f"  {idea:<28s}"
            for task_name in task_names:
                task_r = [r for r in idea_r if r["task"] == task_name]
                if task_r:
                    avg = sum(sum(r["features"].values()) for r in task_r) / len(task_r)
                    row += f"  {avg:>9.1f}"
                else:
                    row += f"  {'—':>9s}"
            w(row)

    w("")

    summary_path = EXAMPLES_DIR / "test-summary.txt"
    summary_path.write_text("\n".join(lines))
    print(f"\n  Summary saved to {summary_path}")


def main():
    parser = argparse.ArgumentParser(description="ilo token comparison and cold-LLM testing")
    parser.add_argument("--test", action="store_true", help="Run full test (spec + all examples)")
    parser.add_argument("-n", type=int, default=3, help="Number of trials per idea (default: 3)")
    parser.add_argument("--ideas", nargs="+", help="Only test specific ideas")
    args = parser.parse_args()

    print_token_counts()

    if args.test:
        run_tests(args.n, args.ideas)
        write_summary()


if __name__ == "__main__":
    main()
