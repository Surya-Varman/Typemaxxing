//! Saves results to a local JSON Lines file (one result per line).
//! Stored as history.jsonl in the data folder (see paths.rs).

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::stats::Stats;
use crate::typing::Mode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub date: DateTime<Local>,
    pub mode: String,
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub seconds: f64,
}

impl Record {
    pub fn new(mode: Mode, stats: &Stats) -> Self {
        Self {
            date: Local::now(),
            mode: mode.label(),
            wpm: stats.wpm,
            raw_wpm: stats.raw_wpm,
            accuracy: stats.accuracy,
            seconds: stats.seconds,
        }
    }
}

pub fn path() -> Result<PathBuf> {
    Ok(crate::paths::data_dir()?.join("history.jsonl"))
}

pub fn append(record: &Record) -> Result<()> {
    let path = path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

/// Loads all saved results, oldest first. Lines that fail to parse are skipped.
pub fn load() -> Result<Vec<Record>> {
    let path = path()?;
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("opening {}", path.display())),
    };
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect())
}

/// Best WPM so far for a given mode, if any.
pub fn best_wpm(records: &[Record], mode: Mode) -> Option<f64> {
    let label = mode.label();
    records
        .iter()
        .filter(|r| r.mode == label)
        .map(|r| r.wpm)
        .fold(None, |best, w| Some(best.map_or(w, |b: f64| b.max(w))))
}

/// `tmx history`: print recent results and a summary per mode.
pub fn print(limit: usize) -> Result<()> {
    let records = load()?;
    if records.is_empty() {
        println!("No results yet. Run `tmx` to take your first test.");
        return Ok(());
    }

    println!(
        "{:<17} {:<10} {:>6} {:>6} {:>7}",
        "date", "mode", "wpm", "raw", "acc"
    );
    let start = records.len().saturating_sub(limit);
    for r in &records[start..] {
        println!(
            "{:<17} {:<10} {:>6.0} {:>6.0} {:>6.0}%",
            r.date.format("%Y-%m-%d %H:%M"),
            r.mode,
            r.wpm,
            r.raw_wpm,
            r.accuracy
        );
    }

    println!();
    let mut modes: Vec<&str> = records.iter().map(|r| r.mode.as_str()).collect();
    modes.sort();
    modes.dedup();
    for mode in modes {
        let wpms: Vec<f64> = records
            .iter()
            .filter(|r| r.mode == mode)
            .map(|r| r.wpm)
            .collect();
        let best = wpms.iter().cloned().fold(f64::MIN, f64::max);
        let avg = wpms.iter().sum::<f64>() / wpms.len() as f64;
        let last10 = &wpms[wpms.len().saturating_sub(10)..];
        let avg10 = last10.iter().sum::<f64>() / last10.len() as f64;
        println!(
            "{mode:<10} tests {:>4}   best {best:>4.0}   avg {avg:>4.0}   last 10 avg {avg10:>4.0}",
            wpms.len()
        );
    }
    println!("\nSaved in {}", path()?.display());
    Ok(())
}
