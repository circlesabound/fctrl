pub type Result<T> = std::result::Result<T, crate::error::Error>;

#[derive(Debug)]
pub enum Error {
    // Process management
    ProcessAlreadyRunning,
    ProcessAlreadyStopped,
    ProcessPidError,
    ProcessSignalError(nix::Error),

    // Generic wrappers around external error types
    Io(std::io::Error),
    Reqwest(reqwest::Error),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
