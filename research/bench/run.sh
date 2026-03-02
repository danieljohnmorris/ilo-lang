#!/usr/bin/env bash
# ilo-lang benchmark suite — compare ilo JIT/VM against other languages
# Usage: ./research/bench/run.sh [iters]
# Requires: go, node, ruby, lua, luajit, rustc (via rustup)
set -euo pipefail

ITERS=${1:-10000}
DIR="$(cd "$(dirname "$0")" && pwd)"
TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

ILO="$(cd "$DIR/../.." && pwd)/target/release/ilo"
if [[ ! -x "$ILO" ]]; then
  echo "Build ilo first: cargo build --release --features cranelift"
  exit 1
fi

# ---------- Write benchmark programs ----------

# --- ilo ---
cat > "$TMP/numeric.ilo" <<< 'f n:n>n;s=0;i=1;wh <= i n{s=+s i;i=+i 1};s'
printf 'type pt{x:n;y:n}\nf n:n>n;s=0;i=0;wh < i n{yv=* i 2;p=pt x:i y:yv;s=+ s + p.x p.y;i=+ i 1};s\n' > "$TMP/record.ilo"
cat > "$TMP/string.ilo" <<< 'f n:n>t;s="";i=0;wh < i n{s=+ s "x";i=+i 1};s'
printf 'type item{name:t;val:n}\nf n:n>t;items=[];i=0;wh < i n{nm=str i;vl=* i 3;it=item name:nm val:vl;items=+=items it;i=+ i 1};jdmp items\n' > "$TMP/mixed.ilo"

# --- Helper: run one benchmark, output ns/call ---
run_ilo_jit()  { "$ILO" "$TMP/$1.ilo" --bench f "$2" 2>&1 | awk '/^Cranelift JIT/{b=1} b && /per call/{gsub(/[^0-9]/,"",$0); print; b=0}'; }
run_ilo_vm()   { "$ILO" "$TMP/$1.ilo" --bench f "$2" 2>&1 | awk '/^Register VM \(reusable\)/{b=1} b && /per call/{gsub(/[^0-9]/,"",$0); print; b=0}'; }
run_ilo_py()   { "$ILO" "$TMP/$1.ilo" --bench f "$2" 2>&1 | awk '/^Python transpiled/{b=1} b && /per call/{gsub(/[^0-9]/,"",$0); print; b=0}'; }

# --- Go ---
write_go() {
  local name=$1; shift
  cat > "$TMP/${name}.go" << GOEOF
package main
import ("fmt";"time"$([ "$name" = "mixed" ] && echo ';"encoding/json";"strconv"'))
$@
func main() {
  bench(); iters := $ITERS; start := time.Now()
  var r interface{}; for i := 0; i < iters; i++ { r = bench() }
  e := time.Since(start); fmt.Printf("%d\n", e.Nanoseconds()/int64(iters)); _ = r
}
GOEOF
}

cat > "$TMP/numeric.go" << GOEOF
package main
import ("fmt";"time")
func bench() interface{} { s := 0; for i := 1; i <= 1000; i++ { s += i }; return s }
func main() {
  bench(); iters := $ITERS; start := time.Now()
  var r interface{}; for i := 0; i < iters; i++ { r = bench() }
  e := time.Since(start); fmt.Printf("%d\n", e.Nanoseconds()/int64(iters)); _ = r
}
GOEOF

cat > "$TMP/string.go" << GOEOF
package main
import ("fmt";"time")
func bench() interface{} { s := ""; for i := 0; i < 100; i++ { s += "x" }; return s }
func main() {
  bench(); iters := $ITERS; start := time.Now()
  var r interface{}; for i := 0; i < iters; i++ { r = bench() }
  e := time.Since(start); fmt.Printf("%d\n", e.Nanoseconds()/int64(iters)); _ = r
}
GOEOF

cat > "$TMP/record.go" << GOEOF
package main
import ("fmt";"time")
type pt struct { x, y int }
func bench() interface{} { s := 0; for i := 0; i < 100; i++ { p := pt{i, i*2}; s += p.x + p.y }; return s }
func main() {
  bench(); iters := $ITERS; start := time.Now()
  var r interface{}; for i := 0; i < iters; i++ { r = bench() }
  e := time.Since(start); fmt.Printf("%d\n", e.Nanoseconds()/int64(iters)); _ = r
}
GOEOF

cat > "$TMP/mixed.go" << GOEOF
package main
import ("encoding/json";"fmt";"strconv";"time")
type item struct { Name string \`json:"name"\`; Val int \`json:"val"\` }
func bench() interface{} {
  its := make([]item, 0, 100)
  for i := 0; i < 100; i++ { its = append(its, item{strconv.Itoa(i)+"x", i*3}) }
  b, _ := json.Marshal(its); return string(b)
}
func main() {
  bench(); iters := $ITERS; start := time.Now()
  var r interface{}; for i := 0; i < iters; i++ { r = bench() }
  e := time.Since(start); fmt.Printf("%d\n", e.Nanoseconds()/int64(iters)); _ = r
}
GOEOF

# --- Rust ---
cat > "$TMP/numeric.rs" << RSEOF
use std::time::Instant; use std::hint::black_box;
fn f(n: i64) -> i64 { let mut s=0i64; let mut i=1; while i<=n { s+=i; i+=1; } s }
fn main() {
  black_box(f(black_box(1000)));
  let iters=${ITERS}_u128; let start=Instant::now();
  for _ in 0..iters { black_box(f(black_box(1000))); }
  println!("{}", start.elapsed().as_nanos()/iters);
}
RSEOF

cat > "$TMP/string.rs" << RSEOF
use std::time::Instant; use std::hint::black_box;
fn f(n: i64) -> String { let mut s=String::new(); for _ in 0..n { s.push_str("x"); } s }
fn main() {
  black_box(f(black_box(100)));
  let iters=${ITERS}_u128; let start=Instant::now();
  for _ in 0..iters { black_box(f(black_box(100))); }
  println!("{}", start.elapsed().as_nanos()/iters);
}
RSEOF

cat > "$TMP/record.rs" << RSEOF
use std::time::Instant; use std::hint::black_box;
struct Pt { x: i64, y: i64 }
fn f(n: i64) -> i64 { let mut s=0i64; for i in 0..n { let p=Pt{x:i,y:i*2}; s+=p.x+p.y; } s }
fn main() {
  black_box(f(black_box(100)));
  let iters=${ITERS}_u128; let start=Instant::now();
  for _ in 0..iters { black_box(f(black_box(100))); }
  println!("{}", start.elapsed().as_nanos()/iters);
}
RSEOF

cat > "$TMP/mixed.rs" << RSEOF
use std::time::Instant; use std::hint::black_box;
struct Item { name: String, val: i64 }
fn f(n: i64) -> String {
  let mut its=Vec::with_capacity(n as usize);
  for i in 0..n { its.push(Item{name:format!("{}x",i),val:i*3}); }
  let mut o=String::from("[");
  for (j,it) in its.iter().enumerate() { if j>0{o.push(',');} o.push_str(&format!("{{\"name\":\"{}\",\"val\":{}}}",it.name,it.val)); }
  o.push(']'); o
}
fn main() {
  black_box(f(black_box(100)));
  let iters=${ITERS}_u128; let start=Instant::now();
  for _ in 0..iters { black_box(f(black_box(100))); }
  println!("{}", start.elapsed().as_nanos()/iters);
}
RSEOF

# --- Node/V8 ---
cat > "$TMP/numeric.js" << JSEOF
function f(n){let s=0,i=1;while(i<=n){s+=i;i++}return s}
f(1000);const I=$ITERS,t=performance.now();let r;for(let i=0;i<I;i++)r=f(1000);
console.log(Math.round((performance.now()-t)*1e6/I));
JSEOF

cat > "$TMP/string.js" << JSEOF
function f(n){let s="";for(let i=0;i<n;i++)s+="x";return s}
f(100);const I=$ITERS,t=performance.now();let r;for(let i=0;i<I;i++)r=f(100);
console.log(Math.round((performance.now()-t)*1e6/I));
JSEOF

cat > "$TMP/record.js" << JSEOF
function f(n){let s=0;for(let i=0;i<n;i++){const p={x:i,y:i*2};s+=p.x+p.y}return s}
f(100);const I=$ITERS,t=performance.now();let r;for(let i=0;i<I;i++)r=f(100);
console.log(Math.round((performance.now()-t)*1e6/I));
JSEOF

cat > "$TMP/mixed.js" << JSEOF
function f(n){const a=[];for(let i=0;i<n;i++)a.push({name:i+"x",val:i*3});return JSON.stringify(a)}
f(100);const I=$ITERS,t=performance.now();let r;for(let i=0;i<I;i++)r=f(100);
console.log(Math.round((performance.now()-t)*1e6/I));
JSEOF

# --- Ruby ---
for bench in numeric string record mixed; do
  case $bench in
    numeric) cat > "$TMP/$bench.rb" << RBEOF
def f(n);s=0;i=1;while i<=n;s+=i;i+=1;end;s;end
f(1000);iters=$ITERS;t=Process.clock_gettime(Process::CLOCK_MONOTONIC)
iters.times{f(1000)};e=Process.clock_gettime(Process::CLOCK_MONOTONIC)-t
puts (e*1e9/iters).to_i
RBEOF
    ;;
    string) cat > "$TMP/$bench.rb" << RBEOF
def f(n);s="";i=0;while i<n;s+="x";i+=1;end;s;end
f(100);iters=$ITERS;t=Process.clock_gettime(Process::CLOCK_MONOTONIC)
iters.times{f(100)};e=Process.clock_gettime(Process::CLOCK_MONOTONIC)-t
puts (e*1e9/iters).to_i
RBEOF
    ;;
    record) cat > "$TMP/$bench.rb" << RBEOF
Pt=Struct.new(:x,:y)
def f(n);s=0;i=0;while i<n;p=Pt.new(i,i*2);s+=p.x+p.y;i+=1;end;s;end
f(100);iters=$ITERS;t=Process.clock_gettime(Process::CLOCK_MONOTONIC)
iters.times{f(100)};e=Process.clock_gettime(Process::CLOCK_MONOTONIC)-t
puts (e*1e9/iters).to_i
RBEOF
    ;;
    mixed) cat > "$TMP/$bench.rb" << RBEOF
require 'json'
def f(n);a=[];i=0;while i<n;a<<{name:"#{i}x",val:i*3};i+=1;end;JSON.generate(a);end
f(100);iters=$ITERS;t=Process.clock_gettime(Process::CLOCK_MONOTONIC)
iters.times{f(100)};e=Process.clock_gettime(Process::CLOCK_MONOTONIC)-t
puts (e*1e9/iters).to_i
RBEOF
    ;;
  esac
done

# --- Lua ---
cat > "$TMP/numeric.lua" << LUAEOF
local function f(n) local s=0; for i=1,n do s=s+i end; return s end
f(1000); local iters=$ITERS; local t=os.clock()
for _=1,iters do f(1000) end
print(math.floor((os.clock()-t)*1e9/iters))
LUAEOF

cat > "$TMP/string.lua" << LUAEOF
local function f(n) local s=""; for _=1,n do s=s.."x" end; return s end
f(100); local iters=$ITERS; local t=os.clock()
for _=1,iters do f(100) end
print(math.floor((os.clock()-t)*1e9/iters))
LUAEOF

cat > "$TMP/record.lua" << LUAEOF
local function f(n) local s=0; for i=0,n-1 do local p={x=i,y=i*2}; s=s+p.x+p.y end; return s end
f(100); local iters=$ITERS; local t=os.clock()
for _=1,iters do f(100) end
print(math.floor((os.clock()-t)*1e9/iters))
LUAEOF

cat > "$TMP/mixed.lua" << LUAEOF
local function jenc(items)
  local p={}; for _,it in ipairs(items) do p[#p+1]=string.format('{"name":"%s","val":%d}',it.name,it.val) end
  return "["..table.concat(p,",").."]"
end
local function f(n)
  local a={}; for i=0,n-1 do a[#a+1]={name=i.."x",val=i*3} end; return jenc(a)
end
f(100); local iters=$ITERS; local t=os.clock()
for _=1,iters do f(100) end
print(math.floor((os.clock()-t)*1e9/iters))
LUAEOF

# ---------- Compile ----------
echo "Compiling..." >&2
for bench in numeric string record mixed; do
  go build -o "$TMP/${bench}_go" "$TMP/$bench.go" &
done
for bench in numeric string record mixed; do
  rustup run stable rustc -O "$TMP/$bench.rs" -o "$TMP/${bench}_rs" 2>/dev/null &
done
wait

# ---------- Run ----------
echo "Running benchmarks ($ITERS iterations)..." >&2
echo ""

# Collect all results into a flat file: bench lang ns
RESULTS="$TMP/results.txt"
> "$RESULTS"

for spec in numeric:1000 string:100 record:100 mixed:100; do
  bench="${spec%%:*}"
  arg="${spec##*:}"
  echo "  $bench..." >&2

  echo "$bench rust $("$TMP/${bench}_rs")" >> "$RESULTS"
  echo "$bench go $("$TMP/${bench}_go")" >> "$RESULTS"
  echo "$bench luajit $(luajit "$TMP/$bench.lua")" >> "$RESULTS"
  echo "$bench node $(node "$TMP/$bench.js")" >> "$RESULTS"
  echo "$bench ilo_jit $(run_ilo_jit "$bench" "$arg")" >> "$RESULTS"
  echo "$bench ilo_vm $(run_ilo_vm "$bench" "$arg")" >> "$RESULTS"
  echo "$bench lua $(lua "$TMP/$bench.lua")" >> "$RESULTS"
  echo "$bench ruby $(ruby "$TMP/$bench.rb")" >> "$RESULTS"
  echo "$bench python $(run_ilo_py "$bench" "$arg")" >> "$RESULTS"
done

# ---------- Format ----------
fmt_ns() {
  local ns=$1
  if [ "$ns" -ge 1000000 ] 2>/dev/null; then
    awk "BEGIN{printf \"%.1fms\", $ns/1000000}"
  elif [ "$ns" -ge 1000 ] 2>/dev/null; then
    awk "BEGIN{printf \"%.1fus\", $ns/1000}"
  else
    printf "%dns" "$ns"
  fi
}

get_result() {
  awk -v b="$1" -v l="$2" '$1==b && $2==l {print $3}' "$RESULTS"
}

printf "%-14s  %-12s  %-12s  %-12s  %-12s\n" "Language" "numeric" "string" "record" "mixed"
printf "%-14s  %-12s  %-12s  %-12s  %-12s\n" "--------------" "------------" "------------" "------------" "------------"

for lang in rust go luajit node ilo_jit ilo_vm lua ruby python; do
  case $lang in
    rust)    label="Rust (native)" ;;
    go)      label="Go" ;;
    luajit)  label="LuaJIT" ;;
    node)    label="Node/V8" ;;
    ilo_jit) label="ilo JIT" ;;
    ilo_vm)  label="ilo VM" ;;
    lua)     label="Lua" ;;
    ruby)    label="Ruby" ;;
    python)  label="Python (ilo)" ;;
  esac
  printf "%-14s" "$label"
  for bench in numeric string record mixed; do
    ns=$(get_result "$bench" "$lang")
    printf "  %-12s" "$(fmt_ns "$ns")"
  done
  echo ""
done

echo ""
echo "$ITERS iterations | $(date '+%Y-%m-%d %H:%M') | $(uname -ms)"
