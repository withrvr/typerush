use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub wpm: f64,
    pub accuracy: f64,
    pub mode: String,
    pub word_count: usize,
    pub correct_chars: usize,
    pub total_chars: usize,
    pub duration_secs: f64,
    pub timestamp: DateTime<Local>,
}

pub fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".typerush")
}

pub fn stats_path() -> PathBuf {
    data_dir().join("stats.json")
}

pub fn save_session(record: &SessionRecord) -> Result<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let path = stats_path();
    let mut records: Vec<SessionRecord> = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        vec![]
    };
    records.push(record.clone());
    fs::write(&path, serde_json::to_string_pretty(&records)?)?;
    Ok(())
}

pub fn load_sessions() -> Result<Vec<SessionRecord>> {
    let path = stats_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn personal_best(sessions: &[SessionRecord]) -> Option<f64> {
    sessions.iter().map(|s| s.wpm).fold(None, |best, w| match best {
        None => Some(w),
        Some(b) if w > b => Some(w),
        Some(b) => Some(b),
    })
}

pub fn average_accuracy(sessions: &[SessionRecord]) -> Option<f64> {
    if sessions.is_empty() { return None; }
    let sum: f64 = sessions.iter().map(|s| s.accuracy).sum();
    Some(sum / sessions.len() as f64)
}
