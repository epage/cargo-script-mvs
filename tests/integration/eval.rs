#[test]
fn test_0() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(with_output_marker!("0"))
        .assert()
        .success()
        .stdout_eq(
            "--output--
0
",
        );

    fixture.close();
}

#[test]
fn test_rebuild() {
    let script = with_output_marker!("env!(\"_RUST_SCRIPT_TEST_MESSAGE\")");
    let fixture = crate::util::Fixture::new();

    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(&script)
        .env("_RUST_SCRIPT_TEST_MESSAGE", "hello")
        .assert()
        .success()
        .stdout_eq(
            r#"--output--
"hello"
"#,
        );

    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(&script)
        .env("_RUST_SCRIPT_TEST_MESSAGE", "hello")
        .assert()
        .success()
        .stdout_eq(
            r#"--output--
"hello"
"#,
        );

    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(&script)
        .env("_RUST_SCRIPT_TEST_MESSAGE", "goodbye")
        .assert()
        .success()
        .stdout_eq(
            r#"--output--
"goodbye"
"#,
        );

    fixture.close();
}

#[test]
fn test_comma() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(with_output_marker!("[1, 2, 3]"))
        .assert()
        .success()
        .stdout_eq(
            "--output--
[1, 2, 3]
",
        );

    fixture.close();
}

#[test]
fn test_dnc() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg("swing-begin")
        .assert()
        .failure();

    fixture.close();
}

#[test]
fn test_temporary() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(with_output_marker!("[1].iter().max()"))
        .assert()
        .success()
        .stdout_eq(
            "--output--
Some(1)
",
        );

    fixture.close();
}

#[test]
fn test_qmark() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(with_output_marker!(
            "\"42\".parse::<i32>()?.wrapping_add(1)"
        ))
        .assert()
        .success()
        .stdout_eq(
            "--output--
43
",
        );

    fixture.close();
}

#[test]
fn test_default_backtrace() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zeval", "-e"])
        .arg(r#"panic!("a pink elephant!")"#)
        .assert()
        .failure()
        .stderr_matches(
            "thread 'main' panicked at 'a pink elephant!', [CWD]/target/eval/[..]/eval.rs:21:12
stack backtrace:
...",
        );

    fixture.close();
}

#[test]
fn test_override_backtrace() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .env("RUST_BACKTRACE", "0")
        .args(["-Zeval", "-e"])
        .arg(r#"panic!("a pink elephant!")"#)
        .assert()
        .failure()
        .stderr_matches(
            "thread 'main' panicked at 'a pink elephant!', [CWD]/target/eval/[..]/eval.rs:21:12
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
",
        );

    fixture.close();
}
