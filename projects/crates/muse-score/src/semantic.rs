use muse_music::{Meter, Note, Score};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationValue {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    Other(u64),
}

impl DurationValue {
    pub fn from_ticks(ticks: u64, quarter: u32) -> Self {
        let q = u64::from(quarter);
        match ticks {
            x if x == q * 4 => Self::Whole,
            x if x == q * 2 => Self::Half,
            x if x == q => Self::Quarter,
            x if x * 2 == q => Self::Eighth,
            x if x * 4 == q => Self::Sixteenth,
            x if x * 8 == q => Self::ThirtySecond,
            x => Self::Other(x),
        }
    }
    pub fn beam_count(self) -> u8 {
        match self {
            Self::Sixteenth => 2,
            Self::ThirtySecond => 3,
            Self::Eighth => 1,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteColumn {
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub duration: DurationValue,
    pub notes: Vec<NoteRef>,
    pub rest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRef {
    pub key: u8,
    pub velocity: u8,
    pub articulation: String,
    pub tie_from_previous: bool,
    pub tie_to_next: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measure {
    pub index: u32,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub voices: Vec<VoiceMeasure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceMeasure {
    pub track: usize,
    pub name: String,
    pub role: String,
    pub columns: Vec<NoteColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticScore {
    pub title: String,
    pub key: String,
    pub meter: Meter,
    pub ticks_per_quarter: u32,
    pub measures: Vec<Measure>,
}

pub fn normalize(score: &Score) -> SemanticScore {
    let beats = u64::from(score.meter.beats.max(1));
    let measure_ticks = beats * u64::from(score.ticks_per_quarter) * 4 / u64::from(score.meter.beat_unit.max(1));
    let end = score
        .notes
        .iter()
        .map(|n| n.start_tick + n.duration_ticks)
        .max()
        .unwrap_or(measure_ticks);
    let measure_count = end.div_ceil(measure_ticks).max(1) as u32;
    let mut measures = Vec::with_capacity(measure_count as usize);
    for index in 0..measure_count {
        let start = u64::from(index) * measure_ticks;
        let voices = score
            .tracks
            .iter()
            .enumerate()
            .map(|(track, info)| VoiceMeasure {
                track,
                name: info.name.clone(),
                role: info.role.clone(),
                columns: columns_for(score, track, start, start + measure_ticks, measure_ticks),
            })
            .collect();
        measures.push(Measure {
            index,
            start_tick: start,
            duration_ticks: measure_ticks,
            voices,
        });
    }
    SemanticScore {
        title: score.title.clone().unwrap_or_else(|| "Muse composition".into()),
        key: score.key.clone(),
        meter: score.meter.clone(),
        ticks_per_quarter: score.ticks_per_quarter,
        measures,
    }
}

fn columns_for(score: &Score, track: usize, start: u64, end: u64, measure_ticks: u64) -> Vec<NoteColumn> {
    let mut by_tick: BTreeMap<u64, Vec<&Note>> = BTreeMap::new();
    for note in score
        .notes
        .iter()
        .filter(|n| n.track == track && n.start_tick < end && n.start_tick + n.duration_ticks > start)
    {
        by_tick.entry(note.start_tick.max(start)).or_default().push(note);
    }
    let mut columns = Vec::new();
    let mut cursor = start;
    for (tick, notes) in by_tick {
        if tick > cursor {
            columns.push(column(cursor, tick - cursor, Vec::new(), score.ticks_per_quarter, true));
        }
        let duration = notes
            .iter()
            .map(|n| (n.start_tick + n.duration_ticks).saturating_sub(tick))
            .min()
            .unwrap_or(1)
            .min(end - tick);
        let refs = notes
            .into_iter()
            .map(|n| NoteRef {
                key: n.key,
                velocity: n.velocity,
                articulation: n.articulation.clone(),
                tie_from_previous: n.start_tick < start,
                tie_to_next: n.start_tick + n.duration_ticks > end,
            })
            .collect();
        columns.push(column(tick, duration, refs, score.ticks_per_quarter, false));
        cursor = (tick + duration).min(end);
    }
    if cursor < end {
        columns.push(column(cursor, end - cursor, Vec::new(), score.ticks_per_quarter, true));
    }
    if columns.is_empty() {
        columns.push(column(start, measure_ticks, Vec::new(), score.ticks_per_quarter, true));
    }
    columns
}

fn column(start_tick: u64, duration_ticks: u64, notes: Vec<NoteRef>, quarter: u32, rest: bool) -> NoteColumn {
    NoteColumn {
        start_tick,
        duration_ticks,
        duration: DurationValue::from_ticks(duration_ticks, quarter),
        notes,
        rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muse_music::{Meter, Note, Score, Track};
    #[test]
    fn groups_chords_and_inserts_rests() {
        let score = Score {
            meter: Meter { beats: 4, beat_unit: 4 },
            tracks: vec![Track {
                name: "Piano".into(),
                channel: 0,
                program: 0,
                role: "melody".into(),
            }],
            notes: vec![
                Note {
                    track: 0,
                    channel: 0,
                    key: 60,
                    velocity: 100,
                    start_tick: 480,
                    duration_ticks: 480,
                    articulation: "".into(),
                },
                Note {
                    track: 0,
                    channel: 0,
                    key: 64,
                    velocity: 100,
                    start_tick: 480,
                    duration_ticks: 480,
                    articulation: "".into(),
                },
            ],
            ..Score::default()
        };
        let normalized = normalize(&score);
        assert_eq!(normalized.measures[0].voices[0].columns.len(), 3);
        assert!(normalized.measures[0].voices[0].columns[0].rest);
        assert_eq!(normalized.measures[0].voices[0].columns[1].notes.len(), 2);
    }

    #[test]
    fn splits_sustained_notes_across_measure_boundaries() {
        let mut score = Score::default();
        score.tracks.push(Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "motif".into(),
        });
        score.notes.push(Note {
            track: 0,
            channel: 0,
            key: 60,
            velocity: 90,
            start_tick: 1_440,
            duration_ticks: 960,
            articulation: String::new(),
        });
        let normalized = normalize(&score);
        assert_eq!(normalized.measures.len(), 2);
        let first = normalized.measures[0].voices[0]
            .columns
            .iter()
            .find(|column| !column.rest)
            .unwrap();
        let second = normalized.measures[1].voices[0]
            .columns
            .iter()
            .find(|column| !column.rest)
            .unwrap();
        assert!(first.notes[0].tie_to_next);
        assert!(second.notes[0].tie_from_previous);
    }
}
