#[test]
fn test_version() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["--version"])
        .assert()
        .success()
        .stdout_matches(
            "rust-script [..]
",
        );

    fixture.close();
}

#[test]
fn test_empty_clear_cache() {
    let fixture = crate::util::Fixture::new();
    fixture.cmd().args(["--clear-cache"]).assert().success();
    fixture.close();
}

#[test]
fn test_clear_cache() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .arg("tests/data/script-full-block.rs")
        .assert()
        .success();

    fixture.cmd().args(["--clear-cache"]).assert().success();

    fixture.close();
}
