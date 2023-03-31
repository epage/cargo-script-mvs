#[test]
fn test_version() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--version"])
        .assert()
        .success()
        .stdout_matches(
            "cargo-shell [..]
",
        );

    fixture.close();
}

#[test]
fn test_clean_noop() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--clean", "-Zpolyfill"])
        .arg("tests/data/hello_world.rs")
        .assert()
        .success();
    fixture.close();
}

#[test]
fn test_clean() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/hello_world.rs")
        .assert()
        .success();

    fixture
        .cmd()
        .args(["--clean", "-Zpolyfill"])
        .arg("tests/data/hello_world.rs")
        .assert()
        .success();

    fixture.close();
}
