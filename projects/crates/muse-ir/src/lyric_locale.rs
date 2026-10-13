//! Localized lyric display catalog keyed by CueId (docs 18/20).
//!
//! Vocal (source) timing stays on [`crate::LyricLine`]; this table only supplies
//! per-locale display text and highlight policy. Word-index mapping across
//! languages is intentionally unsupported.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// How a locale paints highlight relative to source vocal timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LyricHighlightPolicy {
    /// Use source vocal word/syllable windows (typically source locale).
    Word,
    /// Emphasize a reviewed phrase span inside the display line.
    Phrase,
    /// Highlight the whole display line for the cue window.
    #[default]
    Line,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedLyricText {
    pub text: String,
    #[serde(default)]
    pub highlight: LyricHighlightPolicy,
    /// Optional display-only segment times in seconds (not vocal word indices).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub display_segments: Vec<DisplaySegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedLyricCue {
    pub cue_id: String,
    /// locale (BCP 47) → display payload
    pub localized: BTreeMap<String, LocalizedLyricText>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedLyricCatalog {
    #[serde(default = "default_version")]
    pub version: u32,
    pub source_locale: String,
    pub cues: Vec<LocalizedLyricCue>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default = "default_version")]
    version: u32,
    source_locale: String,
    cues: Vec<CueFile>,
}

#[derive(Debug, Deserialize)]
struct CueFile {
    cue_id: String,
    localized: BTreeMap<String, TextFile>,
}

#[derive(Debug, Deserialize)]
struct TextFile {
    text: String,
    #[serde(default)]
    highlight: LyricHighlightPolicy,
    #[serde(default)]
    display_segments: Vec<DisplaySegment>,
}

/// Load a localized lyric catalog JSON (display side only).
pub fn load_localized_lyric_catalog(path: &Path) -> Result<LocalizedLyricCatalog> {
    let raw = fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let file: CatalogFile = serde_json::from_str(&raw)
        .with_context(|| format!("invalid localized lyric catalog {}", path.display()))?;
    if file.version == 0 {
        bail!(
            "localized lyric catalog `{}` version must be >= 1",
            path.display()
        );
    }
    if file.source_locale.trim().is_empty() {
        bail!(
            "localized lyric catalog `{}` missing source_locale",
            path.display()
        );
    }
    if file.cues.is_empty() {
        bail!(
            "localized lyric catalog `{}` has no cues",
            path.display()
        );
    }
    let mut seen = BTreeMap::new();
    let mut cues = Vec::with_capacity(file.cues.len());
    for cue in file.cues {
        let cue_id = cue.cue_id.trim().to_owned();
        if cue_id.is_empty() {
            bail!(
                "localized lyric catalog `{}` has empty cue_id",
                path.display()
            );
        }
        if seen.insert(cue_id.clone(), ()).is_some() {
            bail!(
                "localized lyric catalog `{}` duplicate cue_id `{cue_id}`",
                path.display()
            );
        }
        if cue.localized.is_empty() {
            bail!(
                "localized lyric catalog `{}` cue `{cue_id}` has no localized entries",
                path.display()
            );
        }
        let mut localized = BTreeMap::new();
        for (locale, text) in cue.localized {
            let locale = locale.trim().to_owned();
            if locale.is_empty() {
                bail!(
                    "localized lyric catalog `{}` cue `{cue_id}` has empty locale key",
                    path.display()
                );
            }
            if text.text.trim().is_empty() {
                bail!(
                    "localized lyric catalog `{}` cue `{cue_id}` locale `{locale}` has empty text",
                    path.display()
                );
            }
            for (i, seg) in text.display_segments.iter().enumerate() {
                if !(seg.end > seg.start) || !seg.start.is_finite() || !seg.end.is_finite() {
                    bail!(
                        "localized lyric catalog `{}` cue `{cue_id}` locale `{locale}` segment {} needs finite end > start",
                        path.display(),
                        i + 1
                    );
                }
            }
            localized.insert(
                locale,
                LocalizedLyricText {
                    text: text.text,
                    highlight: text.highlight,
                    display_segments: text.display_segments,
                },
            );
        }
        cues.push(LocalizedLyricCue { cue_id, localized });
    }
    Ok(LocalizedLyricCatalog {
        version: file.version,
        source_locale: file.source_locale.trim().to_owned(),
        cues,
    })
}

impl LocalizedLyricCatalog {
    /// Display text for `cue_id` at `locale`, if present.
    pub fn text_for(&self, cue_id: &str, locale: &str) -> Option<&LocalizedLyricText> {
        self.cues
            .iter()
            .find(|c| c.cue_id == cue_id)
            .and_then(|c| c.localized.get(locale))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn loads_catalog_and_resolves_locale() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("muse-lyric-loc-{stamp}.json"));
        fs::write(
            &path,
            r#"{
  "version": 1,
  "source_locale": "ja",
  "cues": [
    {
      "cue_id": "C09",
      "localized": {
        "ja": { "text": "知らない知らない", "highlight": "word" },
        "zh-Hans": { "text": "不知道不知道", "highlight": "line" },
        "en": { "text": "I don't know, I don't know", "highlight": "line" }
      }
    }
  ]
}"#,
        )
        .unwrap();
        let catalog = load_localized_lyric_catalog(&path).unwrap();
        assert_eq!(catalog.source_locale, "ja");
        assert_eq!(
            catalog.text_for("C09", "zh-Hans").map(|t| t.text.as_str()),
            Some("不知道不知道")
        );
        assert_eq!(
            catalog.text_for("C09", "en").map(|t| t.highlight),
            Some(LyricHighlightPolicy::Line)
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_duplicate_cue_id() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("muse-lyric-loc-dup-{stamp}.json"));
        fs::write(
            &path,
            r#"{
  "version": 1,
  "source_locale": "ja",
  "cues": [
    { "cue_id": "C1", "localized": { "ja": { "text": "a" } } },
    { "cue_id": "C1", "localized": { "ja": { "text": "b" } } }
  ]
}"#,
        )
        .unwrap();
        let err = load_localized_lyric_catalog(&path).unwrap_err().to_string();
        assert!(err.contains("duplicate cue_id"), "{err}");
        let _ = fs::remove_file(path);
    }
}
