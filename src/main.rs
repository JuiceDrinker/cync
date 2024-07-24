use crate::error::Error;
use clap::Parser;
use cync::Cync;
use logging::initialize_logging;
use setup::run_setup_wizard;
use tui::run_tui;
use util::{initialize_terminal, restore_terminal};

mod cync;
mod error;
mod logging;
mod s3;
mod setup;
mod tui;
mod util;

#[derive(Parser)]
struct Args {
    init: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    initialize_logging()?;
    // NOTE: According to AWS docs(https://docs.aws.amazon.com/sdkref/latest/guide/environment-variables.html)
    // enviornemnt variables take precedence over config file
    // i.e. We can set these to override what is inside the config file
    let aws_config = &aws_config::load_from_env().await;

    let Args { init } = Args::parse();

    let res = if init.is_some() {
        run_setup_wizard().await
    } else {
        let mut terminal = initialize_terminal()?;
        let mut app = Cync::new(aws_config).await?;
        let app_res = run_tui(&mut terminal, &mut app).await;
        // TODO: Restoration seems to be broken if app panics?
        restore_terminal(terminal)?;
        app_res
    };

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
