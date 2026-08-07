use chumsky::{error::Rich, extra, prelude::*, span::SimpleSpan};
use muse_types::{Diagnostic, Span};

use crate::{
    ast::{
        ActDecl, Block, BlockContent, BlockKind, Declaration, EmotionDecl, MappingDecl, OrchestrationDecl, Script,
        TempoDecl, VisualAssetKind, VisualDecl,
    },
    diagnostics::from_chumsky,
    lexer::{Token, TokenKind, lex, span},
};

#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub script: Script,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a Muse source file. Logos is responsible for token boundaries and
/// comments; chumsky validates balanced syntax and provides recoverable syntax
/// diagnostics; this pass then lowers top-level declarations while preserving
/// verbatim block bodies for the semantic compiler.
pub fn parse(source: &str) -> ParseOutput {
    let (tokens, mut diagnostics) = lex(source);
    diagnostics.extend(validate_braces(source, &tokens));

    let mut declarations = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::Namespace => parse_namespace(source, &tokens, &mut index, &mut declarations, &mut diagnostics),
            TokenKind::Using => parse_using(source, &tokens, &mut index, &mut declarations, &mut diagnostics),
            TokenKind::Asset => parse_asset(source, &tokens, &mut index, &mut declarations, &mut diagnostics),
            TokenKind::Use => parse_skill(source, &tokens, &mut index, &mut declarations, &mut diagnostics),
            TokenKind::Intent
            | TokenKind::Timeline
            | TokenKind::Tempo
            | TokenKind::Music
            | TokenKind::Visual
            | TokenKind::Orchestrate
            | TokenKind::Directing => parse_block(source, &tokens, &mut index, &mut declarations, &mut diagnostics),
            _ => {
                let token = &tokens[index];
                diagnostics.push(Diagnostic::error(
                    format!("expected a Muse declaration, found `{}`", token.lexeme),
                    token.span(source),
                ));
                recover_top_level(&tokens, &mut index);
            }
        }
    }
    ParseOutput {
        script: Script { declarations },
        diagnostics,
    }
}

fn validate_braces(source: &str, tokens: &[Token]) -> Vec<Diagnostic> {
    let input: Vec<(TokenKind, SimpleSpan<usize>)> = tokens
        .iter()
        .map(|token| (token.kind.clone(), (token.start..token.end).into()))
        .collect();
    let end_span: SimpleSpan<usize> = (source.len()..source.len()).into();
    let stream = chumsky::input::Stream::from_iter(input).map(end_span, |(token, span)| (token, span));

    // This recursive grammar is deliberately small: inner block syntax is
    // extensible, but delimiters are language syntax and must be balanced.
    let atom = any::<_, extra::Err<Rich<'static, TokenKind, SimpleSpan<usize>>>>()
        .filter(|token: &TokenKind| !matches!(token, TokenKind::LBrace | TokenKind::RBrace))
        .ignored();
    let grammar = recursive(|body| {
        atom.or(body
            .clone()
            .delimited_by(just(TokenKind::LBrace), just(TokenKind::RBrace)))
            .repeated()
            .ignored()
    })
    .then_ignore(end());

    grammar
        .parse(stream)
        .into_errors()
        .into_iter()
        .map(|error| from_chumsky(source, error))
        .collect()
}

fn parse_namespace(
    source: &str,
    tokens: &[Token],
    index: &mut usize,
    declarations: &mut Vec<Declaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = *index;
    *index += 1;
    let Some(name) = identifier(tokens, *index) else {
        missing(source, tokens, start, "namespace requires an identifier", diagnostics);
        recover_statement(tokens, index);
        return;
    };
    *index += 1;
    if !consume(tokens, index, TokenKind::Semicolon) {
        missing(source, tokens, start, "namespace declaration requires `;`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    declarations.push(Declaration::Namespace {
        name,
        span: joined_span(source, &tokens[start], &tokens[*index - 1]),
    });
}

fn parse_using(
    source: &str,
    tokens: &[Token],
    index: &mut usize,
    declarations: &mut Vec<Declaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = *index;
    *index += 1;
    let Some(value) = string(tokens, *index) else {
        missing(source, tokens, start, "using requires a quoted source", diagnostics);
        recover_statement(tokens, index);
        return;
    };
    *index += 1;
    let alias = if consume(tokens, index, TokenKind::As) {
        let alias = identifier(tokens, *index);
        if alias.is_none() {
            missing(source, tokens, start, "`as` requires an alias", diagnostics);
        }
        *index += usize::from(alias.is_some());
        alias
    } else {
        None
    };
    if !consume(tokens, index, TokenKind::Semicolon) {
        missing(source, tokens, start, "using declaration requires `;`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    declarations.push(Declaration::Using {
        source: value,
        alias,
        span: joined_span(source, &tokens[start], &tokens[*index - 1]),
    });
}

fn parse_asset(
    source: &str,
    tokens: &[Token],
    index: &mut usize,
    declarations: &mut Vec<Declaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = *index;
    *index += 1;
    let name = identifier(tokens, *index);
    *index += usize::from(name.is_some());
    let has_equals = consume(tokens, index, TokenKind::Equals);
    let uri = string(tokens, *index);
    *index += usize::from(uri.is_some());
    if name.is_none() || !has_equals || uri.is_none() {
        missing(source, tokens, start, "asset requires `name = \"uri\"`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    if !consume(tokens, index, TokenKind::Semicolon) {
        missing(source, tokens, start, "asset declaration requires `;`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    declarations.push(Declaration::Asset {
        name: name.unwrap_or_default(),
        uri: uri.unwrap_or_default(),
        span: joined_span(source, &tokens[start], &tokens[*index - 1]),
    });
}

fn parse_skill(
    source: &str,
    tokens: &[Token],
    index: &mut usize,
    declarations: &mut Vec<Declaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = *index;
    *index += 1;
    if !consume(tokens, index, TokenKind::Skill) {
        missing(source, tokens, start, "expected `skill` after `use`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    let name = string(tokens, *index).or_else(|| identifier(tokens, *index));
    *index += usize::from(name.is_some());
    if name.is_none() || !consume(tokens, index, TokenKind::Semicolon) {
        missing(source, tokens, start, "use skill requires a name and `;`", diagnostics);
        recover_statement(tokens, index);
        return;
    }
    declarations.push(Declaration::Skill {
        name: name.unwrap_or_default(),
        span: joined_span(source, &tokens[start], &tokens[*index - 1]),
    });
}

fn parse_block(
    source: &str,
    tokens: &[Token],
    index: &mut usize,
    declarations: &mut Vec<Declaration>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = *index;
    let kind = block_kind(&tokens[*index].kind).expect("caller only dispatches block keywords");
    *index += 1;
    let target = if matches!(kind, BlockKind::Music | BlockKind::Visual | BlockKind::Orchestrate)
        && consume(tokens, index, TokenKind::For)
    {
        let target = identifier(tokens, *index);
        *index += usize::from(target.is_some());
        if target.is_none() {
            missing(source, tokens, start, "`for` requires an act name", diagnostics);
        }
        target
    } else {
        None
    };
    if !consume(tokens, index, TokenKind::LBrace) {
        missing(source, tokens, start, "block requires `{`", diagnostics);
        recover_top_level(tokens, index);
        return;
    }
    let body_start = tokens[*index - 1].end;
    let mut depth = 1usize;
    let mut close = None;
    while *index < tokens.len() {
        match tokens[*index].kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                depth -= 1;
                if depth == 0 {
                    close = Some(*index);
                    break;
                }
            }
            _ => {}
        }
        *index += 1;
    }
    let Some(close_index) = close else {
        diagnostics.push(Diagnostic::error(
            "unclosed block",
            span(source, tokens[start].start, source.len()),
        ));
        *index = tokens.len();
        return;
    };
    let end = tokens[close_index].end;
    let body = source[body_start..tokens[close_index].start].to_owned();
    let content = typed_content(
        source,
        kind.clone(),
        &body,
        span(source, body_start, tokens[close_index].start),
    );
    declarations.push(Declaration::Block(Block {
        kind,
        target,
        body,
        content,
        span: span(source, tokens[start].start, end),
    }));
    *index = close_index + 1;
}

fn typed_content(source: &str, kind: BlockKind, body: &str, body_span: Span) -> BlockContent {
    match kind {
        BlockKind::Timeline => BlockContent::Timeline {
            acts: body
                .lines()
                .filter_map(|line| {
                    let words: Vec<_> = line.split_whitespace().collect();
                    if words.len() >= 4 && words[0] == "act" && words[2] == ":" {
                        let (start_bar, end_bar) = words[3].split_once("..")?;
                        Some(ActDecl {
                            name: words[1].to_owned(),
                            start_bar: start_bar.parse().ok()?,
                            end_bar: end_bar.parse().ok()?,
                            span: body_span,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
        },
        BlockKind::Tempo => BlockContent::Tempo {
            events: body
                .lines()
                .filter_map(|line| {
                    let words: Vec<_> = line.split_whitespace().collect();
                    let index = words.iter().position(|word| *word == "bpm")?;
                    let bpm = words
                        .get(index + 1)
                        .or_else(|| index.checked_sub(1).and_then(|i| words.get(i)))?
                        .parse()
                        .ok()?;
                    Some(TempoDecl { bpm, span: body_span })
                })
                .collect(),
        },
        BlockKind::Intent => BlockContent::Intent {
            emotions: body
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    let emotion = line.strip_prefix("emotion ")?.split('"').nth(1)?.to_owned();
                    let target = line.split_once(" for ").map(|(_, target)| target.trim().to_owned());
                    Some(EmotionDecl {
                        emotion,
                        target,
                        span: body_span,
                    })
                })
                .collect(),
        },
        BlockKind::Orchestrate => BlockContent::Orchestrate(OrchestrationDecl {
            mappings: values_after(body, "map ")
                .into_iter()
                .map(|intent| MappingDecl {
                    intent,
                    span: body_span,
                })
                .collect(),
            hard_constraints: list_after(body, "hard_constraints:"),
            soft_preferences: list_after(body, "soft_preferences:"),
            style_references: values_after(body, "style_reference"),
            transition_strategy: body.contains("transition_strategy").then_some("declared".to_owned()),
        }),
        BlockKind::Visual => BlockContent::Visual {
            elements: body
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if let Some(value) = line.strip_prefix("background color") {
                        return Some(VisualDecl::Background {
                            color: value.trim().trim_matches('"').to_owned(),
                            span: body_span,
                        });
                    }
                    if let Some(value) = line.strip_prefix("image") {
                        return Some(VisualDecl::Asset {
                            kind: VisualAssetKind::Image,
                            uri: value.trim().trim_matches('"').to_owned(),
                            span: body_span,
                        });
                    }
                    if let Some(value) = line.strip_prefix("video") {
                        return Some(VisualDecl::Asset {
                            kind: VisualAssetKind::Video,
                            uri: value.trim().trim_matches('"').to_owned(),
                            span: body_span,
                        });
                    }
                    if let Some(value) = line.strip_prefix("camera") {
                        let (property, value) = value.trim().split_once(char::is_whitespace)?;
                        return Some(VisualDecl::Camera {
                            property: property.to_owned(),
                            value: value.trim().to_owned(),
                            span: body_span,
                        });
                    }
                    line.strip_prefix("particles").map(|value| VisualDecl::Particles {
                        property: "config".to_owned(),
                        value: value.trim().to_owned(),
                        span: body_span,
                    })
                })
                .collect(),
        },
        _ => {
            let _ = source;
            BlockContent::Raw
        }
    }
}

fn quoted_values(input: &str) -> Vec<String> {
    input.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

fn list_after(input: &str, marker: &str) -> Vec<String> {
    input
        .find(marker)
        .and_then(|start| input[start..].find(']').map(|end| &input[start..start + end]))
        .map(quoted_values)
        .unwrap_or_default()
}

fn values_after(input: &str, marker: &str) -> Vec<String> {
    input
        .match_indices(marker)
        .filter_map(|(start, _)| input[start..].split('"').nth(1).map(str::to_owned))
        .collect()
}

fn block_kind(token: &TokenKind) -> Option<BlockKind> {
    Some(match token {
        TokenKind::Intent => BlockKind::Intent,
        TokenKind::Timeline => BlockKind::Timeline,
        TokenKind::Tempo => BlockKind::Tempo,
        TokenKind::Music => BlockKind::Music,
        TokenKind::Visual => BlockKind::Visual,
        TokenKind::Orchestrate => BlockKind::Orchestrate,
        TokenKind::Directing => BlockKind::Directing,
        _ => return None,
    })
}

fn consume(tokens: &[Token], index: &mut usize, expected: TokenKind) -> bool {
    if tokens.get(*index).is_some_and(|token| token.kind == expected) {
        *index += 1;
        true
    } else {
        false
    }
}
fn identifier(tokens: &[Token], index: usize) -> Option<String> {
    tokens
        .get(index)
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.lexeme.clone())
}
fn string(tokens: &[Token], index: usize) -> Option<String> {
    tokens
        .get(index)
        .filter(|token| token.kind == TokenKind::String)
        .map(|token| unquote(&token.lexeme))
}
fn unquote(value: &str) -> String {
    serde_json::from_str(value).unwrap_or_else(|_| value.trim_matches('"').to_owned())
}
fn joined_span(source: &str, first: &Token, last: &Token) -> Span {
    span(source, first.start, last.end)
}
fn missing(source: &str, tokens: &[Token], start: usize, message: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::error(
        message,
        tokens.get(start).map_or_else(
            || Span::new(source.len(), source.len(), 1, 1),
            |token| token.span(source),
        ),
    ));
}
fn recover_statement(tokens: &[Token], index: &mut usize) {
    while *index < tokens.len() && tokens[*index].kind != TokenKind::Semicolon {
        *index += 1;
    }
    *index += usize::from(*index < tokens.len());
}
fn recover_top_level(tokens: &[Token], index: &mut usize) {
    while *index < tokens.len()
        && !matches!(
            tokens[*index].kind,
            TokenKind::Namespace
                | TokenKind::Using
                | TokenKind::Asset
                | TokenKind::Use
                | TokenKind::Intent
                | TokenKind::Timeline
                | TokenKind::Tempo
                | TokenKind::Music
                | TokenKind::Visual
                | TokenKind::Orchestrate
                | TokenKind::Directing
        )
    {
        *index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::{BlockContent, Declaration};

    #[test]
    fn accepts_hash_comments_only() {
        assert!(parse("# note\ntimeline { act intro : 0..8 }").diagnostics.is_empty());
        assert!(!parse("// note\ntimeline { act intro : 0..8 }").diagnostics.is_empty());
    }

    #[test]
    fn keeps_nested_block_body_for_ir_lowering() {
        let result = parse("orchestrate for verse { map \"loneliness\" to { camera_speed: 0.3 } }");
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(block) = &result.script.declarations[0] else {
            panic!("expected block")
        };
        assert_eq!(block.target.as_deref(), Some("verse"));
        assert!(block.body.contains("camera_speed"));
    }

    #[test]
    fn lowers_timeline_and_tempo_to_typed_content() {
        let result = parse("timeline { act intro : 0..8 } tempo { bpm 132 }");
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(timeline) = &result.script.declarations[0] else {
            panic!("expected timeline block")
        };
        let BlockContent::Timeline { acts } = &timeline.content else {
            panic!("expected typed timeline content")
        };
        assert_eq!(acts[0].name, "intro");
        assert_eq!((acts[0].start_bar, acts[0].end_bar), (0, 8));

        let Declaration::Block(tempo) = &result.script.declarations[1] else {
            panic!("expected tempo block")
        };
        let BlockContent::Tempo { events } = &tempo.content else {
            panic!("expected typed tempo content")
        };
        assert_eq!(events[0].bpm, 132.0);
    }

    #[test]
    fn recovers_after_bad_declaration() {
        let result = parse("asset bad = nope; timeline { act intro : 0..8 }");
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.script.declarations.len(), 1);
    }
}
