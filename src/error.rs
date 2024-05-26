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

    #[error("Failed to retrieve object from host")]
    FailedToFetchRemote,
    #[error("Failed to upload loacal files to remote host")]
    FailedToLoadLocalFiles,

    #[error("Failed to sync remote with local")]
    LocalSyncFailed,

    #[error("Failed to sync local with remote")]
    RemoteSyncFailed,

    #[error("Failed to create default Cync directory")]
    FailedToCreateDefaultDirectory,
}
