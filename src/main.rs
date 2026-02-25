mod ast;
mod codegen;
mod interpreter;
mod lexer;
mod parser;
mod vm;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ilo <file.ilo> [--emit python | --run [func] [args...] | --bench [func] [args...]]");
        std::process::exit(1);
    }

    let path = &args[1];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        }
    };

    let tokens = match lexer::lex(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lex error: {}", e);
            std::process::exit(1);
        }
    };

    let token_values: Vec<lexer::Token> = tokens.into_iter().map(|(t, _)| t).collect();

    let program = match parser::parse(token_values) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    // Determine mode from args
    if args.len() > 2 && args[2] == "--bench" {
        // --bench [func] [args...]
        let func_name = if args.len() > 3 { Some(args[3].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > 4 {
            args[4..].iter().map(|a| parse_cli_arg(a)).collect()
        } else {
            vec![]
        };
        run_bench(&program, func_name, &run_args);
    } else if args.len() > 2 && args[2] == "--emit" {
        if args.len() > 3 && args[3] == "python" {
            println!("{}", codegen::python::emit(&program));
        } else {
            eprintln!("Unknown emit target. Supported: python");
            std::process::exit(1);
        }
    } else if args.len() > 2 && args[2] == "--run-vm" {
        // --run-vm [func] [args...]
        let func_name = if args.len() > 3 { Some(args[3].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > 4 {
            args[4..].iter().map(|a| parse_cli_arg(a)).collect()
        } else {
            vec![]
        };

        let compiled = vm::compile(&program);
        match vm::run(&compiled, func_name, run_args) {
            Ok(val) => println!("{}", val),
            Err(e) => {
                eprintln!("VM error: {}", e);
                std::process::exit(1);
            }
        }
    } else if args.len() > 2 && args[2] == "--run" {
        // --run [func] [args...]
        let func_name = if args.len() > 3 { Some(args[3].as_str()) } else { None };
        let run_args: Vec<interpreter::Value> = if args.len() > 4 {
            args[4..].iter().map(|a| parse_cli_arg(a)).collect()
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
    } else {
        // Default: AST JSON
        match serde_json::to_string_pretty(&program) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Serialization error: {}", e);
                std::process::exit(1);
            }
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
        result = interpreter::run(program, func_name, args.to_vec()).unwrap();
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
    let compiled = vm::compile(program);
    // Warmup
    for _ in 0..100 {
        let _ = vm::run(&compiled, func_name, args.to_vec());
    }

    let start = Instant::now();
    let mut vm_result = interpreter::Value::Nil;
    for _ in 0..iterations {
        vm_result = vm::run(&compiled, func_name, args.to_vec()).unwrap();
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
        vm_result = vm_state.call(call_name, args.to_vec()).unwrap();
    }
    let vm_reuse_dur = start.elapsed();
    let vm_reuse_ns = vm_reuse_dur.as_nanos() / iterations as u128;

    println!("Register VM (reusable)");
    println!("  result:     {}", vm_result);
    println!("  iterations: {}", iterations);
    println!("  total:      {:.2}ms", vm_reuse_dur.as_nanos() as f64 / 1e6);
    println!("  per call:   {}ns", vm_reuse_ns);
    println!();

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
            std::io::stderr().write_all(&out.stderr).unwrap();
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
    }
}

fn parse_cli_arg(s: &str) -> interpreter::Value {
    if let Ok(n) = s.parse::<f64>() {
        interpreter::Value::Number(n)
    } else if s == "true" {
        interpreter::Value::Bool(true)
    } else if s == "false" {
        interpreter::Value::Bool(false)
    } else {
        interpreter::Value::Text(s.to_string())
    }
}
