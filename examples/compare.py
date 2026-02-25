#!/usr/bin/env python3
"""Token count comparison and cold-LLM testing for ilo syntax ideas.

Usage:
    python3 examples/compare.py                      # token counts only
    python3 examples/compare.py --test               # generation from examples
    python3 examples/compare.py --test-comprehend    # comprehension test
    python3 examples/compare.py --test-rules         # generation from rules
    python3 examples/compare.py --test-full           # full test (spec + all examples)
    python3 examples/compare.py --test-all           # all four tests
    python3 examples/compare.py --test-all -n 3      # 3 trials each

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

# All ideas including python-baseline (for comprehension tests)
ALL_IDEAS = ["python-baseline"] + TESTABLE_IDEAS

TEST_PROMPT = """You are being given ONE example of a programming format. Study it carefully, then write a new program in the SAME format.

{examples}

Task:
{task}

Output ONLY the code in the same format as the example above. No explanation, no markdown fences."""

FULL_PROMPT = """You are being given the specification and examples of a programming format. Study them carefully, then write a new program in the SAME format.

## Language Specification

{rules}

## Examples

{examples}

Task:
{task}

Output ONLY the code in the same format as the examples above. No explanation, no markdown fences."""

RULES_PROMPT = """You are being given the specification for a programming format. Study it carefully, then write a program in that format.

{rules}

Task:
{task}

Output ONLY the code in the format described above. No explanation, no markdown fences."""

COMPREHEND_PROMPT = """Here is a program written in a custom format. You have never seen this format before. Read it carefully and explain what it does.

{program}

What does this program do? Explain:
1. The function name and purpose
2. Its inputs
3. The steps it performs (including any tool/function calls)
4. Any error handling or conditional logic
5. What it returns

Be specific and precise."""

# Ideas that have a README.md (spec) for rules-based generation
IDEAS_WITH_RULES = [
    "idea1",
    "idea1-compact",
    "idea2-tool-calling",
    "idea3-constrained-decoding",
    "idea4-ast-bytecode",
    "idea5-workflow-dag",
    "idea6-mcp-composition",
    "idea7-dense-wire",
]

# Comprehension test examples and their checkers
COMPREHEND_EXAMPLES = {
    "04-tool-interaction": {
        "checkers": {
            "mentions_notify": lambda t: "notify" in t,
            "mentions_user_id_input": lambda t: "user" in t and "id" in t,
            "mentions_message_input": lambda t: "message" in t,
            "calls_get_user": lambda t: ("get" in t or "fetch" in t or "lookup" in t or "look" in t) and "user" in t,
            "calls_send_email": lambda t: ("send" in t or "dispatch" in t) and "email" in t,
            "checks_verified": lambda t: "verif" in t or "verified" in t or "validation" in t,
            "handles_errors": lambda t: "error" in t or "fail" in t or "err" in t or "exception" in t,
            "returns_result": lambda t: "return" in t or "result" in t or "success" in t or "ok" in t,
        },
    },
    "05-workflow": {
        "checkers": {
            "mentions_checkout": lambda t: "checkout" in t or "check" in t,
            "mentions_payment_input": lambda t: "payment" in t,
            "mentions_items_input": lambda t: "item" in t or "inventory" in t,
            "calls_reserve": lambda t: "reserve" in t or "reservation" in t or "inventory" in t,
            "calls_charge": lambda t: "charge" in t or "payment" in t,
            "compensates_release": lambda t: "release" in t or "rollback" in t or "undo" in t or "compensat" in t,
            "handles_errors": lambda t: "error" in t or "fail" in t or "err" in t or "exception" in t,
            "returns_result": lambda t: "return" in t or "result" in t or "order" in t,
        },
    },
}

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
    print("Token comparison vs Python (cl100k_base)")
    print("=" * 70)

    # Collect totals for all ideas
    totals = {}
    for folder_name, exts in FOLDERS.items():
        folder = EXAMPLES_DIR / folder_name
        if not folder.exists():
            continue
        counts = count_folder(folder, exts)
        if counts:
            totals[folder_name] = sum(counts.values())

    py_total = totals.get("python-baseline", 1)

    print(f"\n  {'Idea':30s}  {'Tokens':>7s}  {'vs Python':>10s}")
    print(f"  {'-' * 51}")
    for folder_name in FOLDERS:
        if folder_name not in totals:
            continue
        total = totals[folder_name]
        ratio = total / py_total if py_total else 0
        marker = "" if folder_name != "python-baseline" else "  (baseline)"
        print(f"  {folder_name:30s}  {total:7d}  {ratio:>9.2f}x{marker}")

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


def run_test(idea: str, task_name: str, client) -> dict:
    """Run a single cold-LLM test for an idea + task. Returns results dict."""
    examples_text = load_examples(idea)
    task_desc = TASKS[task_name]["desc"]
    prompt = TEST_PROMPT.format(examples=examples_text, task=task_desc)

    prompt_tokens = count_tokens(prompt)
    output, elapsed = call_haiku(client, prompt)
    output_tokens = count_tokens(output)
    checker = TASK_CHECKERS[task_name]
    features = checker(output.lower())
    score = sum(features.values())
    total = len(features)

    return {
        "idea": idea,
        "test_type": "examples",
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
    client = get_client()

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


def load_example_file(idea: str, example_name: str) -> str:
    """Load a specific example file from an idea folder."""
    folder = EXAMPLES_DIR / idea
    exts = FOLDERS.get(idea, [".ilo"])
    for ext in exts:
        for f in sorted(folder.glob(f"{example_name}*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if cleaned.strip():
                return cleaned
    return ""


def load_rules(idea: str) -> str:
    """Load the README.md (spec) from an idea folder."""
    readme = EXAMPLES_DIR / idea / "README.md"
    if readme.exists():
        return readme.read_text()
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


def run_comprehension_tests(n_trials: int):
    """Run comprehension tests — can Haiku explain what a program does?"""
    client = get_client()

    examples_to_test = list(COMPREHEND_EXAMPLES.keys())
    total_tests = len(ALL_IDEAS) * len(examples_to_test) * n_trials
    print("=" * 70)
    print(f"Comprehension test (claude-haiku-4-5, {n_trials} trial(s))")
    print(f"Examples: {', '.join(examples_to_test)}")
    print(f"Total API calls: {total_tests}")
    print("=" * 70)

    all_results = []

    for idea in ALL_IDEAS:
        print(f"\n{'=' * 50}")
        print(f"  {idea}")
        print(f"{'=' * 50}")

        for example_name in examples_to_test:
            program = load_example_file(idea, example_name)
            if not program:
                print(f"\n  [{example_name}] — skipped (no file)")
                continue

            checkers = COMPREHEND_EXAMPLES[example_name]["checkers"]
            print(f"\n  [{example_name}]")

            for trial in range(1, n_trials + 1):
                prompt = COMPREHEND_PROMPT.format(program=program)
                prompt_tokens = count_tokens(prompt)
                output, elapsed = call_haiku(client, prompt, max_tokens=800)
                output_lower = output.lower()
                output_tokens = count_tokens(output)

                features = {k: fn(output_lower) for k, fn in checkers.items()}
                score = sum(features.values())
                total = len(features)

                result = {
                    "idea": idea,
                    "test_type": "comprehension",
                    "example": example_name,
                    "output": output,
                    "prompt_tokens": prompt_tokens,
                    "output_tokens": output_tokens,
                    "elapsed_s": round(elapsed, 2),
                    "features": features,
                    "score": f"{score}/{total}",
                }
                all_results.append(result)

                failed = [k for k, v in features.items() if not v]
                label = f"    T{trial}: {score}/{total} | {output_tokens}tok"
                if failed:
                    label += f" | miss: {', '.join(failed)}"
                print(label)

    # Summary
    print(f"\n{'=' * 70}")
    print("Comprehension Summary")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}")
    print(f"  {'-' * 50}")
    for idea in ALL_IDEAS:
        idea_results = [r for r in all_results if r["idea"] == idea]
        if not idea_results:
            continue
        max_score = len(list(COMPREHEND_EXAMPLES.values())[0]["checkers"])
        avg_score = sum(sum(r["features"].values()) for r in idea_results) / len(idea_results)
        avg_tokens = sum(r["output_tokens"] for r in idea_results) / len(idea_results)
        print(f"  {idea:30s}  {avg_score:.1f}/{max_score}   {avg_tokens:>7.0f}")

    # Per-example breakdown
    print(f"\n  Per-example scores (avg across trials):")
    print(f"\n  {'Idea':30s}", end="")
    for ex in examples_to_test:
        print(f"  {ex[:15]:>15s}", end="")
    print()
    print(f"  {'-' * (30 + 17 * len(examples_to_test))}")
    for idea in ALL_IDEAS:
        print(f"  {idea:30s}", end="")
        for example_name in examples_to_test:
            ex_results = [r for r in all_results if r["idea"] == idea and r["example"] == example_name]
            if ex_results:
                checkers = COMPREHEND_EXAMPLES[example_name]["checkers"]
                avg = sum(sum(r["features"].values()) for r in ex_results) / len(ex_results)
                print(f"  {avg:>14.1f}", end="")
            else:
                print(f"  {'skip':>14s}", end="")
        print()

    results_path = EXAMPLES_DIR / "comprehension-results.json"
    with open(results_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n  Raw results saved to {results_path}")

    return all_results


def run_rules_tests(n_trials: int):
    """Run rules-based generation tests — can Haiku write code from just the spec?"""
    client = get_client()

    task_names = list(TASKS.keys())
    testable = [i for i in TESTABLE_IDEAS if i in IDEAS_WITH_RULES]
    total_tests = len(testable) * len(task_names) * n_trials
    print("=" * 70)
    print(f"Rules-based generation test (claude-haiku-4-5, {n_trials} trial(s), {len(task_names)} tasks)")
    print(f"Ideas with rules: {', '.join(testable)}")
    skipped = [i for i in TESTABLE_IDEAS if i not in IDEAS_WITH_RULES]
    if skipped:
        print(f"Skipped (no README): {', '.join(skipped)}")
    print(f"Total API calls: {total_tests}")
    print("=" * 70)

    all_results = []

    for idea in testable:
        rules = load_rules(idea)
        if not rules:
            continue

        print(f"\n{'=' * 50}")
        print(f"  {idea}")
        print(f"{'=' * 50}")

        for task_name in task_names:
            print(f"\n  [{task_name}]")
            task_desc = TASKS[task_name]["desc"]
            task_results = []

            for trial in range(1, n_trials + 1):
                prompt = RULES_PROMPT.format(rules=rules, task=task_desc)
                prompt_tokens = count_tokens(prompt)
                output, elapsed = call_haiku(client, prompt)
                output_tokens = count_tokens(output)

                checker = TASK_CHECKERS[task_name]
                features = checker(output.lower())
                score = sum(features.values())
                total = len(features)

                result = {
                    "idea": idea,
                    "test_type": "rules",
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
    print("Rules-Based Generation Summary")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}  {'Time':>7s}")
    print(f"  {'-' * 57}")
    for idea in testable:
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
    for idea in testable:
        print(f"  {idea:30s}", end="")
        for task_name in task_names:
            task_results = [r for r in all_results if r["idea"] == idea and r["task"] == task_name]
            if task_results:
                avg = sum(sum(r["features"].values()) for r in task_results) / len(task_results)
                print(f"  {avg:>9.1f}", end="")
            else:
                print(f"  {'skip':>9s}", end="")
        print()

    results_path = EXAMPLES_DIR / "rules-results.json"
    with open(results_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n  Raw results saved to {results_path}")

    return all_results


def run_full_tests(n_trials: int):
    """Run full tests — spec + all examples, the realistic usage scenario."""
    client = get_client()

    task_names = list(TASKS.keys())
    testable = [i for i in TESTABLE_IDEAS if i in IDEAS_WITH_RULES]
    total_tests = len(testable) * len(task_names) * n_trials
    skipped = [i for i in TESTABLE_IDEAS if i not in IDEAS_WITH_RULES]
    print("=" * 70)
    print(f"Full test: spec + all examples (claude-haiku-4-5, {n_trials} trial(s), {len(task_names)} tasks)")
    print(f"Ideas: {', '.join(testable)}")
    if skipped:
        print(f"Skipped (no README): {', '.join(skipped)}")
    print(f"Total API calls: {total_tests}")
    print("=" * 70)

    all_results = []

    for idea in testable:
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
                    "test_type": "full",
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
    print("Full Test Summary (spec + all examples)")
    print(f"{'=' * 70}")
    print(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}  {'Time':>7s}")
    print(f"  {'-' * 57}")
    for idea in testable:
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
    for idea in testable:
        print(f"  {idea:30s}", end="")
        for task_name in task_names:
            task_results = [r for r in all_results if r["idea"] == idea and r["task"] == task_name]
            if task_results:
                avg = sum(sum(r["features"].values()) for r in task_results) / len(task_results)
                print(f"  {avg:>9.1f}", end="")
            else:
                print(f"  {'skip':>9s}", end="")
        print()

    results_path = EXAMPLES_DIR / "full-results.json"
    with open(results_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n  Raw results saved to {results_path}")

    return all_results


def write_summary():
    """Write a clean summary file with just the token table + 3 test summary tables."""
    lines = []
    w = lines.append

    # Token comparison table
    totals = {}
    for folder_name, exts in FOLDERS.items():
        folder = EXAMPLES_DIR / folder_name
        if not folder.exists():
            continue
        counts = count_folder(folder, exts)
        if counts:
            totals[folder_name] = sum(counts.values())

    py_total = totals.get("python-baseline", 1)

    w("=" * 70)
    w("Token comparison vs Python (cl100k_base)")
    w("=" * 70)
    w(f"\n  {'Idea':30s}  {'Tokens':>7s}  {'vs Python':>10s}")
    w(f"  {'-' * 51}")
    for folder_name in FOLDERS:
        if folder_name not in totals:
            continue
        total = totals[folder_name]
        ratio = total / py_total if py_total else 0
        marker = "" if folder_name != "python-baseline" else "  (baseline)"
        w(f"  {folder_name:30s}  {total:7d}  {ratio:>9.2f}x{marker}")

    task_names = list(TASKS.keys())

    # Helper to load and summarise a results JSON
    def summarise_generation(path, title, ideas):
        if not path.exists():
            return
        results = json.loads(path.read_text())
        if not results:
            return
        w(f"\n\n{'=' * 70}")
        w(f"{title}")
        w(f"{'=' * 70}")
        w(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}  {'Time':>7s}")
        w(f"  {'-' * 57}")
        for idea in ideas:
            idea_results = [r for r in results if r["idea"] == idea]
            if not idea_results:
                continue
            avg_score = sum(sum(r["features"].values()) for r in idea_results) / len(idea_results)
            avg_tokens = sum(r["output_tokens"] for r in idea_results) / len(idea_results)
            avg_time = sum(r["elapsed_s"] for r in idea_results) / len(idea_results)
            w(f"  {idea:30s}  {avg_score:.1f}/10  {avg_tokens:>7.0f}  {avg_time:>6.2f}s")

        w(f"\n  Per-task scores (avg across trials):")
        w(f"\n  {'Idea':30s}" + "".join(f"  {t[:10]:>10s}" for t in task_names))
        w(f"  {'-' * (30 + 12 * len(task_names))}")
        for idea in ideas:
            row = f"  {idea:30s}"
            idea_results = [r for r in results if r["idea"] == idea]
            if not idea_results:
                continue
            for task_name in task_names:
                task_results = [r for r in idea_results if r["task"] == task_name]
                if task_results:
                    avg = sum(sum(r["features"].values()) for r in task_results) / len(task_results)
                    row += f"  {avg:>9.1f}"
                else:
                    row += f"  {'—':>9s}"
            w(row)

    # Table 1: Generation from examples
    summarise_generation(
        EXAMPLES_DIR / "test-results.json",
        "Generation from Examples (one-shot)",
        TESTABLE_IDEAS,
    )

    # Table 2: Comprehension
    comp_path = EXAMPLES_DIR / "comprehension-results.json"
    if comp_path.exists():
        results = json.loads(comp_path.read_text())
        if results:
            examples_to_test = list(COMPREHEND_EXAMPLES.keys())
            w(f"\n\n{'=' * 70}")
            w("Comprehension (explain what the program does)")
            w(f"{'=' * 70}")
            w(f"\n  {'Idea':30s}  {'Score':>8s}  {'Tokens':>8s}")
            w(f"  {'-' * 50}")
            for idea in ALL_IDEAS:
                idea_results = [r for r in results if r["idea"] == idea]
                if not idea_results:
                    continue
                max_score = len(list(COMPREHEND_EXAMPLES.values())[0]["checkers"])
                avg_score = sum(sum(r["features"].values()) for r in idea_results) / len(idea_results)
                avg_tokens = sum(r["output_tokens"] for r in idea_results) / len(idea_results)
                w(f"  {idea:30s}  {avg_score:.1f}/{max_score}   {avg_tokens:>7.0f}")

            w(f"\n  Per-example scores (avg across trials):")
            w(f"\n  {'Idea':30s}" + "".join(f"  {ex[:15]:>15s}" for ex in examples_to_test))
            w(f"  {'-' * (30 + 17 * len(examples_to_test))}")
            for idea in ALL_IDEAS:
                row = f"  {idea:30s}"
                has_data = False
                for example_name in examples_to_test:
                    ex_results = [r for r in results if r["idea"] == idea and r["example"] == example_name]
                    if ex_results:
                        avg = sum(sum(r["features"].values()) for r in ex_results) / len(ex_results)
                        row += f"  {avg:>14.1f}"
                        has_data = True
                    else:
                        row += f"  {'—':>14s}"
                if has_data:
                    w(row)

    # Table 3: Generation from rules
    summarise_generation(
        EXAMPLES_DIR / "rules-results.json",
        "Generation from Rules (spec only, no examples)",
        [i for i in TESTABLE_IDEAS if i in IDEAS_WITH_RULES],
    )

    # Table 4: Full (spec + all examples)
    summarise_generation(
        EXAMPLES_DIR / "full-results.json",
        "Full Test (spec + all examples)",
        [i for i in TESTABLE_IDEAS if i in IDEAS_WITH_RULES],
    )

    w("")

    summary_path = EXAMPLES_DIR / "test-summary.txt"
    summary_path.write_text("\n".join(lines))
    print(f"\n  Summary saved to {summary_path}")


def main():
    parser = argparse.ArgumentParser(description="ilo token comparison and cold-LLM testing")
    parser.add_argument("--test", action="store_true", help="Run generation-from-examples test")
    parser.add_argument("--test-comprehend", action="store_true", help="Run comprehension test")
    parser.add_argument("--test-rules", action="store_true", help="Run generation-from-rules test")
    parser.add_argument("--test-full", action="store_true", help="Run full test (spec + all examples)")
    parser.add_argument("--test-all", action="store_true", help="Run all four test modes")
    parser.add_argument("-n", type=int, default=3, help="Number of trials per idea (default: 3)")
    args = parser.parse_args()

    print_token_counts()

    ran_tests = False
    if args.test_all or args.test:
        run_tests(args.n)
        ran_tests = True
    if args.test_all or args.test_comprehend:
        run_comprehension_tests(args.n)
        ran_tests = True
    if args.test_all or args.test_rules:
        run_rules_tests(args.n)
        ran_tests = True
    if args.test_all or args.test_full:
        run_full_tests(args.n)
        ran_tests = True

    if ran_tests:
        write_summary()


if __name__ == "__main__":
    main()
