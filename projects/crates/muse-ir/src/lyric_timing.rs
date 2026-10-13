//! External lyric timing JSON (doc 14 §3). Loaded at compile when `lyric { from … }`.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{fs, path::Path};

use crate::{LyricLine, LyricWord};

#[derive(Debug, Deserialize)]
struct TimingFile {
    #[serde(default)]
    lang: Option<String>,
    #[serde(default = "default_time_unit")]
    time_unit: String,
    lines: Vec<TimingLine>,
}

fn default_time_unit() -> String {
    "seconds".into()
}

#[derive(Debug, Deserialize)]
struct TimingLine {
    #[serde(default)]
    id: Option<String>,
    /// Stable CueId for cross-locale display (docs 18/20). Independent of vocal word index.
    #[serde(default)]
    cue_id: Option<String>,
    words: Vec<TimingWord>,
}

#[derive(Debug, Deserialize)]
struct TimingWord {
    #[serde(default)]
    id: Option<String>,
    text: String,
    start: f64,
    end: f64,
}

/// Load a doc-14 timing JSON into IR lines + optional lang.
pub fn load_timing_json(path: &Path) -> Result<(Option<String>, Vec<LyricLine>)> {
    let raw = fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let file: TimingFile = serde_json::from_str(&raw)
        .with_context(|| format!("invalid lyric timing JSON {}", path.display()))?;
    if file.time_unit != "seconds" {
        bail!(
            "lyric timing `{}` time_unit `{}` is unsupported; use `seconds`",
            path.display(),
            file.time_unit
        );
    }
    if file.lines.is_empty() {
        bail!("lyric timing `{}` has no lines", path.display());
    }
    let mut lines = Vec::with_capacity(file.lines.len());
    for (line_idx, line) in file.lines.iter().enumerate() {
        if line.words.is_empty() {
            bail!(
                "lyric timing `{}` line {} has no words",
                path.display(),
                line_idx + 1
            );
        }
        let mut words = Vec::with_capacity(line.words.len());
        for (word_idx, word) in line.words.iter().enumerate() {
            if !(word.end > word.start) || !word.start.is_finite() || !word.end.is_finite() {
                bail!(
                    "lyric timing `{}` word `{}` needs finite end > start",
                    path.display(),
                    word.text
                );
            }
            words.push(LyricWord {
                id: word
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("W{}", word_idx + 1)),
                text: word.text.clone(),
                start: word.start,
                end: word.end,
            });
        }
        let cue_id = line
            .cue_id
            .as_ref()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        lines.push(LyricLine {
            id: line
                .id
                .clone()
                .unwrap_or_else(|| format!("L{}", line_idx + 1)),
            cue_id,
            words,
        });
    }
    Ok((file.lang, lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, time::{SystemTime, UNIX_EPOCH}};

    #[test]
    fn loads_seconds_timing() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = env::temp_dir().join(format!("muse-lyric-{stamp}.json"));
        fs::write(
            &path,
            r#"{
  "version": 1,
  "lang": "zh-Hans",
  "time_unit": "seconds",
  "lines": [
    { "id": "L1", "cue_id": "C1", "words": [
      { "id": "W1", "text": "残", "start": 0.0, "end": 0.4 },
      { "text": "响", "start": 0.4, "end": 0.8 }
    ]}
  ]
}"#,
        )
        .unwrap();
        let (lang, lines) = load_timing_json(&path).unwrap();
        assert_eq!(lang.as_deref(), Some("zh-Hans"));
        assert_eq!(lines[0].cue_id.as_deref(), Some("C1"));
        assert_eq!(lines[0].words[0].text, "残");
        assert_eq!(lines[0].words[1].id, "W2");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn blank_cue_id_becomes_none() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = env::temp_dir().join(format!("muse-lyric-cue-{stamp}.json"));
        fs::write(
            &path,
            r#"{
  "version": 1,
  "time_unit": "seconds",
  "lines": [
    { "id": "L1", "cue_id": "  ", "words": [
      { "text": "a", "start": 0.0, "end": 0.2 }
    ]}
  ]
}"#,
        )
        .unwrap();
        let (_, lines) = load_timing_json(&path).unwrap();
        assert_eq!(lines[0].cue_id, None);
        let _ = fs::remove_file(path);
    }
}
