use aws_sdk_s3::primitives::ByteStream;
use config::Config;
use ratatui::widgets::TableState;
use std::cmp;
use std::fs;
use std::sync::Arc;
use tokio::fs::create_dir;
use tracing::info;
use unicode_width::UnicodeWidthStr;

use crate::error::Error;
use crate::trace_dbg;

use self::file_viewer::FileKind;
use self::file_viewer::FileViewer;
use self::file_viewer::Files;

pub mod config;
pub mod file_viewer;

pub type FilePath = String;
pub type FileHash = md5::Digest;
pub type FileContents = Vec<u8>;
pub type FileMetaData = (FileHash, FileContents);

#[derive(PartialEq)]
pub enum Mode {
    Default,
    PendingAction(FileKind),
    NoFilesFound,
}

pub struct Cync {
    pub mode: Mode,
    pub config: Arc<Config>,
    pub files: FileViewer,
    pub table_state: TableState,
    pub selected_file: Option<usize>,
}

impl Cync {
    pub async fn new(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = Arc::new(Config::load(aws_config)?);
        let files = FileViewer::new().load_files(&config).await?;
        Ok(Self {
            mode: if files.0.is_empty() {
                Mode::NoFilesFound
            } else {
                Mode::Default
            },
            config: Arc::clone(&config),
            files,
            table_state: TableState::default().with_selected(0),
            selected_file: None,
        })
    }

    pub async fn reload_files(&mut self) -> Result<(), Error> {
        self.files = FileViewer::new().load_files(&self.config).await?;
        Ok(())
    }

    pub fn view_files(&self) -> &Files {
        &self.files.0
    }

    pub fn select_file(&mut self, index: usize) {
        let (_, kind) = self.view_files().iter().nth(index).unwrap();
        self.mode = Mode::PendingAction(kind.clone());
        self.selected_file = Some(index);
    }

    pub fn constraint_len_calculator(&self) -> (u16, u16, u16) {
        let (key_len, local_len, remote_len) = &self.view_files().iter().fold(
            (0, 0, 0),
            |(mut path_len, mut remote_len, mut local_len), (path, kind)| {
                path_len = cmp::max(path_len, UnicodeWidthStr::width(path.as_str()));
                match kind {
                    FileKind::OnlyInRemote { hash, .. } => {
                        remote_len = cmp::max(
                            remote_len,
                            UnicodeWidthStr::width(format!("{:?}", hash).as_str()),
                        );
                    }
                    FileKind::OnlyInLocal { hash, .. } => {
                        local_len = cmp::max(
                            local_len,
                            UnicodeWidthStr::width(format!("{:?}", hash).as_str()),
                        );
                    }
                    FileKind::ExistsInBoth {
                        local_hash,
                        remote_hash,
                        ..
                    } => {
                        local_len = cmp::max(
                            local_len,
                            UnicodeWidthStr::width(format!("{:?}", local_hash).as_str()),
                        );
                        remote_len = cmp::max(
                            remote_len,
                            UnicodeWidthStr::width(format!("{:?}", remote_hash).as_str()),
                        );
                    }
                }
                (path_len, remote_len, local_len)
            },
        );

        (*key_len as u16, *local_len as u16, *remote_len as u16)
    }

    pub fn prev_file(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.view_files().keys().count() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };

        self.table_state.select(Some(i));
    }

    pub fn next_file(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.view_files().keys().count() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.table_state.select(Some(i));
    }

    pub async fn create_default_directory(config: &Config) -> Result<(), Error> {
        // TODO: If we choose to keep this what do we do about config file?
        info!("Creating default directory");

        if create_dir(config.local_directory()).await.is_ok() {
            Ok(())
        } else {
            Err(Error::FailedToCreateDefaultDirectory)
        }
    }

    pub async fn push_file_to_remote(&self, index: usize) -> Result<(), Error> {
        let (path, kind) = self
            .view_files()
            .iter()
            .nth(index)
            .expect("to pass a valid index");

        let content = match kind {
            FileKind::OnlyInRemote { .. } => Err(Error::RemoteSyncFailed),
            FileKind::OnlyInLocal { contents, .. } => Ok(contents),
            FileKind::ExistsInBoth { local_contents, .. } => Ok(local_contents),
        }?;

        self.config
            .aws_client()
            .put_object(
                self.config.clone().remote_directory().to_string(),
                path.to_string(),
                ByteStream::from(content.clone()),
            )
            .await
            .map_err(|_| Error::RemoteSyncFailed)?;
        Ok(())
    }

    pub fn pull_file_from_remote(&self, index: usize) -> Result<(), Error> {
        let (path, kind) = self
            .view_files()
            .iter()
            .nth(index)
            .expect("to pass a valid index");

        let content = match trace_dbg!(kind) {
            FileKind::OnlyInRemote { contents, .. } => Ok(contents),
            FileKind::ExistsInBoth {
                remote_contents, ..
            } => Ok(remote_contents),
            FileKind::OnlyInLocal { .. } => Err(Error::LocalSyncFailed),
        }?;
        fs::write(
            format!("{}/{}", self.config.local_directory().display(), path),
            content,
        )
        .map_err(|_| Error::LocalSyncFailed)?;
        Ok(())
    }
}
