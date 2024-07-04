use crate::{
    config::ConfigFile,
    error::{Error, SetupWizardErrorKind},
};
use requestty::Question;
use std::{collections::HashMap, fs, sync::Arc};

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
                    _ => panic!("Must pass string"),
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

        match aws_client
            .create_bucket()
            .bucket(remote_directory_name)
            .send()
            .await
        {
            Ok(_) => Ok(remote_directory_name.clone()),
            Err(_) => Err(Error::SetupWizard(SetupWizardErrorKind::BucketCreation)),
        }
    });

    let home_dir = match home::home_dir() {
        Some(v) => Ok(v),
        None => Err(Error::SetupWizard(SetupWizardErrorKind::HomeDirectory)),
    }?;

    fs::create_dir(format!(
        "{}/{}",
        home_dir.display(),
        local_directory_name.clone()
    ))
    .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::LocalDirectoryCreation))?;

    let remote_directory_name = remote_handle
        .await
        .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::BucketCreation))??;

    let config_file = ConfigFile {
        remote_directory_name: remote_directory_name.to_string(),
        local_directory_name: local_directory_name.into(),
    };

    let toml = toml::to_string(&config_file).unwrap();

    let xdg_config = xdg::BaseDirectories::with_prefix("cync")
        .unwrap()
        .get_config_home();

    fs::write(format!("{}/config.toml", xdg_config.display()), toml)
        .map_err(|_| Error::SetupWizard(SetupWizardErrorKind::ConfigFile))?;

    Ok(())
}
