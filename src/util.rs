// #[cfg(test)] // https://github.com/rust-lang/rust/issues/45599
pub mod testing {
    pub fn logger_init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
}
