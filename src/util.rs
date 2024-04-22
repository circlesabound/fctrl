pub mod version {
    pub const BUILD_TIMESTAMP: &'static str = env!("VERGEN_BUILD_TIMESTAMP");
    pub const GIT_SHA: Option<&'static str> = option_env!("GIT_COMMIT_HASH");
}

// #[cfg(test)] // https://github.com/rust-lang/rust/issues/45599
pub mod testing {
    pub fn logger_init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
}
