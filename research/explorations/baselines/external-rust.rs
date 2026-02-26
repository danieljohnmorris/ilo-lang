use std::time::Instant;

#[inline(never)]
fn tot(p: f64, q: f64, r: f64) -> f64 {
    let s = p * q;
    let t = s * r;
    s + t
}

fn main() {
    let n: u64 = 10_000_000;
    // warmup
    for i in 0..1000 {
        std::hint::black_box(tot(i as f64, (i+1) as f64, (i+2) as f64));
    }

    let start = Instant::now();
    let mut r = 0.0;
    for _ in 0..n {
        r = std::hint::black_box(tot(10.0, 20.0, 30.0));
    }
    let elapsed = start.elapsed();
    let per = elapsed.as_nanos() as f64 / n as f64;

    println!("result:     {}", r as i64);
    println!("iterations: {}", n);
    println!("total:      {:.2}ms", elapsed.as_nanos() as f64 / 1e6);
    println!("per call:   {:.1}ns", per);
}
