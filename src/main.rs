use std::{
    fs::{self},
    path::Path,
};
use aws_config::meta::region::RegionProviderChain;
use thiserror::Error;
use tracing::{error, info};

#[derive(Error, Debug)]
pub enum Error {
    #[error("File hashing failed")]
    Hashing(HashingError),
}

#[derive(Error, Debug)]
pub enum HashingError {
    #[error("File path provided does not exist")]
    PathNotFound { path: String },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let aws_config = aws_config::load_from_env().await;
    let aws_client = aws_sdk_s3::Client::new(&aws_config);

    match aws_client
        .get_object()
        .bucket("cync-buck")
        .send()
        .await
    {
        Ok(_) => {
            info!("Got object")
        }
        Err(e) => {
            error!("Error: {:?}", e.into_source())
        }
    }
}

fn compute_hash_for_path(path: &Path) -> Result<md5::Digest, Error> {
    fs::read(path)
        .map_err(|_| {
            Error::Hashing(HashingError::PathNotFound {
                path: path.as_os_str().to_str().unwrap().to_string(),
            })
        })
        .map(md5::compute)
}
