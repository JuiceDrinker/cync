use crate::error::Error;
use aws_sdk_s3::primitives::ByteStream;
use config::Config;
use std::sync::Arc;
use std::{collections::HashMap, fs, path::Path};
use tokio::fs::create_dir;
use tracing::info;
use util::walk_directory;

mod config;
mod error;
mod util;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    Cync::new(&aws_config::load_from_env().await)
        .await?
        .run_sync()
        .await?;

    info!("Successfully synced");

    Ok(())
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

    async fn load_local(config: &Config) -> Result<HashMap<FilePath, FileMetaData>, Error> {
        if fs::metadata(config.local_path())?.is_dir() {
            let local_files = walk_directory(Path::new(config.local_path()))?;
            info!("Found {} local files", local_files.keys().count());
            Ok(local_files)
        } else {
            info!("Could not find local directory");
            Cync::create_default_directory(config).await?;
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

    async fn sync_local_with_remote(&self) -> Result<(), Error> {
        let (successful, error): (Vec<_>, Vec<_>) = futures::future::join_all(
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
        .partition(Result::is_ok);

        if error.is_empty() {
            info!("Overwrote {} local files", successful.len());
            Ok(())
        } else {
            Err(Error::LocalSyncFailed)
        }
    }

    async fn sync_remote_with_local(&self) -> Result<(), Error> {
        let (successful, error): (Vec<_>, Vec<_>) = futures::future::join_all(
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
                        .bucket(self.config.bucket_name())
                        .key(path)
                        .body(ByteStream::from(content.clone()))
                        .send()
                }),
        )
        .await
        .into_iter()
        .partition(Result::is_ok);

        if error.is_empty() {
            info!("Overwrote {} remote files", successful.len());
            Ok(())
        } else {
            Err(Error::RemoteSyncFailed)
        }
    }
}
