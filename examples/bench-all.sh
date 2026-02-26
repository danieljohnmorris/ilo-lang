#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# ── Ensure cargo/rustc are in PATH ──────────────────────────────────
if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

# ── Config ──────────────────────────────────────────────────────────
ILO=./target/release/ilo
ARGS="10 20 30"
PYARGS="10, 20, 30"
EXPECTED=6200

# ── Helpers ─────────────────────────────────────────────────────────
check_cmd() { command -v "$1" >/dev/null 2>&1; }

section() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "  $1"
    echo "═══════════════════════════════════════════════════════════"
}

skip() {
    echo "  [SKIP] $1 not found"
    echo ""
}

# ── Build ilo release ───────────────────────────────────────────────
section "Building ilo (release)"

if check_cmd rustc; then
    if cargo build --release --features cranelift 2>/dev/null; then
        echo "  Built with cranelift"
    else
        cargo build --release
        echo "  Built without cranelift"
    fi
else
    echo "  [SKIP] rustc not found, using existing binary"
fi

if [[ ! -x "$ILO" ]]; then
    echo "  ERROR: $ILO not found. Build ilo first."
    exit 1
fi

# ── Compile AOT languages ──────────────────────────────────────────
section "Compiling AOT benchmarks"

if check_cmd cc; then
    cc -O2 -o /tmp/bench-c examples/bench-native.c
    echo "  C      → /tmp/bench-c"
else
    echo "  [SKIP] cc not found"
fi

if check_cmd rustc; then
    rustc -O -o /tmp/bench-rs examples/bench-native.rs
    echo "  Rust   → /tmp/bench-rs"
else
    echo "  [SKIP] rustc not found"
fi

if check_cmd go; then
    go build -o /tmp/bench-go examples/bench-go.go
    echo "  Go     → /tmp/bench-go"
else
    echo "  [SKIP] go not found"
fi

# ── Section 1: ilo ideas ───────────────────────────────────────────
section "ilo — idea8 (ultra-dense)"
$ILO examples/idea8-ultra-dense/01-simple-function.ilo --bench total $ARGS

section "ilo — idea9 (ultra-dense-short)"
$ILO examples/idea9-ultra-dense-short/01-simple-function.ilo --bench tot $ARGS

# ── Section 2: Interpreted ─────────────────────────────────────────
section "External — Interpreted"

echo "--- Python 3 (CPython) ---"
if check_cmd python3; then
    python3 -c "
import time
def tot(p, q, r):
    s = p * q
    t = s * r
    return s + t
n = 10000
for i in range(1000): tot(i, i+1, i+2)
start = time.monotonic_ns()
r = 0
for i in range(n): r = tot($PYARGS)
elapsed = time.monotonic_ns() - start
per = elapsed // n
print(f'result:     {r}')
print(f'iterations: {n}')
print(f'total:      {elapsed / 1e6:.2f}ms')
print(f'per call:   {per}ns')
"
else
    skip "python3"
fi
echo ""

echo "--- Ruby ---"
if check_cmd ruby; then
    ruby examples/bench-ruby.rb
else
    skip "ruby"
fi
echo ""

echo "--- PHP ---"
if check_cmd php; then
    php examples/bench-php.php
else
    skip "php"
fi
echo ""

echo "--- Lua ---"
if check_cmd lua; then
    lua examples/bench-lua.lua
else
    skip "lua"
fi

# ── Section 3: JIT ─────────────────────────────────────────────────
section "External — JIT"

echo "--- Node.js (V8) ---"
if check_cmd node; then
    node examples/bench-v8.js
else
    skip "node"
fi
echo ""

echo "--- LuaJIT ---"
if check_cmd luajit; then
    luajit examples/bench-luajit.lua
else
    skip "luajit"
fi
echo ""

echo "--- PyPy3 ---"
if check_cmd pypy3; then
    pypy3 -c "
import time
def tot(p, q, r):
    s = p * q
    t = s * r
    return s + t
n = 10000
for i in range(1000): tot(i, i+1, i+2)
start = time.monotonic_ns()
r = 0
for i in range(n): r = tot($PYARGS)
elapsed = time.monotonic_ns() - start
per = elapsed // n
print(f'result:     {r}')
print(f'iterations: {n}')
print(f'total:      {elapsed / 1e6:.2f}ms')
print(f'per call:   {per}ns')
"
else
    skip "pypy3"
fi

# ── Section 4: AOT ─────────────────────────────────────────────────
section "External — AOT (compiled)"

echo "--- C (cc -O2) ---"
if [[ -x /tmp/bench-c ]]; then
    /tmp/bench-c
else
    skip "bench-c (not compiled)"
fi
echo ""

echo "--- Rust (rustc -O) ---"
if [[ -x /tmp/bench-rs ]]; then
    /tmp/bench-rs
else
    skip "bench-rs (not compiled)"
fi
echo ""

echo "--- Go ---"
if [[ -x /tmp/bench-go ]]; then
    /tmp/bench-go
else
    skip "bench-go (not compiled)"
fi

# ── Done ────────────────────────────────────────────────────────────
section "Done"
echo "  All benchmarks complete. Expected result: $EXPECTED"
echo ""
