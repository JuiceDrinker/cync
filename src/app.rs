use aws_sdk_s3::primitives::ByteStream;
use config::Config;
use ratatui::widgets::TableState;
use std::cmp;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::{collections::HashMap, fs, path::Path};
use tokio::fs::create_dir;
use tracing::info;
use unicode_width::UnicodeWidthStr;
use util::walk_directory;

use crate::error::{self, Error};
use crate::{config, util};

pub type FilePath = String;
pub type FileHash = md5::Digest;
pub type FileContents = Vec<u8>;
pub type FileMetaData = (FileHash, FileContents);
enum Source {
    Remote,
    Local,
}
pub type Files = BTreeMap<FilePath, FileKind>;
pub struct App {
    pub config: Arc<Config>,
    pub files: FileViewer,
    pub table_state: TableState,
    pub selected_file: Option<usize>,
}

impl FileKind {
    fn create_local(hash: md5::Digest, contents: Vec<u8>) -> Self {
        FileKind::OnlyInLocal { hash, contents }
    }
    fn create_dual_entry(
        remote_hash: md5::Digest,
        remote_contents: Vec<u8>,
        local_hash: md5::Digest,
        local_contents: Vec<u8>,
    ) -> Self {
        FileKind::ExistsInBoth {
            local_hash,
            local_contents,
            remote_hash,
            remote_contents,
        }
    }
    fn create_remote(hash: md5::Digest, contents: Vec<u8>) -> Self {
        FileKind::OnlyInRemote { hash, contents }
    }
}
pub enum FileKind {
    OnlyInRemote {
        hash: md5::Digest,
        contents: Vec<u8>,
    },
    OnlyInLocal {
        hash: md5::Digest,
        contents: Vec<u8>,
    },
    ExistsInBoth {
        local_hash: md5::Digest,
        local_contents: Vec<u8>,
        remote_hash: md5::Digest,
        remote_contents: Vec<u8>,
    },
}

pub struct FileViewer(pub BTreeMap<FilePath, FileKind>);

impl FileViewer {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        let local_files = FileViewer::load_local(config).await?;
        let remote_files = FileViewer::fetch_remote(config).await?;
        let files = remote_files
            .into_iter()
            .map(|(path, (hash, content))| (path, (Source::Remote, hash, content)))
            .chain(
                local_files
                    .into_iter()
                    .map(|(path, (hash, content))| (path, (Source::Local, hash, content))),
            )
            .fold(
                BTreeMap::<FilePath, FileKind>::new(),
                |mut acc, (path, (source, incoming_hash, incoming_content))| {
                    match acc.get_mut(&path) {
                        Some(existing) => match existing {
                            FileKind::OnlyInRemote { hash, contents } => match source {
                                Source::Remote => {
                                    *existing = FileKind::create_remote(
                                        incoming_hash,
                                        incoming_content.clone(),
                                    )
                                }
                                Source::Local => {
                                    *existing = FileKind::create_dual_entry(
                                        *hash,
                                        contents.to_vec(),
                                        incoming_hash,
                                        incoming_content,
                                    )
                                }
                            },
                            FileKind::OnlyInLocal { hash, contents } => match source {
                                Source::Remote => {
                                    *existing = FileKind::create_dual_entry(
                                        incoming_hash,
                                        incoming_content,
                                        *hash,
                                        contents.to_vec(),
                                    )
                                }
                                Source::Local => {
                                    *existing =
                                        FileKind::create_local(incoming_hash, incoming_content)
                                }
                            },
                            FileKind::ExistsInBoth {
                                local_hash,
                                local_contents,
                                remote_hash,
                                remote_contents,
                            } => match source {
                                Source::Remote => {
                                    *existing = FileKind::create_dual_entry(
                                        incoming_hash,
                                        incoming_content,
                                        *local_hash,
                                        local_contents.to_vec(),
                                    )
                                }
                                Source::Local => {
                                    *existing = FileKind::create_dual_entry(
                                        *remote_hash,
                                        remote_contents.to_vec(),
                                        incoming_hash,
                                        incoming_content,
                                    )
                                }
                            },
                        },
                        None => match source {
                            Source::Remote => {
                                acc.insert(
                                    path.to_owned(),
                                    FileKind::OnlyInRemote {
                                        hash: incoming_hash,
                                        contents: incoming_content,
                                    },
                                );
                            }
                            Source::Local => {
                                acc.insert(
                                    path.to_owned(),
                                    FileKind::OnlyInRemote {
                                        hash: incoming_hash,
                                        contents: incoming_content,
                                    },
                                );
                            }
                        },
                    };
                    acc
                },
            );
        Ok(FileViewer(files))
    }

    async fn fetch_remote(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        let mut remote = HashMap::new();
        let mut paginated_response = config
            .aws_client()
            .list_objects_v2()
            .bucket(config.bucket_name())
            .max_keys(10)
            .into_paginator()
            .send();

        while let Some(result) = paginated_response.next().await {
            if let Ok(output) = result {
                for object in output.contents() {
                    if let Ok(remote_object) = config
                        .aws_client()
                        .get_object()
                        .bucket(config.bucket_name())
                        .key(object.key().expect("Uploaded objects must have a key"))
                        .send()
                        .await
                    {
                        let aggregated_bytes = remote_object
                            .body
                            .collect()
                            .await
                            .expect("Contents are valid utf-8");
                        remote.insert(
                            object
                                .key()
                                .expect("Uploaded objects must have a key")
                                .to_string(),
                            (
                                md5::compute(&aggregated_bytes.clone().into_bytes()),
                                aggregated_bytes.clone().to_vec(),
                            ),
                        );
                    };
                }
            } else {
                return Err(Error::FailedToFetchRemote);
            };
        }

        info!("Fetched {} object from remote host", remote.keys().count());
        Ok(remote)
    }

    async fn load_local(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        if fs::metadata(config.local_path())
            .map_err(|_| Error::LoadingLocalFiles(error::LoadingLocalFiles::FileSystem))?
            .is_dir()
        {
            let local_files = walk_directory(Path::new(config.local_path()))?;
            info!("Found {} local files", local_files.keys().count());
            Ok(local_files)
        } else {
            info!("Could not find local directory");
            App::create_default_directory(config).await?;
            Ok(HashMap::new())
        }
    }
}

impl App {
    pub async fn new(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = Arc::new(Config::load_from_env(aws_config)?);
        Ok(Self {
            config: Arc::clone(&config),
            files: FileViewer::new(&config).await?,
            table_state: TableState::default().with_selected(0),
            selected_file: None,
        })
    }

    pub fn view_files(&self) -> &Files {
        &self.files.0
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

        #[allow(clippy::cast_possible_truncation)]
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

    async fn create_default_directory(config: &Config) -> Result<(), Error> {
        info!("Creating default directory");
        if create_dir(config.local_path()).await.is_ok() {
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
            .aws_client
            .put_object()
            .bucket(self.config.bucket_name())
            .key(path)
            .body(ByteStream::from(content.clone()))
            .send()
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
        let content = match kind {
            FileKind::OnlyInRemote { contents, .. } => Ok(contents),
            FileKind::OnlyInLocal { .. } => Err(Error::LocalSyncFailed),
            FileKind::ExistsInBoth { local_contents, .. } => Ok(local_contents),
        }?;
        fs::write(path, content).map_err(|_| Error::LocalSyncFailed)?;
        Ok(())
    }
}
