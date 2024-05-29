use crate::error::Error;
use aws_sdk_s3::primitives::ByteStream;
use config::Config;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use error::TuiErrorKind;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stderr, Stdout, Stderr};
use std::sync::Arc;
use std::{collections::HashMap, fs, path::Path};
use tokio::fs::create_dir;
use tracing::info;
use util::walk_directory;

mod config;
mod error;
mod util;

async fn run_app(
    terminal: Terminal<CrosstermBackend<Stderr>>,
    app: &mut Cync,
) -> Result<bool, Error> {
    todo!()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    enable_raw_mode().map_err(|_| Error::Tui(error::TuiErrorKind::Initilization));
    let mut stderr = std::io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|_| Error::Tui(error::TuiErrorKind::Initilization));
    let backend = CrosstermBackend::new(stderr);
    let mut terminal =
        Terminal::new(backend).map_err(|_| Error::Tui(TuiErrorKind::Initilization))?;
    let aws_config = &aws_config::load_from_env().await;

    let mut app = Cync::new(aws_config).await?;
    let res = run_app(terminal, &mut app);

    disable_raw_mode().map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration));
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    terminal
        .show_cursor()
        .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    if let Ok(do_print) = res {
        if do_print {
            app.view_files();
        }
    } else if let Err(err) = res {
        println!("{err:?}");
    }
    Ok(())
}

pub struct App {
    config: Arc<Config>,
    remote: HashMap<FilePath, FileMetaData>,
    local: HashMap<FilePath, FileMetaData>,
}
type FilePath = String;
type FileHash = md5::Digest;
type FileContents = Vec<u8>;
type FileMetaData = (FileHash, FileContents);
enum Source {
    Remote,
    Local,
}

struct Cync {
    config: Arc<Config>,
    remote: HashMap<FilePath, FileMetaData>,
    local: HashMap<FilePath, FileMetaData>,
}

struct FileDetails {
    remote_hash: Option<md5::Digest>,
    local_hash: Option<md5::Digest>,
}
type FileViewer = HashMap<FilePath, FileDetails>;

impl Cync {
    async fn new(aws_config: &aws_config::SdkConfig) -> Result<Self, Error> {
        let config = Arc::new(Config::load_from_env(aws_config)?);
        Ok(Self {
            config: Arc::clone(&config),
            remote: Cync::fetch_remote(&config).await?,
            local: Cync::load_local(&config).await?,
        })
    }

    fn view_files(&self) -> FileViewer {
        self.remote
            .iter()
            .map(|(path, (hash, _))| (path, (Source::Remote, hash)))
            .chain(
                self.local
                    .iter()
                    .map(|(path, (hash, _))| (path, (Source::Local, hash))),
            )
            .fold(
                HashMap::<FilePath, FileDetails>::new(),
                |mut acc, (path, (source, hash))| {
                    match acc.get_mut(path) {
                        Some(existing) => match source {
                            Source::Remote => existing.remote_hash = Some(*hash),
                            Source::Local => existing.local_hash = Some(*hash),
                        },
                        None => match source {
                            Source::Remote => {
                                acc.insert(
                                    path.to_owned(),
                                    FileDetails {
                                        remote_hash: Some(*hash),
                                        local_hash: None,
                                    },
                                );
                            }
                            Source::Local => {
                                acc.insert(
                                    path.to_owned(),
                                    FileDetails {
                                        remote_hash: Some(*hash),
                                        local_hash: None,
                                    },
                                );
                            }
                        },
                    };
                    acc
                },
            )
    }

    async fn run_sync(self) -> Result<(), Error> {
        self.sync_local_with_remote().await?;
        self.sync_remote_with_local().await?;
        Ok(())
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
