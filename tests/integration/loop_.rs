#[test]
fn test_basic() {
    let fixture = crate::util::Fixture::new();
    fixture
        .cmd()
        .args(["-Zloop", "--loop"])
        .arg("let mut n=0; move |line| {n+=1; println!(\"{:>6}: {}\",n,line.trim_end())}")
        .stdin("hello\nworld")
        .assert()
        .success()
        .stdout_eq(
            "     1: hello
     2: world
",
        );

    fixture.close();
}
