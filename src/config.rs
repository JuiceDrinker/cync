use crate::error::Error;
use aws_sdk_s3::Client;
use std::env::{self, VarError};

pub struct Config {
    pub aws_bucket: String,
    pub local_path: String,
    pub aws_client: Client,
}

impl Config {
    pub fn load_from_env(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        Ok(Config {
            local_path: env::var("LOCAL_PATH").map_err(|e| match e {
                VarError::NotPresent => Error::MissingEnvVar(String::from("LOCAL_PATH")),
                VarError::NotUnicode(p) => Error::LocalPathVariableCorrupt(p.into()),
            })?,
            aws_bucket: env::var("AWS_BUCKET_NAME").map_err(|e| match e {
                VarError::NotPresent => Error::MissingEnvVar(String::from("AWS_BUCKET_NAME")),
                VarError::NotUnicode(p) => Error::LocalPathVariableCorrupt(p.into()),
            })?,
            aws_client: aws_sdk_s3::Client::new(aws_config),
        })
    }

    pub fn local_path(&self) -> &str {
        &self.local_path
    }

    pub fn bucket_name(&self) -> &str {
        &self.aws_bucket
    }

    pub fn aws_client(&self) -> &Client {
        &self.aws_client
    }
}
