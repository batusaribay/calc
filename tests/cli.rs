use std::io::Write;
use std::process::{Command, Stdio};

fn calc(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_calc"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run calc")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn calc_with_stdin(args: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_calc"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn calc");
    child
        .stdin
        .take()
        .expect("missing stdin")
        .write_all(input)
        .expect("failed to write stdin");
    child.wait_with_output().expect("failed to wait for calc")
}

#[test]
fn evaluates_arguments_as_one_expression() {
    let out = calc(&["2", "+", "3", "*", "4"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "14\n");
}

#[test]
fn supports_precision() {
    let out = calc(&["--precision", "4", "1/3"]);
    assert_eq!(stdout(&out), "0.3333\n");
}

#[test]
fn emits_json() {
    let out = calc(&["--json", "sqrt(2)"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        "{\"expression\":\"sqrt(2)\",\"value\":1.4142135623730951,\"display\":\"1.414213562373\"}\n"
    );
}

#[test]
fn invalid_expression_exits_with_2() {
    let out = calc(&["2+"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unexpected end of expression"));
}

#[test]
fn reads_piped_stdin() {
    let out = calc_with_stdin(&[], b"6 * 7\n");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn rejects_excessive_nesting_without_crashing() {
    let expression = format!("{}1{}", "(".repeat(4096), ")".repeat(4096));
    let out = calc(&["--", &expression]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nesting exceeds 256 levels"));
}

#[test]
fn rejects_an_invalid_attached_precision() {
    let out = calc(&["-pfoo", "1/3"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid precision `foo`"));
}

#[test]
fn limits_stdin_input() {
    let out = calc_with_stdin(&[], &vec![b'1'; 1024 * 1024 + 1]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("stdin limit"));
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_arguments_without_panicking() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let out = Command::new(env!("CARGO_BIN_EXE_calc"))
        .arg(OsString::from_vec(vec![b'1', 0x80]))
        .stdin(Stdio::null())
        .output()
        .expect("failed to run calc");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("valid UTF-8"));
}
