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
    /// The original source within the block braces. Semantic lowering owns the
    /// language-specific interpretation of this extensible content.
    pub body: String,
    pub content: BlockContent,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum BlockContent {
    Raw,
    Timeline { acts: Vec<ActDecl> },
    Tempo { events: Vec<TempoDecl> },
    Intent { emotions: Vec<EmotionDecl> },
    Orchestrate(OrchestrationDecl),
}

#[derive(Debug, Clone, Serialize)]
pub struct ActDecl {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
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
