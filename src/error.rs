use std::ffi::OsStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Missing env variable: {0}")]
    MissingEnvVar(String),

    #[error("Env variable corrupt")]
    LocalPathVariableCorrupt(Box<OsStr>),

    #[error("Could not read file at path: {0}")]
    LocalFileCorrupted(String),

    #[error("Sync with remote failed")]
    SyncFailed(String),
}
