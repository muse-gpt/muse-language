//! Muse source front end: logos tokenisation, chumsky delimiter parsing, and
//! recoverable AST construction with miette-compatible diagnostics.
mod ast;
pub mod diagnostics;
pub mod lexer;
mod parser;
mod time_parse;

pub use ast::{
    ActDecl, AudioTrackDecl, Block, BlockContent, BlockKind, Declaration, EmotionDecl, InstancePortArg,
    LyricDecl, LyricLayerDecl, LyricLineDecl, LyricWordDecl, MappingDecl, MeterDecl, MusicDecl,
    OrchestrationDecl, Script, ShotDecl, TempoDecl, TimePoint, TimeUnit, VisualAssetKind, VisualDecl,
};
pub use lexer::{Token, TokenKind, lex};
pub use parser::{ParseOutput, parse};
pub use time_parse::{parse_time_point, parse_time_range};
