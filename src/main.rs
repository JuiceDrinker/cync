use aws_sdk_s3::error::ProvideErrorMetadata;
use std::{
    env::{self, VarError},
    ffi::OsStr,
    fs::{self, DirEntry, File},
    io::{self, Read},
};
use thiserror::Error;
use tokio::fs::create_dir;
use tokio_stream::StreamExt;
use tracing::{error, info};

#[derive(Error, Debug)]
pub enum Error {
    #[error("File hashing failed")]
    Hashing(HashingError),

    #[error("Missing env variable: {0}")]
    MissingEnvVar(String),

    #[error("Env variable corrupt")]
    LocalPathVariableCorrupt(Box<OsStr>),

    #[error("Could not read file at path: {0}")]
    LocalFileCorrupted(String),

    #[error("Sync with remote failed")]
    SyncFailed(String),
}

#[derive(Error, Debug)]
pub enum HashingError {
    #[error("File path provided does not exist")]
    PathNotFound { path: String },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let aws_config = aws_config::load_from_env().await;
    let aws_client = aws_sdk_s3::Client::new(&aws_config);

    let local_path = env::var("LOCAL_PATH").map_err(|e| match e {
        VarError::NotPresent => Error::MissingEnvVar(String::from("LOCAL_PATH")),
        VarError::NotUnicode(p) => Error::LocalPathVariableCorrupt(p.into()),
    })?;

    let aws_bucket = env::var("AWS_BUCKET_NAME").map_err(|e| match e {
        VarError::NotPresent => Error::MissingEnvVar(String::from("AWS_BUCKET_NAME")),
        VarError::NotUnicode(p) => Error::LocalPathVariableCorrupt(p.into()),
    })?;

    if let Ok(buckets) = aws_client.list_buckets().send().await {
        let bucket = buckets.buckets();
        for b in bucket {
            info!("{:?}", b);
        }
    }
    match fs::read_dir(local_path) {
        Ok(entries) => {
            info!("going thru dirs");
            let mut stream = tokio_stream::iter(entries);
            while let Some(entry) = stream.next().await {
                if let Ok(file_entry) = entry {
                    let mut file = File::open(file_entry.path()).unwrap();
                    let mut buf = Vec::new();
                    //TODO: read file in chunks
                    let _ = file
                        .read_to_end(&mut buf)
                        .map_err(|_| Error::LocalFileCorrupted(get_path_from_entry(&file_entry)));
                    let file_hash = md5::compute(buf.clone());
                    match aws_client
                        .get_object()
                        .bucket("cync-buck")
                        .key(get_path_from_entry(&file_entry))
                        .send()
                        .await
                    {
                        Ok(object) => {
                            let object_hash =
                                md5::compute(object.body.collect().await.unwrap().into_bytes());
                            if object_hash != file_hash {
                                let _ = aws_client
                                    .put_object()
                                    .bucket(aws_bucket.clone())
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
                            info!("Here?");
                            info!("{}", e.code().unwrap());
                            if e.code() == Some("NoSuchKey") {
                                info!("Hereee?");
                                let _ = aws_client
                                    .put_object()
                                    .bucket(aws_bucket.clone())
                                    .key(get_path_from_entry(&file_entry))
                                    .body(buf.into())
                                    .send()
                                    .await
                                    .map_err(|e| {
                                        error!("{}", e.code().unwrap());
                                        error!("{}", e.message().unwrap());
                                    });
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

    Ok(())
}

fn get_path_from_entry(entry: &DirEntry) -> String {
    entry.path().as_path().to_str().unwrap().to_string()
}
