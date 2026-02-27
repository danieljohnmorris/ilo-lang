use std::process::Command;

fn ilo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ilo"))
}

// --- Inline code: single function ---

#[test]
fn inline_single_func_bare_args() {
    let out = ilo()
        .args(["tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "10", "20", "30"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6200");
}

#[test]
fn inline_no_args_outputs_ast() {
    let out = ilo()
        .args(["tot p:n q:n r:n>n;s=*p q;t=*s r;+s t"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"name\""), "expected AST JSON, got: {}", stdout);
}

// --- Inline code: multiple functions ---

#[test]
fn inline_multi_func_select_by_name() {
    let out = ilo()
        .args(["dbl x:n>n;s=*x 2;+s 0 tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "tot", "10", "20", "30"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6200");
}

#[test]
fn inline_multi_func_first_by_default() {
    let out = ilo()
        .args(["dbl x:n>n;s=*x 2;+s 0 tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "5"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10");
}

// --- Inline code: emit ---

#[test]
fn inline_emit_python() {
    let out = ilo()
        .args(["tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "--emit", "python"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("def tot"), "expected 'def tot', got: {}", stdout);
}

// --- Inline code: explicit --run ---

#[test]
fn inline_explicit_run() {
    let out = ilo()
        .args(["tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "--run", "tot", "10", "20", "30"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6200");
}

// --- Error cases ---

#[test]
fn no_args_shows_usage() {
    let out = ilo()
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage"), "expected usage message, got: {}", stderr);
}

#[test]
fn inline_empty_string_errors() {
    let out = ilo()
        .args([""])
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
}

#[test]
fn inline_invalid_code_errors() {
    let out = ilo()
        .args(["this is not valid ilo code @@##$$"])
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.is_empty(), "expected error on stderr");
}

// --- File mode: bare args ---

#[test]
fn file_bare_args_runs_first_func() {
    let out = ilo()
        .args(["research/explorations/idea9-ultra-dense-short/01-simple-function.ilo", "10", "20", "0.1"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    // 01-simple-function.ilo defines tot: (10*20) + (10*20*0.1) = 200 + 20 = 220
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "220");
}

#[test]
fn file_no_args_outputs_ast() {
    let out = ilo()
        .args(["research/explorations/idea9-ultra-dense-short/01-simple-function.ilo"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"name\""), "expected AST JSON, got: {}", stdout);
}

// --- Nested prefix operators ---

#[test]
fn inline_nested_prefix() {
    let out = ilo()
        .args(["f a:n b:n c:n>n;+*a b c", "2", "3", "4"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10");
}

// --- CLI modes ---

#[test]
fn inline_run_vm_mode() {
    let out = ilo()
        .args(["f x:n>n;*x 2", "--run-vm", "f", "5"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10");
}

#[test]
fn inline_run_with_func_name() {
    let out = ilo()
        .args(["f x:n>n;*x 2", "--run", "f", "5"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10");
}

#[test]
fn inline_emit_unknown_target() {
    let out = ilo()
        .args(["f x:n>n;*x 2", "--emit", "javascript"])
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Unknown emit target"), "expected emit error, got: {}", stderr);
}

#[test]
fn inline_parse_bool_arg() {
    let out = ilo()
        .args(["f x:b>b;!x", "true"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "false");
}

#[test]
fn inline_parse_text_arg() {
    let out = ilo()
        .args(["f x:t>t;x", "hello"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello");
}

#[test]
fn inline_parse_error() {
    let out = ilo()
        .args(["f x:>n;x", "5"])
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Parse error") || stderr.contains("error"), "expected parse error, got: {}", stderr);
}

#[test]
fn inline_bench_mode() {
    let out = ilo()
        .args(["f x:n>n;*x 2", "--bench", "f", "5"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("interpreter") || stdout.contains("vm"), "expected benchmark output, got: {}", stdout);
}

// --- Legacy -e flag ---

#[test]
fn legacy_e_flag_still_works() {
    let out = ilo()
        .args(["-e", "tot p:n q:n r:n>n;s=*p q;t=*s r;+s t", "--run", "tot", "10", "20", "30"])
        .output()
        .expect("failed to run ilo");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "6200");
}

#[test]
fn legacy_e_flag_missing_code() {
    let out = ilo()
        .args(["-e"])
        .output()
        .expect("failed to run ilo");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage"), "expected usage message, got: {}", stderr);
}
