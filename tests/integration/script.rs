#[test]
fn basic() {
    let fixture = crate::util::Fixture::new();

    fixture
        .cmd()
        .arg("tests/data/hello_world.rs")
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches("")
        .stdout_eq(
            "Hello world!
",
        );
}

#[test]
fn eval_explicit_stdin() {
    let fixture = crate::util::Fixture::new();
    let stdin = std::fs::read_to_string("tests/data/hello_world.rs").unwrap();

    fixture
        .cmd()
        .arg("-")
        .stdin(&stdin)
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches("")
        .stdout_eq(
            "Hello world!
",
        );
}

#[test]
fn eval_implicit_stdin() {
    let fixture = crate::util::Fixture::new();
    let stdin = std::fs::read_to_string("tests/data/hello_world.rs").unwrap();

    fixture
        .cmd()
        .stdin(&stdin)
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches("")
        .stdout_eq(
            "Hello world!
",
        );
}

#[test]
fn regular_stdin() {
    let fixture = crate::util::Fixture::new();
    let stdin = "Hello world!";

    fixture
        .cmd()
        .arg("tests/data/echo.rs")
        .stdin(stdin)
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches("")
        .stdout_eq(
            "msg = Hello world!
",
        );
}

#[test]
fn rebuild() {
    let fixture = crate::util::Fixture::new();

    fixture
        .cmd()
        .arg("--verbose")
        .arg("tests/data/cecho.rs")
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches(
            "   Compiling cecho v0.0.0 ([CWD]/target/eval/[..]/cecho)
    Finished dev [unoptimized + debuginfo] target(s) in [..]s
     Running `[CWD]/target/debug/cecho_[..][EXE]`
",
        )
        .stdout_eq(
            "msg = undefined
",
        );

    fixture
        .cmd()
        .arg("--verbose")
        .arg("tests/data/cecho.rs")
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches(
            "    Finished dev [unoptimized + debuginfo] target(s) in [..]s
     Running `[CWD]/target/debug/cecho_[..][EXE]`
",
        )
        .stdout_eq(
            "msg = undefined
",
        );

    fixture
        .cmd()
        .arg("--verbose")
        .arg("tests/data/cecho.rs")
        .env("_RUST_SCRIPT_TEST_MESSAGE", "hello")
        .env("CARGO_HOME", fixture.path().join("cargo_home")) // Avoid package cache lock messages
        .assert()
        .success()
        .stderr_matches(
            "   Compiling cecho v0.0.0 ([CWD]/target/eval/[..]/cecho)
    Finished dev [unoptimized + debuginfo] target(s) in [..]s
     Running `[CWD]/target/debug/cecho_[..][EXE]`
",
        )
        .stdout_eq(
            "msg = hello
",
        );

    fixture.close();
}

#[test]
fn test_line_numbering_preserved() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/print_line.rs")
        .assert()
        .success()
        .stdout_eq(
            "line: 4
",
        );

    fixture.close();
}

#[test]
fn test_default_backtrace() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/panic.rs")
        .assert()
        .failure()
        .stderr_matches(
            "thread 'main' panicked at 'a pink elephant!', [..]/tests/data/panic.rs:4:5
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
        .arg("tests/data/panic.rs")
        .assert()
        .failure()
        .stderr_matches(
            "thread 'main' panicked at 'a pink elephant!', [..]/tests/data/panic.rs:4:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
",
        );

    fixture.close();
}

#[test]
fn test_test() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zpolyfill", "--test"])
        .arg("tests/data/test.rs")
        .assert()
        .success()
        .stdout_matches(
            "
running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [..]s

",
        );

    fixture.close();
}

#[test]
fn test_escaped_hyphen_arg() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--"])
        .arg("tests/data/args.rs")
        .args(["-NotAnArg"])
        .assert()
        .success()
        .stdout_matches(
            r#"
 [0]: "[..]/args_[..]"
 [1]: "-NotAnArg"
"#,
        );

    fixture.close();
}

#[test]
fn test_unescaped_hyphen_arg() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/args.rs")
        .args(["-NotAnArg"])
        .assert()
        .success()
        .stdout_matches(
            r#"
 [0]: "[..]/args_[..]"
 [1]: "-NotAnArg"
"#,
        );

    fixture.close();
}

#[test]
fn test_same_flags() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/args.rs")
        .args(["--help"])
        .assert()
        .success()
        .stdout_matches(
            r#"
 [0]: "[..]args_[..]"
 [1]: "--help"
"#,
        );

    fixture.close();
}

#[test]
fn test_has_weird_chars() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-has.weird§chars!.rs")
        .assert()
        .success();

    fixture.close();
}

#[test]
fn test_script_with_same_name_as_dependency() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/time.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
Hello
",
        );

    fixture.close();
}

#[test]
fn test_path_dep() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/path_dep.rs")
        .assert()
        .success()
        .stdout_eq(
            "Hello world!
",
        );

    fixture.close();
}
