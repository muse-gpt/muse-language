use muse_types::Span;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Script {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, Serialize)]
pub enum Declaration {
    Namespace {
        name: String,
        span: Span,
    },
    Using {
        source: String,
        alias: Option<String>,
        span: Span,
    },
    Asset {
        name: String,
        uri: String,
        span: Span,
    },
    Skill {
        name: String,
        span: Span,
    },
    Block(Block),
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub kind: BlockKind,
    pub target: Option<String>,
    /// Optional scene tag on visual blocks: `visual for main scene ember { ... }`.
    pub scene: Option<String>,
    /// The original source within the block braces. Semantic lowering owns the
    /// language-specific interpretation of this extensible content.
    pub body: String,
    pub content: BlockContent,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum BlockContent {
    Raw,
    Timeline {
        acts: Vec<ActDecl>,
        shots: Vec<ShotDecl>,
    },
    Tempo { events: Vec<TempoDecl> },
    Intent { emotions: Vec<EmotionDecl> },
    Orchestrate(OrchestrationDecl),
    Visual { elements: Vec<VisualDecl> },
    Music(MusicDecl),
    Audio { tracks: Vec<AudioTrackDecl> },
    Lyric(LyricDecl),
}

/// Timeline / shot endpoint unit. Bare numbers default to [`TimeUnit::Bars`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeUnit {
    Bars,
    Beats,
    Seconds,
}

/// A numeric time with an explicit or defaulted unit (`8`, `8bar`, `8b`, `12.5s`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TimePoint {
    pub value: f64,
    pub unit: TimeUnit,
}

impl TimePoint {
    pub fn bars(value: f64) -> Self {
        Self {
            value,
            unit: TimeUnit::Bars,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActDecl {
    pub name: String,
    pub start: TimePoint,
    pub end: TimePoint,
    pub span: Span,
}

/// Timed cut: `shot id : range scene name [for act] [transition] [freeze|reverse|melt source]`.
#[derive(Debug, Clone, Serialize)]
pub struct ShotDecl {
    pub id: String,
    pub start: TimePoint,
    pub end: TimePoint,
    pub scene: String,
    pub target: Option<String>,
    /// Incoming transition kind (`cut`, `xfade`, `flash`, `iris`, `slices`).
    pub transition: String,
    /// Transition length token (`0.5`, `0.5bar`, `2b`, `0.25s`).
    pub transition_len: String,
    /// Optional time remap mode (`freeze`, `reverse`, `melt`).
    pub remap_mode: Option<String>,
    /// Source shot id for remap (immutable reference).
    pub remap_source: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct TempoDecl {
    pub bpm: f64,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmotionDecl {
    pub emotion: String,
    pub target: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct OrchestrationDecl {
    pub mappings: Vec<MappingDecl>,
    pub hard_constraints: Vec<String>,
    pub soft_preferences: Vec<String>,
    pub style_references: Vec<String>,
    pub transition_strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingDecl {
    pub intent: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum VisualDecl {
    Background {
        color: String,
        span: Span,
    },
    Asset {
        kind: VisualAssetKind,
        uri: String,
        span: Span,
    },
    Camera {
        property: String,
        value: String,
        span: Span,
    },
    Particles {
        property: String,
        value: String,
        span: Span,
    },
    /// Full-frame post darken; strength in `[0, 1]` (reference default ≈ 0.35).
    Vignette {
        strength: String,
        span: Span,
    },
    /// Film grain; strength in `[0, 1]` (reference default ≈ 0.06).
    Grain {
        strength: String,
        span: Span,
    },
    /// Soft bloom; strength in `[0, 1]` (reference default ≈ 0.3).
    Bloom {
        strength: String,
        span: Span,
    },
    /// Gradient-map LUT id (`P2`) or PNG path, with mix in `[0, 1]`.
    Lut {
        name: String,
        mix: String,
        span: Span,
    },
    /// Shot FX: `fx pulse 0.8` or `fx stamp "残" 0.9` (procedural; not a lyric layer).
    Fx {
        kind: String,
        label: Option<String>,
        strength: String,
        span: Span,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum VisualAssetKind {
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MusicDecl {
    pub key: Option<String>,
    pub meter: Option<MeterDecl>,
    pub motif: Vec<String>,
    pub template: Option<String>,
    pub density: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeterDecl {
    pub beats: u8,
    pub beat_unit: u8,
    pub span: Span,
}

/// `audio for act { track id { ... } }` track declaration.
#[derive(Debug, Clone, Serialize)]
pub struct AudioTrackDecl {
    pub id: String,
    /// Asset name or project-relative path from `from …`.
    pub from: String,
    pub role: String,
    pub offset: Option<String>,
    pub clip: Option<String>,
    /// Gain in dB when `gain` was written.
    pub gain_db: Option<f64>,
    /// Linear amplitude when `amplitude` was written.
    pub amplitude: Option<f64>,
    pub fade_in: Option<String>,
    pub fade_out: Option<String>,
    pub tail_silence: Option<String>,
    pub span: Span,
}

/// `lyric for act { lang …; line { word "…" start..end } }` (doc 14). Not `fx stamp`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct LyricDecl {
    pub lang: Option<String>,
    /// Asset name or path for external timing JSON (`from …`); first slice may leave unloaded.
    pub from: Option<String>,
    /// CueId×locale display catalog (`localized from …`; docs 18/20). Independent of vocal timing.
    pub localized_from: Option<String>,
    pub lines: Vec<LyricLineDecl>,
    /// Optional appearance block(s); compile may synthesize a default layer.
    pub layers: Vec<LyricLayerDecl>,
}

/// `layer { role … size … align … }` inside a lyric block.
#[derive(Debug, Clone, Serialize)]
pub struct LyricLayerDecl {
    pub role: Option<String>,
    pub size: Option<f64>,
    pub align: Option<String>,
    pub color: Option<String>,
    pub highlight_color: Option<String>,
    pub safe: Option<f64>,
    pub post_group: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct LyricLineDecl {
    pub words: Vec<LyricWordDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct LyricWordDecl {
    pub text: String,
    /// Raw range token (`0s..0.4s` / `0..1bar`); compile converts via TimeMap.
    pub range: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum BlockKind {
    Intent,
    Timeline,
    Tempo,
    Music,
    Audio,
    Lyric,
    Visual,
    Orchestrate,
    /// Deprecated compatibility spelling. New scripts should use `orchestrate`.
    Directing,
}
