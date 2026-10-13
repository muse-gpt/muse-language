//! Semantic validation and render-plan construction for Muse scripts.
use muse_parser::{
    BlockContent, BlockKind, Declaration, ShotDecl, VisualDecl, parse, parse_time_point,
    parse_time_range,
};
use muse_types::{Diagnostic, RenderSettings, Span};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::{Path, PathBuf}};

mod lyric_timing;
mod schema;
mod time_map;
pub use lyric_timing::load_timing_json;
pub use schema::{
    CompilationDocument, IrAct, IrAsset, IrOrchestrationRule, IrShot, compilation_schema,
};
pub use time_map::TimeMap;

/// Optional compile inputs beyond render settings (doc 14 `lyric from`, asset resolution).
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Project / asset root used to resolve relative `from` / asset URIs.
    pub asset_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Act {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
/// Incoming transition at a shot boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotTransition {
    Cut,
    Xfade,
    Flash,
    Iris,
    Slices,
}

impl ShotTransition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cut => "cut",
            Self::Xfade => "xfade",
            Self::Flash => "flash",
            Self::Iris => "iris",
            Self::Slices => "slices",
        }
    }
}

/// Time remap applied while a shot is active (references another shot id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotRemapMode {
    Freeze,
    Reverse,
    Melt,
}

impl ShotRemapMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Freeze => "freeze",
            Self::Reverse => "reverse",
            Self::Melt => "melt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShotRemap {
    pub mode: ShotRemapMode,
    pub source: String,
}

/// Timed cut in the render plan (`shot id : range scene name [xfade …] [remap source]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shot {
    pub id: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub scene: String,
    pub target: Option<String>,
    pub transition: ShotTransition,
    /// Transition length in bars (incoming from the previous shot; schema/compat).
    pub transition_bars: f32,
    /// Transition length in seconds (authoritative for blend windows).
    pub transition_seconds: f32,
    /// Optional freeze/reverse/melt against an existing shot id.
    pub remap: Option<ShotRemap>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub uri: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationRule {
    pub target: String,
    pub mappings: Vec<String>,
    pub hard_constraints: Vec<String>,
    pub soft_preferences: Vec<String>,
    pub style_references: Vec<String>,
    pub transition_strategy: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualLayer {
    pub target: Option<String>,
    /// Optional scene tag from `visual for act scene name { ... }`.
    pub scene: Option<String>,
    pub background_color: Option<String>,
    /// Image URIs declared as `image "..."` inside the visual block (project-relative).
    pub images: Vec<String>,
    /// Optional full-frame vignette strength in `[0, 1]`.
    pub vignette: Option<f32>,
    /// Optional film grain strength in `[0, 1]`.
    pub grain: Option<f32>,
    /// Optional soft bloom strength in `[0, 1]`.
    pub bloom: Option<f32>,
    /// Built-in palette id (`P2`) or project-relative LUT PNG path.
    pub lut: Option<String>,
    /// LUT mix amount in `[0, 1]` (ignored when `lut` is absent).
    pub lut_mix: Option<f32>,
    /// Procedural center pulse FX strength in `[0, 1]` (`fx pulse`).
    pub pulse: Option<f32>,
    /// Procedural stamp/slam FX strength in `[0, 1]` (`fx stamp`).
    pub stamp: Option<f32>,
    /// Optional stamp glyph label (`fx stamp "字"`); rasterized via system font in muse-media.
    pub stamp_label: Option<String>,
}

/// Role of a bound audio track (doc 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioRole {
    Song,
    Guide,
    Sfx,
    Stem,
}

impl AudioRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Guide => "guide",
            Self::Sfx => "sfx",
            Self::Stem => "stem",
        }
    }
}

/// How preview/render should obtain PCM when no external mux override is supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceKind {
    /// One or more `audio { track … }` bindings.
    Tracks,
    /// Development placeholder tone (no tracks declared).
    PlaceholderTone,
}

/// Bound audio track after compile (paths project-relative; PCM resolved at render).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    pub id: String,
    pub role: AudioRole,
    pub uri: String,
    pub file_start: f64,
    pub file_end: Option<f64>,
    pub timeline_offset: f64,
    pub gain_linear: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    pub tail_silence: f64,
}

/// Frozen lyric track (doc 14). Independent from `fx stamp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricTrack {
    pub id: String,
    pub act: Option<String>,
    pub lang: String,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    pub id: String,
    /// Stable semantic cue id (docs 18/20). Optional; localization keys prefer this over `id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue_id: Option<String>,
    pub words: Vec<LyricWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricWord {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricLayerRole {
    Foreground,
    Background,
    BurnIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricAlign {
    Start,
    Center,
    End,
}

/// Appearance binding for a [`LyricTrack`] (doc 14 §5). Not `fx stamp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLayer {
    pub id: String,
    pub track: String,
    pub role: LyricLayerRole,
    /// Pixel size; `0` means compose picks from frame height.
    pub size_px: f64,
    pub color: [u8; 3],
    pub highlight_color: [u8; 3],
    pub align: LyricAlign,
    /// Uniform safe margin as a fraction of the shorter frame edge.
    pub safe_margin: f64,
    /// `None` / `"none"` → draw after bloom (default).
    pub post_group: Option<String>,
}

impl LyricTrack {
    /// Active word at wall-clock seconds (`[start, end)`).
    pub fn active_word_at(&self, t: f64) -> Option<&LyricWord> {
        let t = t.max(0.0);
        for line in &self.lines {
            if let Some(word) = line
                .words
                .iter()
                .find(|w| w.start <= t && t < w.end)
            {
                return Some(word);
            }
        }
        None
    }

    /// Active line at wall-clock seconds (first line whose word range covers `t`).
    pub fn active_line_at(&self, t: f64) -> Option<&LyricLine> {
        let t = t.max(0.0);
        self.lines.iter().find(|line| {
            line.words
                .first()
                .zip(line.words.last())
                .is_some_and(|(a, b)| a.start <= t && t < b.end)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPlan {
    pub settings: RenderSettings,
    pub duration_seconds: f64,
    /// Compile-time clock used for bars/beats ↔ seconds (constant tempo first slice).
    pub time_map: TimeMap,
    pub acts: Vec<Act>,
    pub shots: Vec<Shot>,
    pub assets: Vec<Asset>,
    pub audio_source: AudioSourceKind,
    pub audio_tracks: Vec<AudioTrack>,
    pub lyric_tracks: Vec<LyricTrack>,
    pub lyric_layers: Vec<LyricLayer>,
    pub visual_layers: usize,
    pub visuals: Vec<VisualLayer>,
    pub transitions: usize,
    pub intents: Vec<String>,
    pub orchestration: Vec<OrchestrationRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compilation {
    pub plan: Option<RenderPlan>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    pub fn is_success(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| matches!(d.severity, muse_types::Severity::Error))
    }
}

pub fn compile(source: &str, settings: RenderSettings) -> Compilation {
    compile_with(source, settings, CompileOptions::default())
}

pub fn compile_with(source: &str, settings: RenderSettings, options: CompileOptions) -> Compilation {
    let parsed = parse(source);
    let mut diagnostics = parsed.diagnostics;
    let mut acts = Vec::new();
    let mut shots = Vec::new();
    let mut shot_ids = BTreeSet::new();
    let mut assets = Vec::new();
    let mut names = BTreeSet::new();
    let mut visuals = 0;
    let mut visual_layers = Vec::new();
    let mut transitions = 0;
    let mut intents = Vec::new();
    let mut orchestration = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut audio_ids = BTreeSet::new();
    let mut lyric_tracks = Vec::new();
    let mut lyric_layers = Vec::new();
    let mut lyric_ids = BTreeSet::new();
    let mut lyric_layer_ids = BTreeSet::new();

    // Pre-scan tempo/meter so timeline conversion sees the full-file clock (ADR 0006 / doc 13).
    let mut bpm = 120.0;
    let mut beats_per_bar = 4u32;
    for declaration in &parsed.script.declarations {
        match declaration {
            Declaration::Block(block) => match block.kind {
                BlockKind::Tempo => {
                    if let BlockContent::Tempo { events } = &block.content {
                        for event in events {
                            if event.bpm > 0.0 {
                                bpm = event.bpm;
                            }
                        }
                    }
                }
                BlockKind::Music => {
                    if let BlockContent::Music(music) = &block.content {
                        if let Some(meter) = &music.meter {
                            if meter.beat_unit != 4 {
                                diagnostics.push(Diagnostic::error(
                                    "meter beat unit other than 4 is not supported yet",
                                    meter.span,
                                ));
                            } else {
                                beats_per_bar = u32::from(meter.beats.max(1));
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    let time_map = TimeMap::new(bpm, beats_per_bar);

    for declaration in parsed.script.declarations {
        match declaration {
            Declaration::Asset { name, uri, span } => {
                if !names.insert(name.clone()) {
                    diagnostics.push(Diagnostic::error(format!("duplicate asset `{name}`"), span));
                }
                assets.push(Asset { name, uri });
            }
            Declaration::Block(block) => match block.kind {
                BlockKind::Intent => {
                    if let BlockContent::Intent { emotions } = &block.content {
                        intents.extend(emotions.iter().map(|emotion| emotion.emotion.clone()));
                    }
                }
                BlockKind::Timeline => {
                    if let BlockContent::Timeline {
                        acts: declarations,
                        shots: shot_decls,
                    } = &block.content
                    {
                        for act in declarations {
                            if act.start.value < act.end.value && act.start.unit == act.end.unit {
                                let start_seconds = time_map.point_to_seconds(act.start);
                                let end_seconds = time_map.point_to_seconds(act.end);
                                acts.push(Act {
                                    name: act.name.clone(),
                                    start_bar: bars_u32(time_map.point_to_bars(act.start)),
                                    end_bar: bars_u32(time_map.point_to_bars(act.end)),
                                    start_seconds,
                                    end_seconds,
                                });
                            } else {
                                diagnostics.push(Diagnostic::error(
                                    "act range must be increasing and use one time unit",
                                    act.span,
                                ));
                            }
                        }
                        for shot in shot_decls {
                            if shot.start.value >= shot.end.value || shot.start.unit != shot.end.unit
                            {
                                diagnostics.push(Diagnostic::error(
                                    "shot range must be increasing and use one time unit",
                                    shot.span,
                                ));
                                continue;
                            }
                            if !shot_ids.insert(shot.id.clone()) {
                                diagnostics.push(Diagnostic::error(
                                    format!("duplicate shot id `{}`", shot.id),
                                    shot.span,
                                ));
                                continue;
                            }
                            if let Some(target) = &shot.target
                                && !acts.iter().any(|act| act.name == *target)
                            {
                                diagnostics.push(Diagnostic::error(
                                    format!("unknown shot target `{target}`"),
                                    shot.span,
                                ));
                            }
                            let transition = match shot.transition.trim().to_ascii_lowercase().as_str()
                            {
                                "cut" | "" => ShotTransition::Cut,
                                "xfade" => ShotTransition::Xfade,
                                "flash" => ShotTransition::Flash,
                                "iris" => ShotTransition::Iris,
                                "slices" => ShotTransition::Slices,
                                other => {
                                    diagnostics.push(Diagnostic::error(
                                        format!(
                                            "unknown shot transition `{other}` (cut|xfade|flash|iris|slices)"
                                        ),
                                        shot.span,
                                    ));
                                    ShotTransition::Cut
                                }
                            };
                            let (transition_bars, transition_seconds) =
                                match parse_time_point(shot.transition_len.trim()) {
                                    Some(point) if point.value >= 0.0 => {
                                        let secs = time_map.point_to_seconds(point) as f32;
                                        let bars = time_map.point_to_bars(point) as f32;
                                        (bars, secs)
                                    }
                                    Some(_) => {
                                        diagnostics.push(Diagnostic::error(
                                            "transition length must be non-negative",
                                            shot.span,
                                        ));
                                        (0.0, 0.0)
                                    }
                                    None => {
                                        diagnostics.push(Diagnostic::error(
                                            "transition length must be a time literal (e.g. 0.5, 0.5bar, 2b, 0.25s)",
                                            shot.span,
                                        ));
                                        (0.0, 0.0)
                                    }
                                };
                            let remap = match (
                                shot.remap_mode.as_deref().map(str::trim),
                                shot.remap_source.as_deref().map(str::trim),
                            ) {
                                (None, None) => None,
                                (Some(mode), Some(source)) if !source.is_empty() => {
                                    let mode = match mode.to_ascii_lowercase().as_str() {
                                        "freeze" => Some(ShotRemapMode::Freeze),
                                        "reverse" => Some(ShotRemapMode::Reverse),
                                        "melt" => Some(ShotRemapMode::Melt),
                                        other => {
                                            diagnostics.push(Diagnostic::error(
                                                format!(
                                                    "unknown shot remap `{other}` (freeze|reverse|melt)"
                                                ),
                                                shot.span,
                                            ));
                                            None
                                        }
                                    };
                                    mode.map(|mode| ShotRemap {
                                        mode,
                                        source: source.to_owned(),
                                    })
                                }
                                _ => {
                                    diagnostics.push(Diagnostic::error(
                                        "shot remap requires `freeze|reverse|melt <shot_id>`",
                                        shot.span,
                                    ));
                                    None
                                }
                            };
                            let start_seconds = time_map.point_to_seconds(shot.start);
                            let end_seconds = time_map.point_to_seconds(shot.end);
                            shots.push(Shot {
                                id: shot.id.clone(),
                                start_bar: bars_u32(time_map.point_to_bars(shot.start)),
                                end_bar: bars_u32(time_map.point_to_bars(shot.end)),
                                start_seconds,
                                end_seconds,
                                scene: shot.scene.clone(),
                                target: shot.target.clone(),
                                transition,
                                transition_bars,
                                transition_seconds,
                                remap,
                            });
                        }
                        validate_shot_remaps(&shots, shot_decls, &mut diagnostics);
                    }
                }
                BlockKind::Tempo => {
                    if let BlockContent::Tempo { events } = &block.content {
                        for event in events {
                            if event.bpm <= 0.0 {
                                diagnostics.push(Diagnostic::error("BPM must be positive", event.span));
                            }
                        }
                    }
                }
                BlockKind::Music => {
                    if let Some(target) = block.target
                        && !acts.iter().any(|a| a.name == target)
                    {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown music target `{target}`"),
                            block.span,
                        ));
                    }
                    if block.body.contains("avoid-parallel-fifths") && !block.body.contains("harmony") {
                        diagnostics.push(Diagnostic::error(
                            "voice-leading requirement needs a harmony block",
                            block.span,
                        ));
                    }
                }
                BlockKind::Visual => {
                    let mut background_color = None;
                    let mut images = Vec::new();
                    let mut vignette = None;
                    let mut grain = None;
                    let mut bloom = None;
                    let mut lut = None;
                    let mut lut_mix = None;
                    let mut pulse = None;
                    let mut stamp = None;
                    let mut stamp_label = None;
                    if let BlockContent::Visual { elements } = &block.content {
                        visuals += elements.len().max(1);
                        for element in elements {
                            match element {
                                VisualDecl::Background { color, .. } => {
                                    background_color = Some(color.trim().trim_matches('"').to_owned());
                                }
                                VisualDecl::Asset {
                                    kind: muse_parser::VisualAssetKind::Image,
                                    uri,
                                    ..
                                } => {
                                    images.push(uri.trim().trim_matches('"').to_owned());
                                }
                                VisualDecl::Vignette { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => vignette = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("vignette {message}"), *span)),
                                    }
                                }
                                VisualDecl::Grain { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => grain = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("grain {message}"), *span)),
                                    }
                                }
                                VisualDecl::Bloom { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => bloom = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("bloom {message}"), *span)),
                                    }
                                }
                                VisualDecl::Lut { name, mix, span } => {
                                    let name = name.trim().trim_matches('"').to_owned();
                                    if name.is_empty() {
                                        diagnostics.push(Diagnostic::error(
                                            "lut name must be non-empty",
                                            *span,
                                        ));
                                    } else {
                                        lut = Some(name);
                                    }
                                    match parse_unit_strength(mix) {
                                        Ok(value) => lut_mix = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("lut mix {message}"), *span)),
                                    }
                                }
                                VisualDecl::Fx {
                                    kind,
                                    label,
                                    strength,
                                    span,
                                } => {
                                    let kind = kind.trim().to_ascii_lowercase();
                                    match kind.as_str() {
                                        "pulse" => match parse_unit_strength(strength) {
                                            Ok(value) => pulse = Some(value),
                                            Err(message) => diagnostics.push(Diagnostic::error(
                                                format!("fx pulse {message}"),
                                                *span,
                                            )),
                                        },
                                        "stamp" => match parse_unit_strength(strength) {
                                            Ok(value) => {
                                                stamp = Some(value);
                                                if let Some(label) = label {
                                                    let label =
                                                        label.trim().trim_matches('"').to_owned();
                                                    if !label.is_empty() {
                                                        stamp_label = Some(label);
                                                    }
                                                }
                                            }
                                            Err(message) => diagnostics.push(Diagnostic::error(
                                                format!("fx stamp {message}"),
                                                *span,
                                            )),
                                        },
                                        other => diagnostics.push(Diagnostic::error(
                                            format!(
                                                "unknown fx `{other}` (supported: pulse, stamp)"
                                            ),
                                            *span,
                                        )),
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        visuals += 1;
                    }
                    if let Some(target) = &block.target
                        && !acts.iter().any(|a| a.name == *target)
                    {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown visual target `{target}`"),
                            block.span,
                        ));
                    }
                    visual_layers.push(VisualLayer {
                        target: block.target.clone(),
                        scene: block.scene.clone(),
                        background_color,
                        images,
                        vignette,
                        grain,
                        bloom,
                        lut,
                        lut_mix,
                        pulse,
                        stamp,
                        stamp_label,
                    });
                }
                BlockKind::Audio => {
                    if let Some(target) = &block.target
                        && !acts.iter().any(|a| a.name == *target)
                    {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown audio target `{target}`"),
                            block.span,
                        ));
                    }
                    let BlockContent::Audio { tracks } = &block.content else {
                        continue;
                    };
                    for track in tracks {
                        if track.from.trim().is_empty() {
                            diagnostics.push(Diagnostic::error(
                                format!("audio track `{}` requires `from`", track.id),
                                track.span,
                            ));
                            continue;
                        }
                        if !audio_ids.insert(track.id.clone()) {
                            diagnostics.push(Diagnostic::error(
                                format!("duplicate audio track id `{}`", track.id),
                                track.span,
                            ));
                            continue;
                        }
                        if track.gain_db.is_some() && track.amplitude.is_some() {
                            diagnostics.push(Diagnostic::error(
                                format!(
                                    "audio track `{}` cannot set both `gain` and `amplitude`",
                                    track.id
                                ),
                                track.span,
                            ));
                            continue;
                        }
                        let Some(uri) = resolve_audio_uri(&track.from, &assets) else {
                            diagnostics.push(Diagnostic::error(
                                format!(
                                    "unknown audio source `{}` on track `{}` (asset name or path)",
                                    track.from, track.id
                                ),
                                track.span,
                            ));
                            continue;
                        };
                        let role = match track.role.trim().to_ascii_lowercase().as_str() {
                            "song" | "" => AudioRole::Song,
                            "guide" => AudioRole::Guide,
                            "sfx" => AudioRole::Sfx,
                            "stem" => AudioRole::Stem,
                            other => {
                                diagnostics.push(Diagnostic::error(
                                    format!(
                                        "unknown audio role `{other}` on track `{}` (song|guide|sfx|stem)",
                                        track.id
                                    ),
                                    track.span,
                                ));
                                AudioRole::Song
                            }
                        };
                        let timeline_offset = parse_optional_seconds(
                            track.offset.as_deref(),
                            &time_map,
                            0.0,
                            &mut diagnostics,
                            track.span,
                            "offset",
                        );
                        let (file_start, file_end) = match track.clip.as_deref() {
                            None => (0.0, None),
                            Some(raw) => match parse_time_range(raw) {
                                Some((start, end)) if start.value < end.value => (
                                    time_map.point_to_seconds(start),
                                    Some(time_map.point_to_seconds(end)),
                                ),
                                _ => {
                                    diagnostics.push(Diagnostic::error(
                                        format!(
                                            "audio track `{}` clip must be an increasing range (e.g. 0s..30s)",
                                            track.id
                                        ),
                                        track.span,
                                    ));
                                    (0.0, None)
                                }
                            },
                        };
                        let gain_linear = if let Some(db) = track.gain_db {
                            10f64.powf(db / 20.0)
                        } else if let Some(amp) = track.amplitude {
                            if amp < 0.0 {
                                diagnostics.push(Diagnostic::error(
                                    format!(
                                        "audio track `{}` amplitude must be non-negative",
                                        track.id
                                    ),
                                    track.span,
                                ));
                                1.0
                            } else {
                                amp
                            }
                        } else {
                            1.0
                        };
                        let fade_in = parse_optional_seconds(
                            track.fade_in.as_deref(),
                            &time_map,
                            0.0,
                            &mut diagnostics,
                            track.span,
                            "fade_in",
                        );
                        let fade_out = parse_optional_seconds(
                            track.fade_out.as_deref(),
                            &time_map,
                            0.0,
                            &mut diagnostics,
                            track.span,
                            "fade_out",
                        );
                        let tail_silence = parse_optional_seconds(
                            track.tail_silence.as_deref(),
                            &time_map,
                            0.0,
                            &mut diagnostics,
                            track.span,
                            "tail_silence",
                        );
                        audio_tracks.push(AudioTrack {
                            id: track.id.clone(),
                            role,
                            uri,
                            file_start,
                            file_end,
                            timeline_offset,
                            gain_linear,
                            fade_in,
                            fade_out,
                            tail_silence,
                        });
                    }
                }
                BlockKind::Lyric => {
                    let BlockContent::Lyric(declaration) = &block.content else {
                        continue;
                    };
                    let track_id = block
                        .target
                        .clone()
                        .unwrap_or_else(|| format!("lyric_{}", lyric_tracks.len() + 1));
                    if !lyric_ids.insert(track_id.clone()) {
                        diagnostics.push(Diagnostic::error(
                            format!("duplicate lyric track id `{track_id}`"),
                            block.span,
                        ));
                        continue;
                    }
                    if let Some(act) = &block.target {
                        if !acts.iter().any(|a| a.name == *act) {
                            diagnostics.push(Diagnostic::error(
                                format!("unknown lyric target act `{act}`"),
                                block.span,
                            ));
                        }
                    }
                    if declaration.from.is_some() && !declaration.lines.is_empty() {
                        diagnostics.push(Diagnostic::error(
                            format!(
                                "lyric track `{track_id}` cannot mix `from` with inline lines; choose one"
                            ),
                            block.span,
                        ));
                        continue;
                    }
                    let (file_lang, mut lines) = if let Some(from) = declaration.from.as_deref() {
                        match resolve_and_load_lyric_timing(
                            from,
                            &assets,
                            options.asset_root.as_deref(),
                        ) {
                            Ok(loaded) => loaded,
                            Err(err) => {
                                diagnostics.push(Diagnostic::error(err, block.span));
                                continue;
                            }
                        }
                    } else {
                        (None, Vec::new())
                    };
                    if declaration.from.is_none() {
                        for (line_idx, line_decl) in declaration.lines.iter().enumerate() {
                            let line_id = format!("L{}", line_idx + 1);
                            let mut words = Vec::new();
                            for (word_idx, word_decl) in line_decl.words.iter().enumerate() {
                                let Some((start_pt, end_pt)) = parse_time_range(&word_decl.range)
                                else {
                                    diagnostics.push(Diagnostic::error(
                                        format!(
                                            "lyric word `{}` needs an increasing time range (e.g. 0s..0.4s)",
                                            word_decl.text
                                        ),
                                        word_decl.span,
                                    ));
                                    continue;
                                };
                                let start = time_map.point_to_seconds(start_pt);
                                let end = time_map.point_to_seconds(end_pt);
                                if end <= start {
                                    diagnostics.push(Diagnostic::error(
                                        format!(
                                            "lyric word `{}` end must be after start",
                                            word_decl.text
                                        ),
                                        word_decl.span,
                                    ));
                                    continue;
                                }
                                words.push(LyricWord {
                                    id: format!("W{}", word_idx + 1),
                                    text: word_decl.text.clone(),
                                    start,
                                    end,
                                });
                            }
                            if words.is_empty() {
                                diagnostics.push(Diagnostic::error(
                                    format!("lyric line `{line_id}` has no words"),
                                    line_decl.span,
                                ));
                                continue;
                            }
                            lines.push(LyricLine {
                                id: line_id,
                                cue_id: None,
                                words,
                            });
                        }
                    }
                    if lines.is_empty() {
                        diagnostics.push(Diagnostic::error(
                            format!("lyric track `{track_id}` has no lines"),
                            block.span,
                        ));
                        continue;
                    }
                    if declaration.layers.is_empty() {
                        let layer_id = format!("{track_id}_layer");
                        lyric_layer_ids.insert(layer_id.clone());
                        lyric_layers.push(default_lyric_layer(layer_id, track_id.clone()));
                    } else {
                        for (layer_idx, layer_decl) in declaration.layers.iter().enumerate() {
                            let layer_id = format!("{track_id}_layer{}", layer_idx + 1);
                            if !lyric_layer_ids.insert(layer_id.clone()) {
                                diagnostics.push(Diagnostic::error(
                                    format!("duplicate lyric layer id `{layer_id}`"),
                                    layer_decl.span,
                                ));
                                continue;
                            }
                            match lower_lyric_layer(
                                layer_id,
                                track_id.clone(),
                                layer_decl,
                                &mut diagnostics,
                            ) {
                                Some(layer) => lyric_layers.push(layer),
                                None => continue,
                            }
                        }
                    }
                    lyric_tracks.push(LyricTrack {
                        id: track_id,
                        act: block.target.clone(),
                        lang: declaration
                            .lang
                            .clone()
                            .or(file_lang)
                            .unwrap_or_else(|| "und".to_owned()),
                        lines,
                    });
                }
                BlockKind::Orchestrate => {
                    let target = block.target.unwrap_or_else(|| "entire_song".to_owned());
                    if target != "entire_song" && !acts.iter().any(|act| act.name == target) {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown orchestration target `{target}`"),
                            block.span,
                        ));
                    }
                    let BlockContent::Orchestrate(declaration) = &block.content else {
                        continue;
                    };
                    let hard_constraints = declaration.hard_constraints.clone();
                    let soft_preferences = declaration.soft_preferences.clone();
                    let style_references = declaration.style_references.clone();
                    let transition_strategy = declaration.transition_strategy.clone();
                    transitions += usize::from(transition_strategy.is_some());
                    orchestration.push(OrchestrationRule {
                        target,
                        mappings: declaration
                            .mappings
                            .iter()
                            .map(|mapping| mapping.intent.clone())
                            .collect(),
                        hard_constraints,
                        soft_preferences,
                        style_references,
                        transition_strategy,
                    });
                }
                BlockKind::Directing => {
                    transitions += block.body.matches("transition at").count();
                }
            },
            _ => {}
        }
    }
    if acts.is_empty() {
        diagnostics.push(Diagnostic::error(
            "a timeline with at least one act is required",
            Span::new(0, 0, 1, 1),
        ));
    }
    let mut duration = acts
        .iter()
        .map(|a| a.end_seconds)
        .chain(shots.iter().map(|s| s.end_seconds))
        .fold(0.0_f64, f64::max);
    for track in &audio_tracks {
        if let Some(file_end) = track.file_end {
            let playable = (file_end - track.file_start).max(0.0);
            duration = duration.max(track.timeline_offset + playable + track.tail_silence);
        }
    }
    for track in &lyric_tracks {
        for line in &track.lines {
            for word in &line.words {
                duration = duration.max(word.end);
            }
        }
    }
    let audio_source = if audio_tracks.is_empty() {
        AudioSourceKind::PlaceholderTone
    } else {
        AudioSourceKind::Tracks
    };
    let plan = (!diagnostics
        .iter()
        .any(|d| matches!(d.severity, muse_types::Severity::Error)))
    .then_some(RenderPlan {
        settings,
        duration_seconds: duration,
        time_map,
        acts,
        shots,
        assets,
        audio_source,
        audio_tracks,
        lyric_tracks,
        lyric_layers,
        visual_layers: visuals,
        visuals: visual_layers,
        transitions,
        intents,
        orchestration,
    });
    Compilation { plan, diagnostics }
}

fn default_lyric_layer(id: String, track: String) -> LyricLayer {
    LyricLayer {
        id,
        track,
        role: LyricLayerRole::Foreground,
        size_px: 0.0,
        color: [240, 240, 245],
        highlight_color: [255, 224, 138],
        align: LyricAlign::Center,
        safe_margin: 0.08,
        post_group: Some("none".into()),
    }
}

fn lower_lyric_layer(
    id: String,
    track: String,
    decl: &muse_parser::LyricLayerDecl,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<LyricLayer> {
    let role = match decl
        .role
        .as_deref()
        .unwrap_or("foreground")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "foreground" | "fg" => LyricLayerRole::Foreground,
        "background" | "bg" => LyricLayerRole::Background,
        "burn_in" | "burnin" => LyricLayerRole::BurnIn,
        other => {
            diagnostics.push(Diagnostic::error(
                format!("unknown lyric layer role `{other}` (foreground|background|burn_in)"),
                decl.span,
            ));
            return None;
        }
    };
    let align = match decl
        .align
        .as_deref()
        .unwrap_or("center")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "start" | "left" => LyricAlign::Start,
        "center" | "middle" => LyricAlign::Center,
        "end" | "right" => LyricAlign::End,
        other => {
            diagnostics.push(Diagnostic::error(
                format!("unknown lyric align `{other}` (start|center|end)"),
                decl.span,
            ));
            return None;
        }
    };
    let color = match decl.color.as_deref() {
        Some(raw) => match parse_hex_rgb(raw) {
            Some(rgb) => rgb,
            None => {
                diagnostics.push(Diagnostic::error(
                    format!("invalid lyric color `{raw}` (expected #RRGGBB)"),
                    decl.span,
                ));
                return None;
            }
        },
        None => [240, 240, 245],
    };
    let highlight_color = match decl.highlight_color.as_deref() {
        Some(raw) => match parse_hex_rgb(raw) {
            Some(rgb) => rgb,
            None => {
                diagnostics.push(Diagnostic::error(
                    format!("invalid lyric highlight color `{raw}` (expected #RRGGBB)"),
                    decl.span,
                ));
                return None;
            }
        },
        None => [255, 224, 138],
    };
    let size_px = decl.size.unwrap_or(0.0);
    if size_px < 0.0 {
        diagnostics.push(Diagnostic::error(
            "lyric layer size must be non-negative",
            decl.span,
        ));
        return None;
    }
    let safe_margin = decl.safe.unwrap_or(0.08).clamp(0.0, 0.45);
    Some(LyricLayer {
        id,
        track,
        role,
        size_px,
        color,
        highlight_color,
        align,
        safe_margin,
        post_group: decl.post_group.clone().or_else(|| Some("none".into())),
    })
}

fn parse_hex_rgb(raw: &str) -> Option<[u8; 3]> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([r, g, b])
}

fn resolve_audio_uri(from: &str, assets: &[Asset]) -> Option<String> {
    let from = from.trim();
    if let Some(asset) = assets.iter().find(|asset| asset.name == from) {
        return Some(asset.uri.clone());
    }
    if from.contains('/') || from.contains('\\') || from.contains('.') {
        Some(from.to_owned())
    } else {
        None
    }
}

fn resolve_and_load_lyric_timing(
    from: &str,
    assets: &[Asset],
    asset_root: Option<&Path>,
) -> Result<(Option<String>, Vec<LyricLine>), String> {
    let uri = resolve_audio_uri(from, assets).ok_or_else(|| {
        format!("unknown lyric source `{from}` (asset name or path)")
    })?;
    let path = {
        let path = Path::new(&uri);
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(root) = asset_root {
            root.join(path)
        } else {
            path.to_path_buf()
        }
    };
    if !path.is_file() {
        return Err(format!(
            "lyric timing file `{}` not found (pass compile asset_root / --project)",
            path.display()
        ));
    }
    load_timing_json(&path).map_err(|err| err.to_string())
}

fn parse_optional_seconds(
    raw: Option<&str>,
    time_map: &TimeMap,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
    field: &str,
) -> f64 {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return default;
    };
    match parse_time_point(raw) {
        Some(point) if point.value >= 0.0 => time_map.point_to_seconds(point),
        _ => {
            diagnostics.push(Diagnostic::error(
                format!("invalid audio `{field}` time literal `{raw}`"),
                span,
            ));
            default
        }
    }
}

fn bars_u32(bars: f64) -> u32 {
    if !bars.is_finite() || bars <= 0.0 {
        0
    } else if bars >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        bars.round() as u32
    }
}

fn parse_unit_strength(raw: &str) -> Result<f32, &'static str> {
    match raw.trim().parse::<f32>() {
        Ok(value) if (0.0..=1.0).contains(&value) => Ok(value),
        Ok(_) => Err("strength must be between 0 and 1"),
        Err(_) => Err("strength must be a number"),
    }
}

impl RenderPlan {
    /// Highest act/shot end bar used for normalized time mapping.
    pub fn end_bar(&self) -> u32 {
        self.acts
            .iter()
            .map(|act| act.end_bar)
            .chain(self.shots.iter().map(|shot| shot.end_bar))
            .max()
            .unwrap_or(0)
    }

    /// Active shot at wall-clock seconds (half-open `[start, end)`).
    pub fn shot_at_seconds(&self, t: f64) -> Option<&Shot> {
        if self.shots.is_empty() {
            return None;
        }
        let t = t.max(0.0);
        self.shots
            .iter()
            .find(|shot| shot.start_seconds <= t && t < shot.end_seconds)
            .or_else(|| {
                // Exact end-of-plan: keep the last covering shot.
                let duration = self.duration_seconds;
                if duration > 0.0 && (t - duration).abs() < 1e-9 {
                    self.shots
                        .iter()
                        .rev()
                        .find(|shot| shot.end_seconds <= duration + 1e-9)
                } else {
                    None
                }
            })
    }

    /// Active shot at normalized time `t` in `[0, 1]` across [`Self::duration_seconds`].
    pub fn shot_at_normalized(&self, t: f32) -> Option<&Shot> {
        let duration = self.duration_seconds;
        if duration <= 0.0 {
            return None;
        }
        let t_sec = f64::from(t.clamp(0.0, 1.0)) * duration;
        self.shot_at_seconds(t_sec)
    }

    /// Base visuals (no scene tag) plus overlays matching the active look scene
    /// (remap source scene when a time remap is set).
    pub fn visuals_at_normalized(&self, t: f32) -> Vec<&VisualLayer> {
        let mut layers: Vec<&VisualLayer> = self
            .visuals
            .iter()
            .filter(|layer| layer.scene.is_none())
            .collect();
        if let Some(shot) = self.shot_at_normalized(t) {
            let scene = self.look_scene_for_shot(shot);
            layers.extend(
                self.visuals
                    .iter()
                    .filter(|layer| layer.scene.as_deref() == Some(scene)),
            );
        }
        if layers.is_empty() {
            self.visuals.iter().collect()
        } else {
            layers
        }
    }

    /// Scene name used for visual overlays (follows remap source when present).
    pub fn look_scene_for_shot<'a>(&'a self, shot: &'a Shot) -> &'a str {
        if let Some(remap) = &shot.remap {
            if let Some(source) = self.shots.iter().find(|s| s.id == remap.source) {
                return source.scene.as_str();
            }
        }
        shot.scene.as_str()
    }

    /// Progress within the active shot in `[0, 1]`, or global `t` when no shot.
    /// Applies freeze / reverse / melt when the shot declares a remap.
    pub fn shot_progress_normalized(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let Some(shot) = self.shot_at_normalized(t) else {
            return t;
        };
        let span = shot.end_seconds - shot.start_seconds;
        if span <= 0.0 {
            return 0.0;
        }
        let t_sec = f64::from(t) * self.duration_seconds.max(0.0);
        let linear = (((t_sec - shot.start_seconds) / span) as f32).clamp(0.0, 1.0);
        match shot.remap.as_ref().map(|r| r.mode) {
            None => linear,
            Some(ShotRemapMode::Freeze) => 1.0,
            Some(ShotRemapMode::Reverse) => 1.0 - linear,
            Some(ShotRemapMode::Melt) => 1.0 - (1.0 - linear).powi(3),
        }
    }

    /// Absolute bar position for normalized `t`.
    pub fn bar_at_normalized(&self, t: f32) -> f32 {
        let end = self.end_bar();
        if end == 0 {
            return 0.0;
        }
        (t.clamp(0.0, 1.0) as f64 * f64::from(end)) as f32
    }

    /// Normalized plan time for absolute frame index using wall-clock `index / fps`.
    ///
    /// Truncated renders must use this (not `index / (output_frames - 1)`) so a short
    /// preview covers the start of the plan rather than resampling the whole timeline.
    pub fn normalized_time_at_frame(&self, frame_index: usize) -> f32 {
        let fps = self.settings.frame_rate.max(1) as f64;
        let duration = self.duration_seconds.max(1.0 / fps);
        ((frame_index as f64 / fps) / duration).clamp(0.0, 1.0) as f32
    }

    /// Incoming transition state at `t`, if a blend window (xfade/flash/iris/slices) is active.
    pub fn transition_at_normalized(&self, t: f32) -> Option<TransitionState> {
        let shot = self.shot_at_normalized(t)?;
        if matches!(shot.transition, ShotTransition::Cut) || shot.transition_seconds <= 0.0 {
            return None;
        }
        let duration = self.duration_seconds;
        if duration <= 0.0 {
            return None;
        }
        let t_sec = f64::from(t.clamp(0.0, 1.0)) * duration;
        let start = shot.start_seconds;
        let fade_end = start + f64::from(shot.transition_seconds);
        if t_sec < start || t_sec >= fade_end {
            return None;
        }
        let mix = ((t_sec - start) / f64::from(shot.transition_seconds)).clamp(0.0, 1.0) as f32;
        let prev_t = ((start - 1e-3) / duration).clamp(0.0, 1.0) as f32;
        let flash = if matches!(shot.transition, ShotTransition::Flash) {
            // Peak at the cut, fall off across the flash window.
            (1.0 - mix).powi(2)
        } else {
            0.0
        };
        Some(TransitionState {
            kind: shot.transition,
            mix,
            from_t: prev_t,
            flash,
        })
    }
}

/// Active incoming transition sample for compositors.
#[derive(Debug, Clone, Copy)]
pub struct TransitionState {
    pub kind: ShotTransition,
    /// 0 = fully previous look, 1 = fully current shot.
    pub mix: f32,
    /// Normalized time used to sample the previous shot look.
    pub from_t: f32,
    /// Extra white flash amount in `[0, 1]` (flash transitions).
    pub flash: f32,
}

fn validate_shot_remaps(shots: &[Shot], decls: &[ShotDecl], diagnostics: &mut Vec<Diagnostic>) {
    for shot in shots {
        let Some(remap) = &shot.remap else {
            continue;
        };
        let span = decls
            .iter()
            .find(|d| d.id == shot.id)
            .map(|d| d.span)
            .unwrap_or_else(|| Span::new(0, 0, 1, 1));
        if !shots.iter().any(|s| s.id == remap.source) {
            diagnostics.push(Diagnostic::error(
                format!(
                    "unknown remap source `{}` on shot `{}`",
                    remap.source, shot.id
                ),
                span,
            ));
            continue;
        }
        // Self-remap is allowed; reject only multi-shot cycles (A→B→A).
        if remap.source == shot.id {
            continue;
        }
        let mut cursor = remap.source.as_str();
        let mut guard = 0usize;
        while guard < shots.len() {
            guard += 1;
            let Some(next) = shots
                .iter()
                .find(|s| s.id == cursor)
                .and_then(|s| s.remap.as_ref())
            else {
                break;
            };
            if next.source == shot.id {
                diagnostics.push(Diagnostic::error(
                    format!("cyclic shot remap involving `{}`", shot.id),
                    span,
                ));
                break;
            }
            cursor = next.source.as_str();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_target() {
        let c = compile("timeline { act a : 0..2 } visual for b {}", RenderSettings::default());
        assert!(!c.is_success())
    }

    #[test]
    fn emits_orchestration_rules() {
        let c = compile(
            "timeline { act verse : 0..4 } orchestrate for verse { map \"loneliness\" to {} hard_constraints: [\"no_glitch\"] }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid orchestration");
        assert_eq!(plan.orchestration[0].mappings, ["loneliness"]);
        assert_eq!(plan.orchestration[0].hard_constraints, ["no_glitch"]);
    }

    #[test]
    fn captures_visual_background_color() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals.len(), 1);
        assert_eq!(plan.visuals[0].background_color.as_deref(), Some("#0b1524"));
    }

    #[test]
    fn captures_visual_image_uris() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#000000" image "assets/textures/plate.png" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].images, ["assets/textures/plate.png"]);
    }

    #[test]
    fn captures_visual_vignette() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" vignette 0.35 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].vignette, Some(0.35));
    }

    #[test]
    fn captures_visual_grain() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" grain 0.06 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].grain, Some(0.06));
    }

    #[test]
    fn captures_visual_bloom() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" bloom 0.3 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].bloom, Some(0.3));
    }

    #[test]
    fn captures_visual_lut() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" lut P2 0.25 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].lut.as_deref(), Some("P2"));
        assert_eq!(plan.visuals[0].lut_mix, Some(0.25));
    }

    #[test]
    fn captures_fx_pulse() {
        let c = compile(
            r##"timeline { act main : 0..8 shot s01 : 0..4 scene ember } visual for main scene ember { fx pulse 0.8 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid fx");
        assert_eq!(plan.visuals[0].pulse, Some(0.8));
        assert!((plan.shot_progress_normalized(0.125) - 0.25).abs() < 0.02);
    }

    #[test]
    fn captures_fx_stamp() {
        let c = compile(
            r##"timeline { act main : 0..8 shot s01 : 0..4 scene illust } visual for main scene illust { fx stamp "残" 0.9 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid stamp");
        assert_eq!(plan.visuals[0].stamp, Some(0.9));
        assert_eq!(plan.visuals[0].stamp_label.as_deref(), Some("残"));
    }

    #[test]
    fn normalized_time_at_frame_is_wall_clock() {
        let mut settings = RenderSettings::default();
        settings.frame_rate = 24;
        let c = compile(
            "timeline { act main : 0..100 } tempo { at 0 bpm 120 }",
            settings,
        );
        let plan = c.plan.expect("valid plan");
        // 24 frames = 1s; must not jump to end of plan.
        let t = plan.normalized_time_at_frame(24);
        assert!(t < 0.05, "expected early plan time, got {t}");
        assert!((plan.normalized_time_at_frame(0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn captures_timeline_shots() {
        let c = compile(
            "timeline { act main : 0..16 shot s01 : 0..4 scene ember shot s02 : 4..8 scene illust for main xfade 0.5 }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid shots");
        assert_eq!(plan.shots.len(), 2);
        assert_eq!(plan.shots[0].id, "s01");
        assert_eq!(plan.shots[0].scene, "ember");
        assert_eq!((plan.shots[0].start_bar, plan.shots[0].end_bar), (0, 4));
        assert_eq!(plan.shots[0].transition, ShotTransition::Cut);
        assert_eq!(plan.shots[1].target.as_deref(), Some("main"));
        assert_eq!(plan.shots[1].transition, ShotTransition::Xfade);
        assert_eq!(plan.shots[1].transition_bars, 0.5);
        let edge = 4.1 / 16.0;
        let tr = plan.transition_at_normalized(edge as f32).expect("xfade");
        assert!(tr.mix > 0.0 && tr.mix < 1.0);
    }

    #[test]
    fn bare_bars_duration_matches_classic_formula() {
        let c = compile(
            "timeline { act main : 0..226 } tempo { at 0 bpm 130.5 }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid");
        let expected = 226.0 * 4.0 * 60.0 / 130.5;
        assert!(
            (plan.duration_seconds - expected).abs() < 1e-6,
            "got {} want {expected}",
            plan.duration_seconds
        );
        assert_eq!(plan.time_map.beats_per_bar, 4);
    }

    #[test]
    fn seconds_suffix_sets_wall_clock_duration() {
        let c = compile(
            "timeline { act main : 0s..30s shot c01 : 0s..8s scene paper } tempo { at 0 bpm 120 }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid seconds timeline");
        assert!((plan.duration_seconds - 30.0).abs() < 1e-9);
        assert!((plan.shots[0].end_seconds - 8.0).abs() < 1e-9);
        assert!(plan.shot_at_seconds(4.0).map(|s| s.id.as_str()) == Some("c01"));
    }

    #[test]
    fn compiles_inline_lyric_track() {
        let c = compile(
            r#"timeline { act main : 0s..3s }
tempo { bpm 120 }
lyric for main {
  lang "zh-Hans"
  line {
    word "残" 0s..0.4s
    word "响" 0.4s..0.8s
  }
}"#,
            RenderSettings::default(),
        );
        assert!(c.is_success(), "{:?}", c.diagnostics);
        let plan = c.plan.expect("lyric plan");
        assert_eq!(plan.lyric_tracks.len(), 1);
        let track = &plan.lyric_tracks[0];
        assert_eq!(track.id, "main");
        assert_eq!(track.lang, "zh-Hans");
        assert_eq!(track.lines[0].words[0].text, "残");
        assert!((track.lines[0].words[0].start - 0.0).abs() < 1e-9);
        assert!((track.lines[0].words[0].end - 0.4).abs() < 1e-9);
        assert!((track.lines[0].words[1].start - 0.4).abs() < 1e-9);
        assert_eq!(plan.lyric_layers.len(), 1);
        assert_eq!(plan.lyric_layers[0].track, "main");
        assert_eq!(
            track.active_word_at(0.5).map(|w| w.text.as_str()),
            Some("响")
        );
    }

    #[test]
    fn compiles_lyric_layer_appearance() {
        let c = compile(
            r##"timeline { act main : 0s..2s }
tempo { bpm 120 }
lyric for main {
  lang "zh-Hans"
  line { word "亮" 0s..1s word "起" 1s..2s }
  layer {
    role foreground
    size 42
    align center
    highlight color "#ffe08a"
    safe 0.1
    post_group none
  }
}"##,
            RenderSettings::default(),
        );
        assert!(c.is_success(), "{:?}", c.diagnostics);
        let plan = c.plan.expect("layer plan");
        assert_eq!(plan.lyric_layers.len(), 1);
        let layer = &plan.lyric_layers[0];
        assert!((layer.size_px - 42.0).abs() < 1e-9);
        assert_eq!(layer.highlight_color, [255, 224, 138]);
        assert!((layer.safe_margin - 0.1).abs() < 1e-9);
    }

    #[test]
    fn compiles_lyric_from_timing_json() {
        use std::{env, fs, time::{SystemTime, UNIX_EPOCH}};
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-lyric-from-{stamp}"));
        fs::create_dir_all(root.join("assets/lyrics")).unwrap();
        fs::write(
            root.join("assets/lyrics/chorus.timing.json"),
            r#"{"version":1,"lang":"zh-Hans","time_unit":"seconds","lines":[{"words":[{"text":"花","start":1.0,"end":1.5},{"text":"园","start":1.5,"end":2.0}]}]}"#,
        )
        .unwrap();
        let source = r#"timeline { act main : 0s..3s }
tempo { bpm 120 }
asset chorus_lyric = "assets/lyrics/chorus.timing.json";
lyric for main { from chorus_lyric }
"#;
        let c = compile_with(
            source,
            RenderSettings::default(),
            CompileOptions {
                asset_root: Some(root.clone()),
            },
        );
        assert!(c.is_success(), "{:?}", c.diagnostics);
        let plan = c.plan.expect("from lyric");
        assert_eq!(plan.lyric_tracks[0].lines[0].words[0].text, "花");
        assert!((plan.lyric_tracks[0].lines[0].words[1].end - 2.0).abs() < 1e-9);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn meter_affects_bar_duration() {
        let c = compile(
            r#"timeline { act main : 0..8 } tempo { bpm 120 } music for main { meter 3/4 motif "C4" }"#,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid 3/4");
        assert_eq!(plan.time_map.beats_per_bar, 3);
        // 8 bars × 3 beats × 0.5s = 12s
        assert!((plan.duration_seconds - 12.0).abs() < 1e-9);
    }

    #[test]
    fn explicit_bar_suffix_matches_bare() {
        let bare = compile(
            "timeline { act main : 0..8 } tempo { bpm 120 }",
            RenderSettings::default(),
        )
        .plan
        .expect("bare");
        let explicit = compile(
            "timeline { act main : 0bar..8bar } tempo { bpm 120 }",
            RenderSettings::default(),
        )
        .plan
        .expect("suffix");
        assert!((bare.duration_seconds - explicit.duration_seconds).abs() < 1e-9);
    }

    #[test]
    fn captures_iris_transition() {
        let c = compile(
            "timeline { act main : 0..16 shot s01 : 0..8 scene a shot s02 : 8..16 scene b iris 0.75 }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid iris");
        assert_eq!(plan.shots[1].transition, ShotTransition::Iris);
        assert_eq!(plan.shots[1].transition_bars, 0.75);
        let edge = 8.2 / 16.0;
        let tr = plan.transition_at_normalized(edge as f32).expect("iris");
        assert_eq!(tr.kind, ShotTransition::Iris);
        assert!(tr.mix > 0.0 && tr.mix < 1.0);
        assert_eq!(tr.flash, 0.0);
    }

    #[test]
    fn captures_slices_transition() {
        let c = compile(
            "timeline { act main : 0..12 shot s01 : 0..4 scene a shot s02 : 4..12 scene b slices }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid slices");
        assert_eq!(plan.shots[1].transition, ShotTransition::Slices);
        assert_eq!(plan.shots[1].transition_bars, 0.5);
        let edge = 4.15 / 12.0;
        let tr = plan.transition_at_normalized(edge as f32).expect("slices");
        assert_eq!(tr.kind, ShotTransition::Slices);
        assert!(tr.mix > 0.0 && tr.mix < 1.0);
    }

    #[test]
    fn captures_shot_time_remap() {
        let c = compile(
            r##"timeline { act main : 0..24 shot s01 : 0..8 scene ember shot s02 : 8..16 scene illust reverse s01 shot s03 : 16..24 scene thread freeze s01 } visual for main scene ember { fx pulse 0.8 }"##,
            RenderSettings::default(),
        );
        assert!(c.is_success(), "{:?}", c.diagnostics);
        let plan = c.plan.expect("valid remap");
        let remap = plan.shots[1].remap.as_ref().expect("reverse");
        assert_eq!(remap.mode, ShotRemapMode::Reverse);
        assert_eq!(remap.source, "s01");
        assert_eq!(
            plan.shots[2].remap.as_ref().map(|r| r.mode),
            Some(ShotRemapMode::Freeze)
        );
        // Mid s02 (reverse s01): progress near 0.5 → reversed ~0.5; look scene from s01.
        let mid = 12.0 / 24.0;
        assert!((plan.shot_progress_normalized(mid as f32) - 0.5).abs() < 0.05);
        assert_eq!(
            plan.look_scene_for_shot(&plan.shots[1]),
            "ember"
        );
        assert!(
            plan.visuals_at_normalized(mid as f32)
                .iter()
                .any(|v| v.pulse == Some(0.8))
        );
        // Freeze holds end progress.
        let freeze_t = 20.0 / 24.0;
        assert!((plan.shot_progress_normalized(freeze_t as f32) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn rejects_unknown_remap_source() {
        let c = compile(
            "timeline { act main : 0..8 shot s01 : 0..4 scene a shot s02 : 4..8 scene b freeze missing }",
            RenderSettings::default(),
        );
        assert!(!c.is_success());
    }

    #[test]
    fn rejects_duplicate_shot_ids() {
        let c = compile(
            "timeline { act main : 0..8 shot s01 : 0..2 scene a shot s01 : 2..4 scene b }",
            RenderSettings::default(),
        );
        assert!(!c.is_success());
    }

    #[test]
    fn captures_visual_scene_tag() {
        let c = compile(
            r##"timeline { act main : 0..8 shot s01 : 0..4 scene ember for main } visual for main scene ember { background color "#1a0c08" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual scene");
        assert_eq!(plan.visuals[0].scene.as_deref(), Some("ember"));
        assert_eq!(plan.shot_at_normalized(0.0).map(|s| s.id.as_str()), Some("s01"));
        assert_eq!(
            plan.visuals_at_normalized(0.0)
                .iter()
                .filter_map(|v| v.scene.as_deref())
                .collect::<Vec<_>>(),
            ["ember"]
        );
    }
}
