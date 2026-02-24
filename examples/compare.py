#!/usr/bin/env python3
"""Token count comparison across all ilo syntax ideas and Python baseline.

Usage:
    python3 examples/compare.py

Requires: pip install tiktoken
"""

import os
import sys
from pathlib import Path

try:
    import tiktoken
except ImportError:
    print("Install tiktoken: pip install tiktoken")
    sys.exit(1)

ENC = tiktoken.get_encoding("cl100k_base")

EXAMPLES_DIR = Path(__file__).parent

# Map folder name -> file extensions to count
FOLDERS = {
    "python-baseline": [".py"],
    "idea1": [".ilo"],
    "idea1-compact": [".ilo"],
    "idea2-tool-calling": [".json"],
    "idea3-constrained-decoding": [".md"],  # code blocks in markdown
    "idea4-ast-bytecode": [".ast"],
    "idea5-workflow-dag": [".yaml"],
    "idea6-mcp-composition": [".json"],
    "idea7-dense-wire": [".ilo"],
}

# Files where we should strip comments before counting
COMMENT_PREFIXES = {
    ".ilo": "--",
    ".py": "#",
    ".yaml": "#",
}


def strip_comments(text: str, ext: str) -> str:
    """Remove comment lines and blank lines for fair token comparison."""
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
    """Count tokens per file in a folder. Returns {filename: tokens}."""
    results = {}
    for ext in extensions:
        for f in sorted(folder.glob(f"*{ext}")):
            raw = f.read_text()
            cleaned = strip_comments(raw, ext)
            if not cleaned.strip():
                continue
            results[f.name] = count_tokens(cleaned)
    return results


def main():
    print("=" * 70)
    print("ilo token comparison (cl100k_base)")
    print("=" * 70)

    # Get Python baseline totals per example number
    py_folder = EXAMPLES_DIR / "python-baseline"
    py_counts = count_folder(py_folder, [".py"])
    py_total = sum(py_counts.values())

    # Print Python baseline
    print(f"\n{'python-baseline':30s}  {'tokens':>7s}  {'vs python':>10s}")
    print("-" * 52)
    for name, tokens in sorted(py_counts.items()):
        print(f"  {name:28s}  {tokens:7d}")
    print(f"  {'TOTAL':28s}  {py_total:7d}  {'1.00x':>10s}")

    # Print each idea folder
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


if __name__ == "__main__":
    main()
