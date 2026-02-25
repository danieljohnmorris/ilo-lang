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
    "idea3-constrained-decoding": [".json"],
    "idea4-ast-bytecode": [".ast"],
    "idea5-workflow-dag": [".yaml"],
    "idea6-mcp-composition": [".json"],
    "idea7-dense-wire": [".ilo"],
}

COMMENT_PREFIXES = {
    ".ilo": "--",
    ".py": "#",
    ".yaml": "#",
    ".ast": ";",
}

# All ideas with 5 examples get tested
TESTABLE_IDEAS = [
    "idea1",
    "idea1-compact",
    "idea2-tool-calling",
    "idea3-constrained-decoding",
    "idea4-ast-bytecode",
    "idea5-workflow-dag",
    "idea6-mcp-composition",
    "idea7-dense-wire",
]

TEST_PROMPT = """You are being given ONE example of a programming format. Study it carefully, then write a new program in the SAME format.

{examples}

Task:
{task}

Output ONLY the code in the same format as the example above. No explanation, no markdown fences."""

# Multiple test tasks covering different use cases.
# Each has: description, checker function.
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
        "named_transfer": "transfer" in text,
        "three_inputs": (
            ("from" in text or "source" in text) and
            ("to" in text or "dest" in text) and
            "amount" in text
        ),
        "calls_withdraw": "withdraw" in text,
        "calls_deposit": "deposit" in text,
        "handles_withdraw_err": (
            "withdraw" in text and ("err" in text or "error" in text or "catch" in text or "fail" in text)
        ),
        "handles_deposit_err": (
            "deposit" in text and ("err" in text or "error" in text or "catch" in text or "fail" in text)
        ),
        "compensates_refund": "refund" in text,
        "refund_after_deposit": (
            "refund" in text and "deposit" in text and
            text.index("refund") > text.index("deposit")
        ) if "deposit" in text and "refund" in text else False,
        "returns_receipt": (
            ("receipt" in text or "ok" in text or "return" in text) and
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
        "named_enrich": "enrich" in text,
        "takes_orders": "order" in text,
        "iterates": (
            "for" in text or "map" in text or "each" in text or
            "yield" in text or "items" in text
        ),
        "calls_lookup": "lookup" in text and "customer" in text,
        "handles_lookup_fail": (
            "lookup" in text and
            ("err" in text or "error" in text or "catch" in text or "skip" in text or "fail" in text)
        ),
        "tier_check": "gold" in text and "silver" in text,
        "calculates_discount": (
            ("20" in text or "0.2" in text) and
            ("10" in text or "0.1" in text)
        ),
        "returns_list": (
            "list" in text or "array" in text or
            "yield" in text or "append" in text or "for" in text
        ),
        "includes_final_total": "final" in text or "total" in text,
        "includes_customer_data": "name" in text and "email" in text,
    }


def check_decision_logic(text: str) -> dict[str, bool]:
    return {
        "named_approve": "approve" in text,
        "four_inputs": (
            "income" in text and "debt" in text and
            "score" in text and "amount" in text
        ),
        "credit_check": "500" in text and ("score" in text or "credit" in text),
        "debt_ratio": (
            ("ratio" in text or ("debt" in text and "income" in text)) and
            ("0.4" in text or "40" in text)
        ),
        "max_loan": (
            "income" in text and ("5" in text or "max" in text or "limit" in text or "exceed" in text)
        ),
        "three_rejections": (
            text.count("too low") + text.count("too high") + text.count("exceed") +
            text.count("reject") + text.count("err") + text.count("error")
        ) >= 3,
        "rate_tiers": "750" in text and "650" in text,
        "rate_values": (
            ("3.5" in text or "3.50" in text) and
            ("5.0" in text or "5.00" in text) and
            ("7.5" in text or "7.50" in text)
        ),
        "monthly_calc": "12" in text and ("100" in text or "month" in text or "payment" in text),
        "returns_approved": "approv" in text or "ok" in text or "success" in text,
    }


def check_api_orchestration(text: str) -> dict[str, bool]:
    return {
        "named_deploy": "deploy" in text,
        "three_inputs": "service" in text and "version" in text and "environment" in text,
        "calls_health": "health" in text,
        "calls_snapshot": "snapshot" in text,
        "calls_rollout": "roll" in text,
        "stores_snapshot_id": "snapshot" in text and ("id" in text or "let" in text or "=" in text),
        "handles_rollout_fail": (
            "roll" in text and
            ("err" in text or "error" in text or "catch" in text or "fail" in text)
        ),
        "rollback_on_fail": (
            "restore" in text and "snapshot" in text
        ),
        "post_deploy_health": (
            # health-check appears at least twice (before and after deploy)
            text.count("health") >= 2
        ),
        "returns_success": (
            ("ok" in text or "return" in text or "success" in text) and
            "version" in text
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
    """Load only example 01 from an idea folder — forces extrapolation."""
    folder = EXAMPLES_DIR / idea
    exts = FOLDERS.get(idea, [".ilo"])
    parts = []
    for ext in exts:
        for f in sorted(folder.glob(f"01-*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if cleaned.strip():
                parts.append(f"Example ({f.name}):\n{cleaned}")
    return "\n\n".join(parts)


def run_test(idea: str, task_name: str, client) -> dict:
    """Run a single cold-LLM test for an idea + task. Returns results dict."""
    examples_text = load_examples(idea)
    task_desc = TASKS[task_name]["desc"]
    prompt = TEST_PROMPT.format(examples=examples_text, task=task_desc)

    prompt_tokens = count_tokens(prompt)

    # Retry with backoff on transient errors
    for attempt in range(5):
        try:
            start = time.time()
            response = client.messages.create(
                model="claude-haiku-4-5-20251001",
                max_tokens=1000,
                messages=[{"role": "user", "content": prompt}],
            )
            elapsed = time.time() - start
            break
        except Exception as e:
            if attempt < 4:
                wait = 2 ** attempt
                print(f"    retry in {wait}s: {type(e).__name__}")
                time.sleep(wait)
            else:
                raise

    output = response.content[0].text
    output_tokens = count_tokens(output)
    checker = TASK_CHECKERS[task_name]
    features = checker(output.lower())
    score = sum(features.values())
    total = len(features)

    return {
        "idea": idea,
        "task": task_name,
        "output": output,
        "prompt_tokens": prompt_tokens,
        "output_tokens": output_tokens,
        "elapsed_s": round(elapsed, 2),
        "features": features,
        "score": f"{score}/{total}",
    }


def run_tests(n_trials: int):
    """Run cold-LLM tests across all ideas and tasks."""
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

    task_names = list(TASKS.keys())
    total_tests = len(TESTABLE_IDEAS) * len(task_names) * n_trials
    print("=" * 70)
    print(f"Cold-LLM test (claude-haiku-4-5, {n_trials} trial(s), {len(task_names)} tasks)")
    print(f"Total API calls: {total_tests}")
    print("=" * 70)

    all_results = []

    for idea in TESTABLE_IDEAS:
        print(f"\n{'=' * 50}")
        print(f"  {idea}")
        print(f"{'=' * 50}")

        for task_name in task_names:
            print(f"\n  [{task_name}]")
            task_results = []

            for trial in range(1, n_trials + 1):
                result = run_test(idea, task_name, client)
                all_results.append(result)
                task_results.append(result)

                features = result["features"]
                failed = [k for k, v in features.items() if not v]

                label = f"    T{trial}: {result['score']} | {result['output_tokens']}tok"
                if failed:
                    label += f" | miss: {', '.join(failed)}"
                print(label)

            scores = [sum(r["features"].values()) for r in task_results]
            avg = sum(scores) / len(scores)
            avg_tokens = sum(r["output_tokens"] for r in task_results) / len(task_results)
            print(f"    avg: {avg:.1f}/10 | {avg_tokens:.0f}tok")

    # Cross-idea summary
    print(f"\n{'=' * 70}")
    print("Summary (averaged across all tasks and trials)")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}  {'Time':>7s}")
    print(f"  {'-' * 57}")
    for idea in TESTABLE_IDEAS:
        idea_results = [r for r in all_results if r["idea"] == idea]
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
    for idea in TESTABLE_IDEAS:
        print(f"  {idea:30s}", end="")
        for task_name in task_names:
            task_results = [r for r in all_results if r["idea"] == idea and r["task"] == task_name]
            avg = sum(sum(r["features"].values()) for r in task_results) / len(task_results)
            print(f"  {avg:>9.1f}", end="")
        print()

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
        run_tests(args.n)


if __name__ == "__main__":
    main()
