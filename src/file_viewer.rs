use config::Config;
use std::collections::BTreeMap;
use std::{collections::HashMap, fs, path::Path};
use tracing::info;
use util::walk_directory;

use crate::app::{App, FileMetaData, FilePath};
use crate::error::{self, Error};
use crate::{config, util};

enum Source {
    Remote,
    Local,
}

pub type Files = BTreeMap<FilePath, FileKind>;

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

#[derive(Clone)]
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
        let files = FileViewer::load_files(config).await?;
        Ok(FileViewer(files))
    }

    pub async fn load_files(config: &Config) -> Result<Files, Error> {
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
        Ok(files)
    }

    async fn fetch_remote(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        let mut remote = HashMap::new();
        let mut paginated_response = config
            .aws_client()
            .list_objects_v2()
            .bucket(config.remote_directory())
            .max_keys(10)
            .into_paginator()
            .send();

        while let Some(result) = paginated_response.next().await {
            if let Ok(output) = result {
                for object in output.contents() {
                    if let Ok(remote_object) = config
                        .aws_client()
                        .get_object()
                        .bucket(config.remote_directory())
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
        if fs::metadata(config.local_directory())
            .map_err(|_| Error::LoadingLocalFiles(error::LoadingLocalFiles::FileSystem))?
            .is_dir()
        {
            let local_files = walk_directory(Path::new(config.local_directory()))?;
            info!("Found {} local files", local_files.keys().count());
            Ok(local_files)
        } else {
            info!("Could not find local directory");
            App::create_default_directory(config).await?;
            Ok(HashMap::new())
        }
    }
}
