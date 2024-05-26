use crate::error::Error;
use aws_sdk_s3::{
    operation::get_object::GetObjectError,
    primitives::{ByteStream, ByteStreamError},
};
use config::Config;
use std::{
    collections::HashMap,
    fs::{self, DirEntry, File},
    io::{self, Read},
    sync::Arc,
};
use tokio::fs::create_dir;
use tokio_stream::StreamExt;
use tracing::info;

mod config;
mod error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    Cync::new(&aws_config::load_from_env().await)
        .await?
        .run_sync()
        .await
}

type FilePath = String;
type FileHash = md5::Digest;
type FileContents = Vec<u8>;
type FileMetaData = (FileHash, FileContents);

struct Cync {
    config: Arc<Config>,
    remote: HashMap<FilePath, FileMetaData>,
    local: HashMap<FilePath, FileMetaData>,
}

impl Cync {
    async fn new(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = Arc::new(Config::load_from_env(aws_config)?);
        Ok(Self {
            config: Arc::clone(&config),
            remote: Cync::fetch_remote(&config).await?,
            local: Cync::load_local(&config).await?,
        })
    }

    async fn run_sync(self) -> Result<(), Error> {
        self.sync_local_with_remote().await?;
        self.sync_remote_with_local().await?;
        Ok(())
    }

    async fn sync_remote_with_local(&self) -> Result<(), Error> {
        let (_, error): (Vec<_>, Vec<_>) = futures::future::join_all(
            self.remote
                .iter()
                .filter_map(|(path, (_, content))| {
                    if !self.local.contains_key(path) {
                        return Some((path, content));
                    }
                    None
                })
                .map(|(path, content)| tokio::fs::write(path, content)),
        )
        .await
        .into_iter()
        .partition(|item| item.is_ok());

        if error.is_empty() {
            Ok(())
        } else {
            Err(Error::LocalSyncFailed)
        }
    }

    async fn load_local(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        match fs::read_dir(config.local_path.clone()) {
            Ok(entries) => {
                Ok(tokio_stream::iter(entries)
                    .fold(HashMap::new(), |mut acc, entry| {
                        if let Ok(file_entry) = entry {
                            let mut file = File::open(file_entry.path()).unwrap();
                            let mut buf = Vec::new();
                            //TODO: read file in chunks
                            let _ = file.read_to_end(&mut buf).map_err(|_| {
                                Error::LocalFileCorrupted(get_path_from_entry(&file_entry))
                            });
                            let file_hash = md5::compute(buf.clone());
                            acc.insert(get_path_from_entry(&file_entry), (file_hash, buf));
                        };
                        acc
                    })
                    .await)
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    Cync::create_default_directory(config).await?;
                    Ok(HashMap::new())
                } else {
                    Err(Error::FailedToLoadLocalFiles)
                }
            }
        }
    }

    async fn create_default_directory(config: &Config) -> Result<(), Error> {
        info!("Creating default directory");
        if create_dir(config.local_path.clone()).await.is_ok() {
            Ok(())
        } else {
            Err(Error::FailedToCreateDefaultDirectory)
        }
    }

    async fn fetch_remote(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        let mut remote = HashMap::new();
        let mut paginated_response = config
            .aws_client
            .list_objects_v2()
            .bucket(config.aws_bucket.clone())
            .max_keys(10)
            .into_paginator()
            .send();

        while let Some(result) = paginated_response.next().await {
            if let Ok(output) = result {
                for object in output.contents() {
                    if let Ok(remote_object) = config
                        .aws_client
                        .get_object()
                        .bucket(config.aws_bucket.clone())
                        .key(object.key().unwrap())
                        .send()
                        .await
                    {
                        let aggregated_bytes = remote_object.body.collect().await.unwrap();
                        remote.insert(
                            object.key().unwrap().to_string(),
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

        Ok(remote)
    }

    async fn sync_local_with_remote(&self) -> Result<(), Error> {
        let (_, error): (Vec<_>, Vec<_>) = futures::future::join_all(
            self.local
                .iter()
                .filter_map(|(path, (hash, content))| {
                    if let Some((remote_hash, _)) = self.remote.get(path) {
                        if hash != remote_hash {
                            return Some((path, content));
                        }
                    }
                    None
                })
                .map(|(path, content)| {
                    self.config
                        .aws_client
                        .put_object()
                        .bucket(self.config.aws_bucket.clone())
                        .key(path)
                        .body(ByteStream::from(content.clone()))
                        .send()
                }),
        )
        .await
        .into_iter()
        .partition(|item| item.is_ok());

        if error.is_empty() {
            Ok(())
        } else {
            Err(Error::RemoteSyncFailed)
        }
    }
}

fn get_path_from_entry(entry: &DirEntry) -> String {
    entry.path().as_path().to_str().unwrap().to_string()
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
