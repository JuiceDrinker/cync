use std::{
    collections::HashMap,
    fs::{self, DirEntry, File},
    io::Read,
    path::Path,
};

use crate::{
    app::{FileMetaData, FilePath},
    error::{Error, LoadingLocalFiles},
};

pub fn walk_directory(path: &Path) -> Result<HashMap<FilePath, FileMetaData>, Error> {
    let mut result = HashMap::new();
    for entry in
        fs::read_dir(path).map_err(|_| Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem))?
    {
        let entry = entry.map_err(|_| Error::LoadingLocalFiles(LoadingLocalFiles::FileSystem))?;
        if entry.path().is_dir() {
            if let Ok(next_level) = walk_directory(&entry.path()) {
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
            result.insert(get_path_from_entry(&entry), (file_hash, buf));
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
