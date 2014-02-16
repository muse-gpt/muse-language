//! Neutral score model shared by Muse composition, interchange, and synthesis.
use serde::{Deserialize, Serialize};

pub const TICKS_PER_QUARTER: u32 = 480;
pub const SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Score {
    pub title: Option<String>,
    pub ticks_per_quarter: u32,
    pub tempo_bpm: f64,
    pub tempo_events: Vec<TempoEvent>,
    pub meter: Meter,
    pub key: String,
    pub soundfont: Option<String>,
    pub tracks: Vec<Track>,
    pub notes: Vec<Note>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TempoEvent {
    pub start_tick: u64,
    pub bpm: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meter {
    pub beats: u8,
    pub beat_unit: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub name: String,
    pub channel: u8,
    pub program: u8,
    pub role: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub track: usize,
    pub channel: u8,
    pub key: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub articulation: String,
}
impl Default for Meter {
    fn default() -> Self {
        Self { beats: 4, beat_unit: 4 }
    }
}
impl Default for Score {
    fn default() -> Self {
        Self {
            title: None,
            ticks_per_quarter: TICKS_PER_QUARTER,
            tempo_bpm: 120.0,
            tempo_events: vec![TempoEvent {
                start_tick: 0,
                bpm: 120.0,
            }],
            meter: Meter::default(),
            key: "C major".into(),
            soundfont: None,
            tracks: Vec::new(),
            notes: Vec::new(),
        }
    }
}
