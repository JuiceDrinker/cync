use crate::error::Error;
use aws_sdk_s3::Client;
use std::{fs, path::PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConfigFile {
    pub aws_bucket_name: String,
    pub local_directory: PathBuf,
}

pub struct Config {
    pub aws_bucket_name: String,
    pub local_directory: PathBuf,
    pub aws_client: Client,
}

impl Config {
    pub fn load(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        // TODO: Handle errors
        let config =
            toml::from_str::<ConfigFile>(&String::from_utf8(Config::get_config()).unwrap())
                .unwrap();

        Ok(Config {
            local_directory: config.local_directory,
            aws_bucket_name: config.aws_bucket_name,
            aws_client: aws_sdk_s3::Client::new(aws_config),
        })
    }

    fn get_config() -> Vec<u8> {
        // TODO: Handle errors
        fs::read(
            xdg::BaseDirectories::with_prefix("cync")
                .unwrap()
                .get_config_file("cync"),
        )
        .unwrap()
    }

    pub fn local_path(&self) -> &PathBuf {
        &self.local_directory
    }

    pub fn bucket_name(&self) -> &str {
        &self.aws_bucket_name
    }

    pub fn aws_client(&self) -> &Client {
        &self.aws_client
    }
}
