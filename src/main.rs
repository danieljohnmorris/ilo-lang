#![warn(clippy::all)]

mod ast;
mod codegen;
mod diagnostic;
mod interpreter;
mod lexer;
mod parser;
mod verify;
mod vm;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ilo <file-or-code> [args... | --run func args... | --bench func args... | --emit python]");
        eprintln!("       ilo help | -h     Show usage and examples");
        eprintln!("       ilo help lang     Show language specification");
        std::process::exit(1);
    }

    if args[1] == "--version" || args[1] == "-V" {
        println!("ilo {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    if args[1] == "help" || args[1] == "--help" || args[1] == "-h" {
        if args.len() > 2 && args[2] == "lang" {
            print!("{}", include_str!("../SPEC.md"));
        } else {
            println!("ilo — a constructed language for AI agents\n");
            println!("Usage:");
            println!("  ilo <code> [args...]              Run (Cranelift JIT, falls back to interpreter)");
            println!("  ilo <file.ilo> [args...]          Run from file");
            println!("  ilo <code> func [args...]         Run a specific function");
            println!("  ilo <code> --emit python          Transpile to Python");
            println!("  ilo <code>                        Print AST as JSON (no args)");
            println!("  ilo <code> --bench func [args...] Benchmark a function");
            println!("  ilo help lang                     Show language specification\n");
            println!("Backends:");
            println!("  (default)        Cranelift JIT → interpreter fallback");
            println!("  --run-interp     Tree-walking interpreter");
            println!("  --run-vm         Register VM");
            println!("  --run-cranelift  Cranelift JIT");
            println!("  --run-jit        Custom ARM64 JIT (macOS Apple Silicon only)");
            println!("  --run-llvm       LLVM JIT (requires --features llvm build)\n");
            println!("Examples:");
            println!("  ilo 'f x:n>n;*x 2' 5             Define and call f(5) → 10");
            println!("  ilo 'f xs:L n>n;len xs' 1,2,3     Pass a list → 3");
            println!("  ilo program.ilo 10 20             Run file with arguments");
            println!("  ilo 'f x:n>n;*x 2' --emit python Transpile to Python");
        }
        std::process::exit(0);
    }

    // If args[1] is a file that exists, read it. Otherwise treat it as inline code.
    let (source, mode_args_start) = if std::path::Path::new(&args[1]).is_file() {
        let s = match std::fs::read_to_string(&args[1]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", args[1], e);
                std::process::exit(1);
            }
        };
        (s, 2)
    } else if args[1] == "-e" {
        // Legacy -e flag: skip it, use args[2] as code
        if args.len() < 3 || args[2].is_empty() {
            eprintln!("Usage: ilo <file-or-code> [args... | --run func args... | --emit python]");
            std::process::exit(1);
        }
        (args[2].clone(), 3)
    } else {
        let code = &args[1];
        if code.is_empty() {
            eprintln!("Error: empty code string");
            std::process::exit(1);
        }
        (code.clone(), 2)
    };

    let tokens = match lexer::lex(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lex error: {}", e);
            std::process::exit(1);
        }
    };

    let token_spans: Vec<(lexer::Token, ast::Span)> = tokens
        .into_iter()
        .map(|(t, r)| (t, ast::Span { start: r.start, end: r.end }))
        .collect();

    let mut program = match parser::parse(token_spans) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    program.source = Some(source.clone());

    if let Err(errors) = verify::verify(&program) {
        for e in &errors {
            eprintln!("{}", e);
        }
        std::process::exit(1);
    }

    // Determine mode from args
    let m = mode_args_start;
    if args.len() > m && args[m] == "--bench" {
        // --bench [func] [args...]
        let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > m + 2 {
            args[m + 2..].iter().map(|a| parse_cli_arg(a)).collect()
        } else {
            vec![]
        };
        run_bench(&program, func_name, &run_args);
    } else if args.len() > m && args[m] == "--emit" {
        if args.len() > m + 1 && args[m + 1] == "python" {
            println!("{}", codegen::python::emit(&program));
        } else {
            eprintln!("Unknown emit target. Supported: python");
            std::process::exit(1);
        }
    } else if args.len() > m && args[m] == "--run-jit" {
        // --run-jit [func] [args...] — ARM64 JIT (aarch64 only)
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        {
            let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
            let run_args: Vec<f64> = if args.len() > m + 2 {
                args[m + 2..].iter().map(|a| a.parse::<f64>().expect("JIT args must be numbers")).collect()
            } else {
                vec![]
            };

            let compiled = vm::compile(&program).unwrap_or_else(|e| { eprintln!("Compile error: {}", e); std::process::exit(1); });
            let target = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
            let func_idx = compiled.func_names.iter().position(|n| n == target)
                .unwrap_or_else(|| { eprintln!("undefined function: {}", target); std::process::exit(1); });
            let chunk = &compiled.chunks[func_idx];
            let nan_consts = &compiled.nan_constants[func_idx];

            match vm::jit_arm64::compile_and_call(chunk, nan_consts, &run_args) {
                Some(result) => {
                    if result == (result as i64) as f64 {
                        println!("{}", result as i64);
                    } else {
                        println!("{}", result);
                    }
                }
                None => {
                    eprintln!("JIT: function not eligible for compilation (numeric-only required)");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
        {
            eprintln!("Custom JIT (arm64) is only available on aarch64 macOS");
            std::process::exit(1);
        }
    } else if args.len() > m && args[m] == "--run-cranelift" {
        // --run-cranelift [func] [args...]
        #[cfg(feature = "cranelift")]
        {
            let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
            let run_args: Vec<f64> = if args.len() > m + 2 {
                args[m + 2..].iter().map(|a| a.parse::<f64>().expect("JIT args must be numbers")).collect()
            } else {
                vec![]
            };

            let compiled = vm::compile(&program).unwrap_or_else(|e| { eprintln!("Compile error: {}", e); std::process::exit(1); });
            let target = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
            let func_idx = compiled.func_names.iter().position(|n| n == target)
                .unwrap_or_else(|| { eprintln!("undefined function: {}", target); std::process::exit(1); });
            let chunk = &compiled.chunks[func_idx];
            let nan_consts = &compiled.nan_constants[func_idx];

            match vm::jit_cranelift::compile_and_call(chunk, nan_consts, &run_args) {
                Some(result) => {
                    if result == (result as i64) as f64 {
                        println!("{}", result as i64);
                    } else {
                        println!("{}", result);
                    }
                }
                None => {
                    eprintln!("Cranelift JIT: function not eligible for compilation");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(not(feature = "cranelift"))]
        {
            eprintln!("Cranelift JIT not enabled. Build with: cargo build --features cranelift");
            std::process::exit(1);
        }
    } else if args.len() > m && args[m] == "--run-llvm" {
        // --run-llvm [func] [args...]
        #[cfg(feature = "llvm")]
        {
            let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
            let run_args: Vec<f64> = if args.len() > m + 2 {
                args[m + 2..].iter().map(|a| a.parse::<f64>().expect("JIT args must be numbers")).collect()
            } else {
                vec![]
            };

            let compiled = vm::compile(&program).unwrap_or_else(|e| { eprintln!("Compile error: {}", e); std::process::exit(1); });
            let target = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
            let func_idx = compiled.func_names.iter().position(|n| n == target)
                .unwrap_or_else(|| { eprintln!("undefined function: {}", target); std::process::exit(1); });
            let chunk = &compiled.chunks[func_idx];
            let nan_consts = &compiled.nan_constants[func_idx];

            match vm::jit_llvm::compile_and_call(chunk, nan_consts, &run_args) {
                Some(result) => {
                    if result == (result as i64) as f64 {
                        println!("{}", result as i64);
                    } else {
                        println!("{}", result);
                    }
                }
                None => {
                    eprintln!("LLVM JIT: function not eligible for compilation");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            eprintln!("LLVM JIT not enabled. Build with: cargo build --features llvm");
            std::process::exit(1);
        }
    } else if args.len() > m && args[m] == "--run-vm" {
        // --run-vm [func] [args...]
        let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > m + 2 {
            args[m + 2..].iter().map(|a| parse_cli_arg(a)).collect()
        } else {
            vec![]
        };

        let compiled = vm::compile(&program).unwrap_or_else(|e| { eprintln!("Compile error: {}", e); std::process::exit(1); });
        match vm::run(&compiled, func_name, run_args) {
            Ok(val) => println!("{}", val),
            Err(e) => {
                eprintln!("VM error: {}", e);
                std::process::exit(1);
            }
        }
    } else if args.len() > m && (args[m] == "--run" || args[m] == "--run-interp") {
        // --run / --run-interp [func] [args...]
        let func_name = if args.len() > m + 1 { Some(args[m + 1].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > m + 2 {
            args[m + 2..].iter().map(|a| parse_cli_arg(a)).collect()
        } else {
            vec![]
        };

        match interpreter::run(&program, func_name, run_args) {
            Ok(val) => println!("{}", val),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else if args.len() > m {
        // Bare args: default = Cranelift JIT, fall back to interpreter
        let func_names: Vec<&str> = program.declarations.iter().filter_map(|d| match d {
            ast::Decl::Function { name, .. } => Some(name.as_str()),
            _ => None,
        }).collect();

        let (func_name, run_args_start) = if func_names.contains(&args[m].as_str()) {
            (Some(args[m].as_str()), m + 1)
        } else {
            (None, m)
        };

        let run_args: Vec<interpreter::Value> = args[run_args_start..].iter().map(|a| parse_cli_arg(a)).collect();
        run_default(&program, func_name, run_args);
    } else {
        // No args: AST JSON
        match serde_json::to_string_pretty(&program) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run_default(program: &ast::Program, func_name: Option<&str>, args: Vec<interpreter::Value>) {
    // Try Cranelift JIT first: requires all-numeric args and JIT-eligible function
    #[cfg(feature = "cranelift")]
    {
        let jit_args: Vec<f64> = args.iter().filter_map(|a| match a {
            interpreter::Value::Number(n) => Some(*n),
            _ => None,
        }).collect();

        if jit_args.len() == args.len()
            && let Ok(compiled) = vm::compile(program) {
                let target = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
                if let Some(func_idx) = compiled.func_names.iter().position(|n| n == target) {
                    let chunk = &compiled.chunks[func_idx];
                    let nan_consts = &compiled.nan_constants[func_idx];
                    if let Some(result) = vm::jit_cranelift::compile_and_call(chunk, nan_consts, &jit_args) {
                        if result == (result as i64) as f64 {
                            println!("{}", result as i64);
                        } else {
                            println!("{}", result);
                        }
                        return;
                    }
                }
            }
    }

    // Fall back to interpreter
    match interpreter::run(program, func_name, args) {
        Ok(val) => println!("{}", val),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn run_bench(program: &ast::Program, func_name: Option<&str>, args: &[interpreter::Value]) {
    use std::time::Instant;
    use std::io::Write;
    use std::process::Command;

    let iterations: u32 = 10_000;

    // -- Rust interpreter benchmark --
    // Warmup
    for _ in 0..100 {
        let _ = interpreter::run(program, func_name, args.to_vec());
    }

    let start = Instant::now();
    let mut result = interpreter::Value::Nil;
    for _ in 0..iterations {
        result = interpreter::run(program, func_name, args.to_vec()).expect("interpreter error during benchmark");
    }
    let interp_dur = start.elapsed();
    let interp_ns = interp_dur.as_nanos() / iterations as u128;

    println!("Rust interpreter");
    println!("  result:     {}", result);
    println!("  iterations: {}", iterations);
    println!("  total:      {:.2}ms", interp_dur.as_nanos() as f64 / 1e6);
    println!("  per call:   {}ns", interp_ns);
    println!();

    // -- Register VM benchmark --
    let compiled = vm::compile(program).expect("compile error in benchmark");
    // Warmup
    for _ in 0..100 {
        let _ = vm::run(&compiled, func_name, args.to_vec());
    }

    let start = Instant::now();
    let mut vm_result = interpreter::Value::Nil;
    for _ in 0..iterations {
        vm_result = vm::run(&compiled, func_name, args.to_vec()).expect("VM error during benchmark");
    }
    let vm_dur = start.elapsed();
    let vm_ns = vm_dur.as_nanos() / iterations as u128;

    println!("Register VM");
    println!("  result:     {}", vm_result);
    println!("  iterations: {}", iterations);
    println!("  total:      {:.2}ms", vm_dur.as_nanos() as f64 / 1e6);
    println!("  per call:   {}ns", vm_ns);
    println!();

    // -- Register VM (reusable) benchmark --
    let call_name = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
    let mut vm_state = vm::VmState::new(&compiled);
    for _ in 0..100 {
        let _ = vm_state.call(call_name, args.to_vec());
    }

    let start = Instant::now();
    for _ in 0..iterations {
        vm_result = vm_state.call(call_name, args.to_vec()).expect("VM reusable error during benchmark");
    }
    let vm_reuse_dur = start.elapsed();
    let vm_reuse_ns = vm_reuse_dur.as_nanos() / iterations as u128;

    println!("Register VM (reusable)");
    println!("  result:     {}", vm_result);
    println!("  iterations: {}", iterations);
    println!("  total:      {:.2}ms", vm_reuse_dur.as_nanos() as f64 / 1e6);
    println!("  per call:   {}ns", vm_reuse_ns);
    println!();

    // -- JIT benchmarks --
    // Extract function info for JIT
    let call_name_jit = func_name.unwrap_or(compiled.func_names.first().map(|s| s.as_str()).unwrap_or("main"));
    let func_idx_jit = compiled.func_names.iter().position(|n| n == call_name_jit);
    let jit_args: Vec<f64> = args.iter().filter_map(|a| match a {
        interpreter::Value::Number(n) => Some(*n),
        _ => None,
    }).collect();
    let all_numeric = jit_args.len() == args.len();

    let mut jit_arm64_ns: Option<u128> = None;
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    if let Some(fi) = func_idx_jit
        && all_numeric {
            let chunk = &compiled.chunks[fi];
            let nan_consts = &compiled.nan_constants[fi];
            if let Some(jit_func) = vm::jit_arm64::compile(chunk, nan_consts) {
                // Warmup
                for _ in 0..100 {
                    let _ = vm::jit_arm64::call(&jit_func, &jit_args);
                }

                let start = Instant::now();
                let mut jit_result = 0.0f64;
                for _ in 0..iterations {
                    jit_result = vm::jit_arm64::call(&jit_func, &jit_args).expect("arm64 JIT error during benchmark");
                }
                let jit_dur = start.elapsed();
                let ns = jit_dur.as_nanos() / iterations as u128;
                jit_arm64_ns = Some(ns);

                println!("Custom JIT (arm64)");
                if jit_result == (jit_result as i64) as f64 {
                    println!("  result:     {}", jit_result as i64);
                } else {
                    println!("  result:     {}", jit_result);
                }
                println!("  iterations: {}", iterations);
                println!("  total:      {:.2}ms", jit_dur.as_nanos() as f64 / 1e6);
                println!("  per call:   {}ns", ns);
                println!();
            }
        }

    let mut jit_cranelift_ns: Option<u128> = None;
    #[cfg(feature = "cranelift")]
    if let Some(fi) = func_idx_jit
        && all_numeric {
            let chunk = &compiled.chunks[fi];
            let nan_consts = &compiled.nan_constants[fi];
            if let Some(jit_func) = vm::jit_cranelift::compile(chunk, nan_consts) {
                for _ in 0..100 {
                    let _ = vm::jit_cranelift::call(&jit_func, &jit_args);
                }

                let start = Instant::now();
                let mut jit_result = 0.0f64;
                for _ in 0..iterations {
                    jit_result = vm::jit_cranelift::call(&jit_func, &jit_args).expect("Cranelift JIT error during benchmark");
                }
                let jit_dur = start.elapsed();
                let ns = jit_dur.as_nanos() / iterations as u128;
                jit_cranelift_ns = Some(ns);

                println!("Cranelift JIT");
                if jit_result == (jit_result as i64) as f64 {
                    println!("  result:     {}", jit_result as i64);
                } else {
                    println!("  result:     {}", jit_result);
                }
                println!("  iterations: {}", iterations);
                println!("  total:      {:.2}ms", jit_dur.as_nanos() as f64 / 1e6);
                println!("  per call:   {}ns", ns);
                println!();
            }
        }

    #[allow(unused_variables)]
    let jit_llvm_ns: Option<u128> = None;
    #[cfg(feature = "llvm")]
    if let Some(fi) = func_idx_jit {
        if all_numeric {
            let chunk = &compiled.chunks[fi];
            let nan_consts = &compiled.nan_constants[fi];
            if let Some(jit_func) = vm::jit_llvm::compile(chunk, nan_consts) {
                for _ in 0..100 {
                    let _ = vm::jit_llvm::call(&jit_func, &jit_args);
                }

                let start = Instant::now();
                let mut jit_result = 0.0f64;
                for _ in 0..iterations {
                    jit_result = vm::jit_llvm::call(&jit_func, &jit_args).expect("LLVM JIT error during benchmark");
                }
                let jit_dur = start.elapsed();
                let ns = jit_dur.as_nanos() / iterations as u128;
                jit_llvm_ns = Some(ns);

                println!("LLVM JIT");
                if jit_result == (jit_result as i64) as f64 {
                    println!("  result:     {}", jit_result as i64);
                } else {
                    println!("  result:     {}", jit_result);
                }
                println!("  iterations: {}", iterations);
                println!("  total:      {:.2}ms", jit_dur.as_nanos() as f64 / 1e6);
                println!("  per call:   {}ns", ns);
                println!();
            }
        }
    }

    // -- Python transpiler benchmark (single invocation) --
    let py_code = codegen::python::emit(program);
    let call_func = func_name.unwrap_or("main").replace('-', "_");
    let call_args: Vec<String> = args.iter().map(|a| match a {
        interpreter::Value::Number(n) => {
            if *n == (*n as i64) as f64 { format!("{}", *n as i64) } else { format!("{}", n) }
        }
        interpreter::Value::Text(s) => format!("\"{}\"", s),
        interpreter::Value::Bool(b) => if *b { "True".to_string() } else { "False".to_string() },
        _ => "None".to_string(),
    }).collect();

    // Python script: prints human-readable lines then a final __NS__=<value> for parsing
    let py_script = format!(
        r#"import time
{code}
_n = {n}
for _ in range(100):
    {func}({args})
_start = time.perf_counter_ns()
for _ in range(_n):
    _r = {func}({args})
_elapsed = time.perf_counter_ns() - _start
_per = _elapsed // _n
print(f"result:     {{_r}}")
print(f"iterations: {{_n}}")
print(f"total:      {{_elapsed / 1e6:.2f}}ms")
print(f"per call:   {{_per}}ns")
print(f"__NS__={{_per}}")
"#,
        code = py_code,
        n = iterations,
        func = call_func,
        args = call_args.join(", ")
    );

    println!("Python transpiled");
    let output = Command::new("python3")
        .arg("-c")
        .arg(&py_script)
        .output();

    let mut py_ns: Option<u128> = None;
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if let Some(val) = line.strip_prefix("__NS__=") {
                    py_ns = val.parse().ok();
                } else {
                    println!("  {}", line);
                }
            }
            std::io::stderr().write_all(&out.stderr).expect("write to stderr");
        }
        Err(e) => eprintln!("  failed to run python3: {}", e),
    }

    println!();

    // -- Summary --
    println!("Summary");
    if vm_ns > 0 && interp_ns > 0 {
        if vm_ns < interp_ns {
            println!("  Register VM is {:.1}x faster than interpreter", interp_ns as f64 / vm_ns as f64);
        } else {
            println!("  Interpreter is {:.1}x faster than bytecode VM", vm_ns as f64 / interp_ns as f64);
        }
    }
    if let Some(jit_ns) = jit_arm64_ns
        && jit_ns > 0 && vm_reuse_ns > 0 {
            println!("  Custom JIT (arm64) is {:.1}x faster than VM (reusable)", vm_reuse_ns as f64 / jit_ns as f64);
        }
    if let Some(jit_ns) = jit_cranelift_ns
        && jit_ns > 0 && vm_reuse_ns > 0 {
            println!("  Cranelift JIT is {:.1}x faster than VM (reusable)", vm_reuse_ns as f64 / jit_ns as f64);
        }
    if let Some(jit_ns) = jit_llvm_ns
        && jit_ns > 0 && vm_reuse_ns > 0 {
            println!("  LLVM JIT is {:.1}x faster than VM (reusable)", vm_reuse_ns as f64 / jit_ns as f64);
        }
    if let Some(py) = py_ns {
        if interp_ns > 0 && py > 0 {
            if interp_ns < py {
                println!("  Rust interpreter is {:.1}x faster than Python", py as f64 / interp_ns as f64);
            } else {
                println!("  Python is {:.1}x faster than Rust interpreter", interp_ns as f64 / py as f64);
            }
        }
        if vm_ns > 0 && py > 0 {
            if vm_ns < py {
                println!("  Register VM is {:.1}x faster than Python", py as f64 / vm_ns as f64);
            } else {
                println!("  Python is {:.1}x faster than Register VM", vm_ns as f64 / py as f64);
            }
        }
        if vm_reuse_ns > 0 && py > 0 {
            if vm_reuse_ns < py {
                println!("  VM (reusable) is {:.1}x faster than Python", py as f64 / vm_reuse_ns as f64);
            } else {
                println!("  Python is {:.1}x faster than VM (reusable)", vm_reuse_ns as f64 / py as f64);
            }
        }
        if let Some(jit_ns) = jit_arm64_ns
            && jit_ns > 0 && py > 0 {
                println!("  Custom JIT (arm64) is {:.1}x faster than Python", py as f64 / jit_ns as f64);
            }
        if let Some(jit_ns) = jit_cranelift_ns
            && jit_ns > 0 && py > 0 {
                println!("  Cranelift JIT is {:.1}x faster than Python", py as f64 / jit_ns as f64);
            }
        if let Some(jit_ns) = jit_llvm_ns
            && jit_ns > 0 && py > 0 {
                println!("  LLVM JIT is {:.1}x faster than Python", py as f64 / jit_ns as f64);
            }
    }
}

fn parse_cli_arg(s: &str) -> interpreter::Value {
    // Bracketed list: [1,2,3] or []
    if s.starts_with('[') && s.ends_with(']') {
        let inner = s[1..s.len()-1].trim();
        if inner.is_empty() {
            return interpreter::Value::List(vec![]);
        }
        let items = inner.split(',').map(|part| parse_cli_arg(part.trim())).collect();
        return interpreter::Value::List(items);
    }
    // Bare comma list: 1,2,3
    if s.contains(',') {
        let items = s.split(',').map(|part| parse_cli_arg(part.trim())).collect();
        return interpreter::Value::List(items);
    }
    if let Ok(n) = s.parse::<f64>()
        && n.is_finite() {
            return interpreter::Value::Number(n);
        }
    if s == "true" {
        interpreter::Value::Bool(true)
    } else if s == "false" {
        interpreter::Value::Bool(false)
    } else {
        interpreter::Value::Text(s.to_string())
    }
}
