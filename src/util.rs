use std::{
    collections::HashMap,
    fs::{self, DirEntry, File},
    io::{Read, Stderr},
    path::PathBuf,
};

use crate::{
    app::{FileMetaData, FilePath},
    error::{self, Error, LoadingLocalFiles, TuiErrorKind},
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{prelude::CrosstermBackend, Terminal};

// TODO: Make more readable
pub fn walk_directory(
    path: &PathBuf,
    top_level_path: &PathBuf,
) -> Result<HashMap<FilePath, FileMetaData>, Error> {
    let mut result = HashMap::new();
    for entry in
        fs::read_dir(path).map_err(|_| Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem))?
    {
        let entry = entry.map_err(|_| Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem))?;
        if entry.path().is_dir() {
            if let Ok(next_level) = walk_directory(&entry.path(), top_level_path) {
                result.extend(next_level);
            } else {
                return Err(Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem));
            }
        } else {
            let mut buf = Vec::new();
            let _ = File::open(entry.path())
                .map(|mut file| file.read_to_end(&mut buf))
                .map_err(|_| Error::LocalFileCorrupted(get_path_from_entry(&entry)));
            let file_hash = md5::compute(buf.clone());
            if let Some(local_path) = get_path_from_entry(&entry)
                .strip_prefix(&format!("{}/", top_level_path.as_path().display()))
            {
                result.insert(local_path.to_string(), (file_hash, buf));
            } else {
                panic!()
            }
        }
    }

    Ok(result)
}

pub fn get_path_from_entry(entry: &DirEntry) -> String {
    entry
        .path()
        .as_path()
        .to_str()
        .expect("path to be utf-8")
        .to_string()
}

pub fn initialize_terminal() -> Result<Terminal<CrosstermBackend<Stderr>>, Error> {
    enable_raw_mode().map_err(|_| Error::Tui(TuiErrorKind::Initilization))?;
    let mut stderr = std::io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|_| Error::Tui(error::TuiErrorKind::Initilization))?;
    let backend = CrosstermBackend::new(stderr);
    let terminal = Terminal::new(backend).map_err(|_| Error::Tui(TuiErrorKind::Initilization))?;
    Ok(terminal)
}

pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stderr>>) -> Result<(), Error> {
    disable_raw_mode().map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    terminal
        .show_cursor()
        .map_err(|_| Error::Tui(TuiErrorKind::TerminalRestoration))?;

    Ok(())
}
