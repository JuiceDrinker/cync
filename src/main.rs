use crate::error::Error;
use aws_sdk_s3::error::ProvideErrorMetadata;
use config::Config;
use std::{
    env::{self},
    fs::{self, DirEntry, File},
    io::{self, Read},
};
use tokio::fs::create_dir;
use tokio_stream::StreamExt;
use tracing::info;

mod config;
mod error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();
    let config = Config::load_from_env(&aws_config::load_from_env().await).unwrap();

    Cync::run_sync(config).await;
    Ok(())
}

struct Cync {}

impl Cync {
    async fn run_sync(config: Config) {
        match fs::read_dir(config.local_path) {
            Ok(entries) => {
                let mut stream = tokio_stream::iter(entries);
                while let Some(entry) = stream.next().await {
                    if let Ok(file_entry) = entry {
                        let mut file = File::open(file_entry.path()).unwrap();
                        let mut buf = Vec::new();
                        //TODO: read file in chunks
                        let _ = file.read_to_end(&mut buf).map_err(|_| {
                            Error::LocalFileCorrupted(get_path_from_entry(&file_entry))
                        });
                        let file_hash = md5::compute(buf.clone());
                        match config
                            .aws_client
                            .get_object()
                            .bucket(config.aws_bucket.clone())
                            .key(get_path_from_entry(&file_entry))
                            .send()
                            .await
                        {
                            Ok(object) => {
                                let object_hash =
                                    md5::compute(object.body.collect().await.unwrap().into_bytes());
                                if object_hash != file_hash {
                                    let _ = config
                                        .aws_client
                                        .put_object()
                                        .bucket(config.aws_bucket.clone())
                                        .key(get_path_from_entry(&file_entry))
                                        .body(buf.into())
                                        .send()
                                        .await
                                        .map_err(|_| {
                                            Error::SyncFailed(
                                                file_entry
                                                    .path()
                                                    .as_path()
                                                    .to_str()
                                                    .unwrap()
                                                    .to_string(),
                                            )
                                        });
                                }
                            }
                            Err(e) => {
                                if e.code() == Some("NoSuchKey") {
                                    let _ = config
                                        .aws_client
                                        .put_object()
                                        .bucket(config.aws_bucket.clone())
                                        .key(get_path_from_entry(&file_entry))
                                        .body(buf.into())
                                        .send()
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let _ = create_dir(env::var("LOCAL_PATH").unwrap()).await;
                    info!("Done creating local directory");
                }
            }
        }
    }
}

fn get_path_from_entry(entry: &DirEntry) -> String {
    entry.path().as_path().to_str().unwrap().to_string()
}
