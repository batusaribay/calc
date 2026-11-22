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
    let mut child = Command::new(env!("CARGO_BIN_EXE_calc"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn calc");

    child
        .stdin
        .as_mut()
        .expect("missing stdin")
        .write_all(b"6 * 7\n")
        .expect("failed to write stdin");

    let out = child.wait_with_output().expect("failed to wait for calc");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}
