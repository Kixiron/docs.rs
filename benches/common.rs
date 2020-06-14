use std::{fs, path::PathBuf};

/// Reads all the files in the html directory, returning their name and path
pub fn get_files() -> Vec<(String, PathBuf)> {
    fs::read_dir("benches/html")
        .unwrap()
        .into_iter()
        .filter_map(|d| {
            d.ok().and_then(|file| {
                if file.file_type().unwrap().is_file() {
                    let name = file.file_name().to_string_lossy().to_string();

                    Some((name, file.path()))
                } else {
                    None
                }
            })
        })
        .collect()
}
