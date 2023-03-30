pub struct Fixture {
    fixture: snapbox::path::PathFixture,
    cache_path: std::path::PathBuf,
}

impl Fixture {
    #[track_caller]
    pub fn new() -> Self {
        let fixture = snapbox::path::PathFixture::mutable_temp().unwrap();
        let cache_path = fixture.path().unwrap().join("cache");
        Self {
            fixture,
            cache_path,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.fixture.path().unwrap()
    }

    pub fn cmd(&self) -> snapbox::cmd::Command {
        snapbox::cmd::Command::new(snapbox::cmd::cargo_bin("rust-script"))
            .env_remove("CARGO_TARGET_DIR")
            .env("RUST_SCRIPT_CACHE_PATH", &self.cache_path)
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
