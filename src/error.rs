use aws_sdk_s3::{operation::get_object::GetObjectError, primitives::ByteStreamError};
use std::{ffi::OsStr, io};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Missing env variable: {0}")]
    MissingEnvVar(String),

    #[error("Env variable corrupt")]
    LocalPathVariableCorrupt(Box<OsStr>),

    #[error("Could not read file at path: {0}")]
    LocalFileCorrupted(String),

    #[error("Failed to retrieve object from host")]
    FailedToFetchRemote,

    #[error("Failed to upload loacal files to remote host")]
    LoadingLocalFiles(LoadingLocalFiles),

    #[error("Failed to sync remote with local")]
    LocalSyncFailed,

    #[error("Failed to sync local with remote")]
    RemoteSyncFailed,

    #[error("Failed to create default Cync directory")]
    FailedToCreateDefaultDirectory,
}

#[derive(Error, Debug)]
pub enum LoadingLocalFiles {
    #[error("Failed to upload loacal files to remote host")]
    FileSystem,
}

impl From<GetObjectError> for Error {
    fn from(_value: GetObjectError) -> Self {
        Error::FailedToFetchRemote
    }
}
impl From<ByteStreamError> for Error {
    fn from(_value: ByteStreamError) -> Self {
        Error::FailedToFetchRemote
    }
}

impl From<io::Error> for Error {
    fn from(_value: io::Error) -> Self {
        Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem)
    }
}
