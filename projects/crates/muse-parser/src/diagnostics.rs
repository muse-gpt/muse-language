use chumsky::error::Rich;
use chumsky::span::SimpleSpan;
use muse_types::Diagnostic;

use crate::lexer::{TokenKind, span};

pub fn from_chumsky(source: &str, error: Rich<'_, TokenKind, SimpleSpan<usize>>) -> Diagnostic {
    let range = error.span().into_range();
    Diagnostic::error(format!("syntax error: {error}"), span(source, range.start, range.end))
}
