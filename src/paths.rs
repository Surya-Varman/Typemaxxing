//! Where settings and history are stored:
//! macOS:   ~/Library/Application Support/typemaxxing/
//! Linux:   ~/.local/share/typemaxxing/
//! Windows: %APPDATA%\typemaxxing\data\
//!
//! The app used to be called "typetest". The first time the new folder is
//! needed, files from the old folder are copied over so nobody loses their
//! history or settings.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

const APP: &str = "typemaxxing";
const OLD_APP: &str = "typetest";

pub fn data_dir() -> Result<PathBuf> {
    let dir = ProjectDirs::from("", "", APP)
        .context("could not find a home directory to store data in")?
        .data_dir()
        .to_path_buf();
    if !dir.exists() {
        migrate_from_old_name(&dir);
    }
    Ok(dir)
}

/// Copy settings.json and history.jsonl from the old "typetest" folder, if any.
/// Best effort: if anything fails, the app simply starts fresh.
fn migrate_from_old_name(new_dir: &PathBuf) {
    let Some(old) = ProjectDirs::from("", "", OLD_APP) else {
        return;
    };
    let old_dir = old.data_dir();
    if !old_dir.is_dir() || fs::create_dir_all(new_dir).is_err() {
        return;
    }
    for name in ["settings.json", "history.jsonl"] {
        let from = old_dir.join(name);
        if from.is_file() {
            let _ = fs::copy(&from, new_dir.join(name));
        }
    }
}
