//! Muse source front end: logos tokenisation, chumsky delimiter parsing, and
//! recoverable AST construction with miette-compatible diagnostics.
mod ast;
pub mod diagnostics;
pub mod lexer;
mod parser;

pub use ast::{
    ActDecl, Block, BlockContent, BlockKind, Declaration, EmotionDecl, MappingDecl, OrchestrationDecl, Script,
    TempoDecl, VisualAssetKind, VisualDecl,
};
pub use lexer::{Token, TokenKind, lex};
pub use parser::{ParseOutput, parse};
