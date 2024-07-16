use aws_sdk_s3::{operation::get_object::GetObjectError, primitives::ByteStreamError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Missing env variable: {0}")]
    Tui(TuiErrorKind),

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

    #[error("Failed to setup logging")]
    InitializeLogging,

    #[error("Failed to run setup wizard")]
    SetupWizard(SetupWizardErrorKind),

    #[error("Config file missing")]
    ConfigFileMissing,

    #[error("Config file corrupted")]
    ConfigFileCorrupted,
}

#[derive(Error, Debug)]
pub enum SetupWizardErrorKind {
    #[error("Failed to run setup prompt")]
    Prompt,

    #[error("Failed to create remote folder")]
    BucketCreation,

    #[error("Failed to create directory at path: `{0}`")]
    LocalDirectoryCreation(String),

    #[error("Error saving config")]
    ConfigFile(ConfigFileErrorKind),

    #[error("Failed to local home directory")]
    HomeDirectory,
}

#[derive(Error, Debug)]
pub enum ConfigFileErrorKind {
    #[error("Failed to create config file directory at path: `{0}` ")]
    Directory(String),

    #[error("Failed to create config file at path: `{0}`")]
    FileCreation(String),

    #[error("Failed to write to config file at path: `{0}`")]
    FileWrite(String),
}

#[derive(Error, Debug)]
pub enum TuiErrorKind {
    #[error("Failed to initialize terminal")]
    Initilization,

    #[error("Error exiting application...")]
    TerminalRestoration,

    #[error("Error drawing to terminal")]
    Drawing,

    #[error("Error reading keyboard event")]
    KeyboardEvent,
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
