pub struct Fixture {
    fixture: snapbox::path::PathFixture,
    target_path: std::path::PathBuf,
}

impl Fixture {
    #[track_caller]
    pub fn new() -> Self {
        let fixture = snapbox::path::PathFixture::mutable_temp().unwrap();
        let target_path = fixture.path().unwrap().join("target");
        Self {
            fixture,
            target_path,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.fixture.path().unwrap()
    }

    pub fn cmd(&self) -> snapbox::cmd::Command {
        let mut subst = snapbox::Substitutions::new();
        subst
            .insert("[CWD]", self.path().display().to_string())
            .unwrap();
        subst.insert("[EXE]", std::env::consts::EXE_SUFFIX).unwrap();
        let assert = snapbox::Assert::new().substitutions(subst);
        snapbox::cmd::Command::new(snapbox::cmd::cargo_bin("cargo-eval"))
            .env("CARGO_TARGET_DIR", &self.target_path)
            .with_assert(assert)
    }

    #[track_caller]
    pub fn close(self) {
        self.fixture.close().unwrap();
    }
}

macro_rules! with_output_marker {
    (prelude $p:expr; $e:expr) => {
        format!(concat!($p, "{}", $e), crate::util::OUTPUT_MARKER_CODE)
    };

    ($e:expr) => {
        format!(concat!("{}", $e), crate::util::OUTPUT_MARKER_CODE)
    };
}

pub const OUTPUT_MARKER_CODE: &str = "println!(\"--output--\");";
