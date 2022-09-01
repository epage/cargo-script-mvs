#[test]
fn test_expr_0() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
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
fn test_expr_comma() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
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
fn test_expr_dnc() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
        .arg("swing-begin")
        .assert()
        .failure();

    fixture.close();
}

#[test]
fn test_expr_temporary() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
        .arg("[1].iter().max()")
        .assert()
        .success();

    fixture.close();
}

#[test]
fn test_expr_panic() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
        .arg(with_output_marker!("panic!()"))
        .assert()
        .failure();

    fixture.close();
}

#[test]
fn test_expr_qmark() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
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
fn test_expr_template() {
    let template_dir = "tests/data/templates";
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e", "-t", "shout"])
        .arg(with_output_marker!(r#""no way? no way!""#))
        .env("RUST_SCRIPT_DEBUG_TEMPLATE_PATH", template_dir)
        .assert()
        .success()
        .stdout_eq(
            "--output--
NO WAY? NO WAY!
",
        );

    fixture.close();
}

#[test]
fn test_expr_template_with_deps() {
    let template_dir = "tests/data/templates";
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e", "-t", "boolinate"])
        .arg(with_output_marker!("true"))
        .env("RUST_SCRIPT_DEBUG_TEMPLATE_PATH", template_dir)
        .assert()
        .success()
        .stdout_eq(
            "--output--
Some(())
",
        );

    fixture.close();
}

#[test]
fn test_expr_template_override_expr() {
    let template_dir = "tests/data/templates/override";
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zexpr", "-e"])
        .arg(with_output_marker!("true"))
        .env("RUST_SCRIPT_DEBUG_TEMPLATE_PATH", template_dir)
        .assert()
        .success()
        .stdout_eq(
            "--output--
Some(())
",
        );

    fixture.close();
}
