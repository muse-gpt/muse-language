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
}

#[derive(Debug, Clone, Serialize)]
pub struct ActDecl {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub span: Span,
}

/// Timed cut inside a timeline: `shot s01 : 0..4 scene ember [for main]`.
#[derive(Debug, Clone, Serialize)]
pub struct ShotDecl {
    pub id: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub scene: String,
    pub target: Option<String>,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum BlockKind {
    Intent,
    Timeline,
    Tempo,
    Music,
    Visual,
    Orchestrate,
    /// Deprecated compatibility spelling. New scripts should use `orchestrate`.
    Directing,
}
