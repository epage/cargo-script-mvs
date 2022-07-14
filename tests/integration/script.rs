#[test]
fn test_script_override_backtrace() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .env("RUST_BACKTRACE", "0")
        .arg("tests/data/script-override-backtrace.rs")
        .assert()
        .failure()
        .stderr_matches(
            "...
thread 'main' panicked at 'a pink elephant!', script-override-backtrace.rs:6:5
...",
        );

    fixture.close();
}
#[test]
fn test_script_default_backtrace() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-default-backtrace.rs")
        .assert()
        .failure()
        .stderr_matches(
            "...
thread 'main' panicked at 'a pink elephant!', script-default-backtrace.rs:6:5
stack backtrace:
   0: std::panicking::begin_panic
...",
        );

    fixture.close();
}
#[test]
fn test_script_features() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--features", "dont-panic"])
        .arg("tests/data/script-features.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
Keep calm and borrow check.
",
        );

    fixture.cmd()
        .arg("tests/data/script-features.rs")
        .assert()
        .failure()
        .stderr_matches(
            "...
thread 'main' panicked at 'Do I really exist from an external, non-subjective point of view?', script-features.rs:15:5
...",
        );

    fixture.close();
}

#[test]
fn test_script_full_block() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-full-block.rs")
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
fn test_script_full_line() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-full-line.rs")
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
fn test_script_full_line_without_main() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-full-line-without-main.rs")
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
fn test_script_invalid_doc_comment() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-invalid-doc-comment.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
Hello, World!
",
        );

    fixture.close();
}

#[test]
fn test_script_no_deps() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-no-deps.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
Hello, World!
",
        );

    fixture.close();
}

#[test]
fn test_script_short() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-short.rs")
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
fn test_script_short_without_main() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-short-without-main.rs")
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
fn test_script_test() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Ztest", "--test"])
        .arg("tests/data/script-test.rs")
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
fn test_script_hyphens() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--"])
        .arg("tests/data/script-args.rs")
        .args(["-NotAnArg"])
        .assert()
        .success()
        .stdout_matches(
            r#"--output--
 [0]: "[..]/script-args_[..]"
 [1]: "-NotAnArg"
"#,
        );

    fixture.close();
}

#[test]
fn test_script_hyphens_without_separator() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-args.rs")
        .args(["-NotAnArg"])
        .assert()
        .success()
        .stdout_matches(
            r#"--output--
 [0]: "[..]/script-args_[..]"
 [1]: "-NotAnArg"
"#,
        );

    fixture.close();
}

#[test]
fn test_script_has_weird_chars() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-has.weird§chars!.rs")
        .assert()
        .success();

    fixture.close();
}

#[test]
fn test_script_cs_env() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-cs-env.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
Ok
",
        );

    fixture.close();
}

#[test]
fn test_script_including_relative() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-including-relative.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
hello, including script
",
        );

    fixture.close();
}

#[test]
fn script_with_same_name_as_dependency() {
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
fn script_without_main_question_mark() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/question-mark")
        .assert()
        .failure()
        .stderr_matches(
            "Error: Os { code: 2, kind: NotFound, message: [..] }
",
        );

    fixture.close();
}

#[test]
fn test_script_async_main() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-async-main.rs")
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
fn test_pub_fn_main() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/pub-fn-main.rs")
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
fn test_cargo_target_dir_env() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/cargo-target-dir-env.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
true
",
        );

    fixture.close();
}

#[test]
fn test_outer_line_doc() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/outer-line-doc.rs")
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
fn test_whitespace_before_main() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/whitespace-before-main.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
hello, world
",
        );

    fixture.close();
}

#[test]
fn test_stable_toolchain() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Ztoolchain-version", "--toolchain-version", "stable"])
        .arg("tests/data/script-unstable-feature.rs")
        .assert()
        .failure()
        .stderr_matches(
            "error[E0554]: `#![feature]` may not be used on the stable release channel
...
",
        );

    fixture.close();
}

#[test]
fn test_nightly_toolchain() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Ztoolchain-version", "--toolchain-version", "nightly"])
        .arg("tests/data/script-unstable-feature.rs")
        .assert()
        .success()
        .stdout_eq(
            "--output--
`#![feature]` *may* be used!
",
        );

    fixture.close();
}

#[test]
fn test_same_flags() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/same-flags.rs")
        .args(["--help"])
        .assert()
        .success()
        .stdout_eq(
            "--output--
Argument: --help
",
        );

    fixture.close();
}
