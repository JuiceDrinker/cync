use aws_sdk_s3::primitives::ByteStream;
use config::Config;
use ratatui::widgets::TableState;
use std::cmp;
use std::collections::btree_map::Keys;
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
pub struct App {
    pub config: Arc<Config>,
    pub remote_files: HashMap<FilePath, FileMetaData>,
    pub local_files: HashMap<FilePath, FileMetaData>,
    pub table_state: TableState,
    pub selected_file: Option<usize>,
}

pub struct FileDetails {
    pub remote_hash: Option<md5::Digest>,
    pub local_hash: Option<md5::Digest>,
    pub are_hashes_identical: bool,
}

impl FileDetails {
    pub fn local_hash(&self) -> Option<md5::Digest> {
        self.local_hash
    }

    pub fn remote_hash(&self) -> Option<md5::Digest> {
        self.remote_hash
    }
}
pub struct FileViewer(pub BTreeMap<FilePath, FileDetails>);

impl FileViewer {
    fn keys(&self) -> Keys<FilePath, FileDetails> {
        self.0.keys()
    }
}

impl App {
    pub async fn new(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = Arc::new(Config::load_from_env(aws_config)?);
        Ok(Self {
            config: Arc::clone(&config),
            remote_files: App::fetch_remote(&config).await?,
            local_files: App::load_local(&config).await?,
            table_state: TableState::default().with_selected(0),
            selected_file: None,
        })
    }

    pub fn constraint_len_calculator(&self) -> (u16, u16, u16) {
        let (key_len, local_len, remote_len) = &self.view_files().0.iter().fold(
            (0, 0, 0),
            |(mut path_len, mut remote_len, mut local_len),
             (
                path,
                FileDetails {
                    remote_hash,
                    local_hash,
                    ..
                },
            )| {
                path_len = cmp::max(path_len, UnicodeWidthStr::width(path.as_str()));
                if let Some(r) = remote_hash {
                    remote_len = cmp::max(
                        remote_len,
                        UnicodeWidthStr::width(format!("{:?}", r).as_str()),
                    );
                }
                if let Some(l) = local_hash {
                    local_len = cmp::max(
                        remote_len,
                        UnicodeWidthStr::width(format!("{:?}", l).as_str()),
                    );
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

    pub fn view_files(&self) -> FileViewer {
        let files = self
            .remote_files
            .iter()
            .map(|(path, (hash, _))| (path, (Source::Remote, hash)))
            .chain(
                self.local_files
                    .iter()
                    .map(|(path, (hash, _))| (path, (Source::Local, hash))),
            )
            .fold(
                BTreeMap::<FilePath, FileDetails>::new(),
                |mut acc, (path, (source, hash))| {
                    match acc.get_mut(path) {
                        Some(existing) => match source {
                            Source::Remote => {
                                existing.remote_hash = Some(*hash);
                                existing.are_hashes_identical =
                                    existing.local_hash() == Some(*hash);
                            }
                            Source::Local => {
                                existing.local_hash = Some(*hash);
                                existing.are_hashes_identical =
                                    existing.remote_hash() == Some(*hash);
                            }
                        },
                        None => match source {
                            Source::Remote => {
                                acc.insert(
                                    path.to_owned(),
                                    FileDetails {
                                        remote_hash: Some(*hash),
                                        local_hash: None,
                                        are_hashes_identical: false,
                                    },
                                );
                            }
                            Source::Local => {
                                acc.insert(
                                    path.to_owned(),
                                    FileDetails {
                                        remote_hash: Some(*hash),
                                        local_hash: None,
                                        are_hashes_identical: true,
                                    },
                                );
                            }
                        },
                    };
                    acc
                },
            );
        FileViewer(files)
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

    async fn create_default_directory(config: &Config) -> Result<(), Error> {
        info!("Creating default directory");
        if create_dir(config.local_path()).await.is_ok() {
            Ok(())
        } else {
            Err(Error::FailedToCreateDefaultDirectory)
        }
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

    pub async fn push_file_to_remote(&self, selected_file_key: usize) -> Result<(), Error> {
        let files = self.view_files().0;
        let (path, _) = files
            .iter()
            .nth(selected_file_key)
            .expect("file to be present");
        let (_, content) = self
            .local_files
            .get(path)
            .expect("file to be present locally");
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

    pub fn pull_file_from_remote(&self, selected_file_key: usize) -> Result<(), Error> {
        let files = self.view_files().0;
        let (path, _) = files
            .iter()
            .nth(selected_file_key)
            .expect("file to be present");
        let (_, content) = self
            .remote_files
            .get(path)
            .expect("file to be present locally");
        fs::write(path, content).map_err(|_| Error::LocalSyncFailed)?;
        Ok(())
    }

    pub async fn referesh_app_state(&mut self) -> Result<(), Error> {
        self.remote_files = App::fetch_remote(&self.config).await?;
        self.local_files = App::load_local(&self.config).await?;

        Ok(())
    }
}
