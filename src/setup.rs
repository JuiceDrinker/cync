use crate::{
    cync::config::ConfigFile,
    error::{ConfigFileErrorKind, Error, SetupWizardErrorKind},
};
use requestty::Question;
use std::{collections::HashMap, fs, io::Write, sync::Arc};

// TODO: If config file already exists, read config file.
// If expected directories don't exist -> create the directories (currently we create ~/cync and
// ~/.config/.cync)
// If directories do exist -> skip that step (currently we would panic)

pub async fn run_setup_wizard() -> Result<(), Error> {
    let questions = vec![
        Question::input("local_directory")
            .message("Provide the name of the local directory to create, defaulted to .cync if left empty")
            .default(".cync")
            .build(),
        Question::input("remote_directory")
            .message("Provide the name of the remote directory to create, if left empty will be named cync")
            .default("cync")
            .build(),
    ];

    let answers = Arc::new(
        requestty::prompt(questions)
            .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::Prompt))?
            .into_iter()
            .fold(HashMap::new(), |mut acc, (question, answer)| {
                let ans = match answer {
                    requestty::Answer::String(v) => v,
                    _ => unreachable!(),
                };
                acc.insert(question, ans);
                acc
            }),
    );

    let local_directory_name = answers
        .get(&String::from("local_directory"))
        .expect("user must provide local directory name");

    let answers = Arc::clone(&answers);

    let remote_handle = tokio::spawn(async move {
        let remote_directory_name = answers
            .get(&String::from("remote_directory"))
            .expect("user must provide remote directory name");

        let aws_config = &aws_config::load_from_env().await;
        let aws_client = aws_sdk_s3::Client::new(aws_config);

        aws_client
            .create_bucket()
            .bucket(remote_directory_name)
            .send()
            .await
            .map(|_| remote_directory_name.clone())
            // TODO: Investigate AWS error types to be more explicit as to why operation failed
            // Two most likely erorrs are BucketAlreadyExists and invalid bucket names
            .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::BucketCreation))
    });

    let home_dir =
        home::home_dir().ok_or(Error::SetupWizard(SetupWizardErrorKind::HomeDirectory))?;
    let full_local_directory_path =
        format!("{}/{}", home_dir.display(), local_directory_name.clone());
    fs::create_dir(full_local_directory_path.clone()).map_err(|_| {
        Error::SetupWizard(SetupWizardErrorKind::LocalDirectoryCreation(
            full_local_directory_path.clone(),
        ))
    })?;

    let remote_directory_name = remote_handle
        .await
        .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::BucketCreation))??;

    let xdg_config = xdg::BaseDirectories::new().unwrap().get_config_home();
    let full_config_path = format!("{}.cync", xdg_config.display());

    let config_file = ConfigFile {
        remote_directory_name,
        local_directory_name: full_local_directory_path.into(),
    };

    let toml = toml::to_string(&config_file).unwrap();

    fs::create_dir(full_config_path.clone()).map_err(|_| {
        Error::SetupWizard(SetupWizardErrorKind::ConfigFile(
            ConfigFileErrorKind::Directory(full_config_path.clone()),
        ))
    })?;

    fs::File::create_new(format!("{}/config.toml", full_config_path))
        .map_err(|_| {
            Error::SetupWizard(SetupWizardErrorKind::ConfigFile(
                ConfigFileErrorKind::FileCreation(full_config_path.clone()),
            ))
        })?
        .write_all(toml.as_bytes())
        .map_err(|_| {
            Error::SetupWizard(SetupWizardErrorKind::ConfigFile(
                ConfigFileErrorKind::FileWrite(full_config_path.clone()),
            ))
        })?;

    Ok(())
}
