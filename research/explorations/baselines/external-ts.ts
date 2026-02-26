function tot(p: number, q: number, r: number): number {
    const s = p * q;
    const t = s * r;
    return s + t;
}

const n = 10000;
for (let i = 0; i < 1000; i++) tot(i, i+1, i+2);

const start = process.hrtime.bigint();
let r = 0;
for (let i = 0; i < n; i++) r = tot(10, 20, 30);
const elapsed = Number(process.hrtime.bigint() - start);
const per = Math.floor(elapsed / n);

console.log(`result:     ${r}`);
console.log(`iterations: ${n}`);
console.log(`total:      ${(elapsed / 1e6).toFixed(2)}ms`);
console.log(`per call:   ${per}ns`);
