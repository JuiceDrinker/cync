use crate::{error::Error, s3::S3Client};
use std::{fs, path::PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConfigFile {
    pub remote_directory_name: String,
    pub local_directory_name: PathBuf,
}

pub struct Config {
    pub remote_directory_name: String,
    pub local_directory_name: PathBuf,
    pub aws_client: S3Client,
}

impl Config {
    pub fn load(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = toml::from_str::<ConfigFile>(
            &String::from_utf8(Config::get_config_file_path()?)
                .map_err(|_| Error::ConfigFileMissing)?,
        )
        .map_err(|_| Error::ConfigFileCorrupted)?;

        Ok(Config {
            local_directory_name: config.local_directory_name,
            remote_directory_name: config.remote_directory_name,
            aws_client: S3Client::new(aws_sdk_s3::Client::new(aws_config)),
        })
    }

    fn get_config_file_path() -> Result<Vec<u8>, Error> {
        let config_file_path = fs::read(
            xdg::BaseDirectories::with_prefix(".cync")
                .map_err(|_| Error::ConfigFileCorrupted)?
                .get_config_file("config.toml"),
        )
        .map_err(|_| Error::ConfigFileMissing)?;

        Ok(config_file_path)
    }

    pub fn local_directory(&self) -> &PathBuf {
        &self.local_directory_name
    }

    pub fn remote_directory(&self) -> &str {
        &self.remote_directory_name
    }

    pub fn aws_client(&self) -> &S3Client {
        &self.aws_client
    }
}
