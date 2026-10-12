use chumsky::{error::Rich, extra, prelude::*, span::SimpleSpan};
use muse_types::{Diagnostic, Span};

use crate::{
    ast::{
        ActDecl, Block, BlockContent, BlockKind, Declaration, EmotionDecl, MappingDecl, MeterDecl, MusicDecl,
        ShotDecl,
        OrchestrationDecl, Script, TempoDecl, VisualAssetKind, VisualDecl,
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
    let scene = if matches!(kind, BlockKind::Visual)
        && identifier(tokens, *index).as_deref() == Some("scene")
    {
        *index += 1;
        let scene = identifier(tokens, *index);
        *index += usize::from(scene.is_some());
        if scene.is_none() {
            missing(
                source,
                tokens,
                start,
                "`scene` requires a scene name",
                diagnostics,
            );
        }
        scene
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
        scene,
        body,
        content,
        span: span(source, tokens[start].start, end),
    }));
    *index = close_index + 1;
}

fn typed_content(source: &str, kind: BlockKind, body: &str, body_span: Span) -> BlockContent {
    match kind {
        BlockKind::Timeline => {
            let (acts, shots) = parse_timeline_body(body, body_span);
            BlockContent::Timeline { acts, shots }
        }
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
            elements: parse_visual_elements(body, body_span),
        },
        BlockKind::Music => BlockContent::Music(MusicDecl {
            key: quoted_after(body, "key"),
            meter: body.lines().find_map(|line| {
                let value = line.split_once("meter")?.1.split_whitespace().next()?;
                let (beats, unit) = value.split_once('/')?;
                Some(MeterDecl {
                    beats: beats.parse().ok()?,
                    beat_unit: unit.parse().ok()?,
                    span: body_span,
                })
            }),
            motif: quoted_after(body, "motif")
                .map(|value| value.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
            template: quoted_after(body, "template"),
            density: body
                .lines()
                .find_map(|line| line.split_once("density")?.1.split_whitespace().next()?.parse().ok()),
        }),
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

fn parse_visual_elements(body: &str, body_span: Span) -> Vec<VisualDecl> {
    let mut elements = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            break;
        }
        if let Some(after) = trimmed.strip_prefix("background color") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Background {
                color: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("image") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Asset {
                kind: VisualAssetKind::Image,
                uri: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("video") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Asset {
                kind: VisualAssetKind::Video,
                uri: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("camera") {
            let after = after.trim_start();
            let (property, after_prop) = take_visual_token(after);
            let (value, next) = take_visual_value(after_prop);
            elements.push(VisualDecl::Camera {
                property,
                value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("particles") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Particles {
                property: "config".to_owned(),
                value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("vignette") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Vignette {
                strength: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("grain") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Grain {
                strength: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("bloom") {
            let (value, next) = take_visual_value(after);
            elements.push(VisualDecl::Bloom {
                strength: value,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("lut") {
            let (name, after_name) = take_visual_value(after);
            let after_name = after_name.trim_start();
            let (mix, next) = if after_name.is_empty()
                || after_name.starts_with("background")
                || after_name.starts_with("image")
                || after_name.starts_with("video")
                || after_name.starts_with("camera")
                || after_name.starts_with("particles")
                || after_name.starts_with("vignette")
                || after_name.starts_with("grain")
                || after_name.starts_with("bloom")
                || after_name.starts_with("lut")
                || after_name.starts_with("fx")
            {
                ("1.0".to_owned(), after_name)
            } else {
                take_visual_value(after_name)
            };
            elements.push(VisualDecl::Lut {
                name,
                mix,
                span: body_span,
            });
            rest = next;
            continue;
        }
        if let Some(after) = trimmed.strip_prefix("fx") {
            let after = after.trim_start();
            let (kind, after_kind) = take_visual_token(after);
            let after_kind = after_kind.trim_start();
            let (label, after_label) = if after_kind.starts_with('"') {
                let (label, rest) = take_visual_value(after_kind);
                (Some(label), rest.trim_start())
            } else {
                (None, after_kind)
            };
            let (strength, next) = take_visual_value(after_label);
            elements.push(VisualDecl::Fx {
                kind,
                label,
                strength,
                span: body_span,
            });
            rest = next;
            continue;
        }
        // Skip one grapheme cluster / char and keep scanning.
        let mut chars = trimmed.chars();
        chars.next();
        rest = chars.as_str();
    }
    elements
}

/// Scan timeline body tokens so `act` / `shot` may share a line.
fn parse_timeline_body(body: &str, span: Span) -> (Vec<ActDecl>, Vec<ShotDecl>) {
    let words: Vec<_> = body.split_whitespace().collect();
    let mut acts = Vec::new();
    let mut shots = Vec::new();
    let mut index = 0usize;
    while index < words.len() {
        if words[index] == "act" && index + 3 < words.len() && words[index + 2] == ":" {
            if let Some((start_bar, end_bar)) = words[index + 3].split_once("..") {
                if let (Ok(start_bar), Ok(end_bar)) =
                    (start_bar.parse::<u32>(), end_bar.parse::<u32>())
                {
                    acts.push(ActDecl {
                        name: words[index + 1].to_owned(),
                        start_bar,
                        end_bar,
                        span,
                    });
                    index += 4;
                    continue;
                }
            }
        }
        if words[index] == "shot" {
            if let Some((shot, consumed)) = parse_shot_at(&words[index..], span) {
                shots.push(shot);
                index += consumed;
                continue;
            }
        }
        index += 1;
    }
    (acts, shots)
}

/// `shot id : start..end scene name [for act] [cut|xfade|flash [bars]]` starting at `words[0]`.
fn parse_shot_at(words: &[&str], span: Span) -> Option<(ShotDecl, usize)> {
    if words.len() < 6 || words[0] != "shot" || words[2] != ":" || words[4] != "scene" {
        return None;
    }
    let (start_bar, end_bar) = words[3].split_once("..")?;
    let start_bar = start_bar.parse().ok()?;
    let end_bar = end_bar.parse().ok()?;
    let scene = words[5].trim_matches('"').to_owned();
    if scene.is_empty() {
        return None;
    }
    let mut index = 6;
    let target = if words.len() >= 8 && words[6] == "for" {
        index = 8;
        Some(words[7].to_owned())
    } else {
        None
    };
    let mut transition = "cut".to_owned();
    let mut transition_bars = "0".to_owned();
    if index < words.len() {
        match words[index] {
            "cut" => {
                index += 1;
            }
            kind @ ("xfade" | "flash") => {
                transition = kind.to_owned();
                index += 1;
                if index < words.len() {
                    if words[index].parse::<f32>().is_ok() {
                        transition_bars = words[index].to_owned();
                        index += 1;
                    } else {
                        transition_bars = if kind == "flash" {
                            "0.25".into()
                        } else {
                            "0.5".into()
                        };
                    }
                } else {
                    transition_bars = if kind == "flash" {
                        "0.25".into()
                    } else {
                        "0.5".into()
                    };
                }
            }
            _ => {}
        }
    }
    Some((
        ShotDecl {
            id: words[1].to_owned(),
            start_bar,
            end_bar,
            scene,
            target,
            transition,
            transition_bars,
            span,
        },
        index,
    ))
}

fn take_visual_value(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_owned(), &rest[end + 1..]);
        }
    }
    take_visual_token(input)
}

fn take_visual_token(input: &str) -> (String, &str) {
    let input = input.trim_start();
    let end = input
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    (input[..end].to_owned(), &input[end..])
}

fn values_after(input: &str, marker: &str) -> Vec<String> {
    input
        .match_indices(marker)
        .filter_map(|(start, _)| input[start..].split('"').nth(1).map(str::to_owned))
        .collect()
}

fn quoted_after(input: &str, keyword: &str) -> Option<String> {
    input.lines().find_map(|line| {
        let rest = line.split_once(keyword)?.1.trim();
        rest.split('"').nth(1).map(str::to_owned)
    })
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
        let BlockContent::Timeline { acts, shots } = &timeline.content else {
            panic!("expected typed timeline content")
        };
        assert_eq!(acts[0].name, "intro");
        assert_eq!((acts[0].start_bar, acts[0].end_bar), (0, 8));
        assert!(shots.is_empty());

        let Declaration::Block(tempo) = &result.script.declarations[1] else {
            panic!("expected tempo block")
        };
        let BlockContent::Tempo { events } = &tempo.content else {
            panic!("expected typed tempo content")
        };
        assert_eq!(events[0].bpm, 132.0);
    }

    #[test]
    fn lowers_timeline_shots() {
        let result = parse(
            "timeline { act main : 0..16 shot s01 : 0..4 scene ember shot s02 : 4..8 scene illust for main }",
        );
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(timeline) = &result.script.declarations[0] else {
            panic!("expected timeline block")
        };
        let BlockContent::Timeline { shots, .. } = &timeline.content else {
            panic!("expected typed timeline content")
        };
        assert_eq!(shots.len(), 2);
        assert_eq!(shots[0].id, "s01");
        assert_eq!((shots[0].start_bar, shots[0].end_bar), (0, 4));
        assert_eq!(shots[0].scene, "ember");
        assert_eq!(shots[0].target, None);
        assert_eq!(shots[1].id, "s02");
        assert_eq!(shots[1].scene, "illust");
        assert_eq!(shots[1].target.as_deref(), Some("main"));
    }

    #[test]
    fn lowers_visual_scene_tag() {
        let result = parse(
            "timeline { act main : 0..4 } visual for main scene ember { background color \"#1a0c08\" }",
        );
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(visual) = &result.script.declarations[1] else {
            panic!("expected visual block")
        };
        assert_eq!(visual.target.as_deref(), Some("main"));
        assert_eq!(visual.scene.as_deref(), Some("ember"));
    }

    #[test]
    fn lowers_music_fields_to_typed_content() {
        let result = parse(
            "timeline { act theme : 0..8 } music for theme { key \"C major\" meter 4/4 motif \"C4 E4\" density 0.8 }",
        );
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(music) = &result.script.declarations[1] else {
            panic!("expected music block")
        };
        let BlockContent::Music(declaration) = &music.content else {
            panic!("expected typed music content")
        };
        assert_eq!(declaration.key.as_deref(), Some("C major"));
        assert_eq!(declaration.motif, ["C4", "E4"]);
        assert_eq!(declaration.density, Some(0.8));
        assert_eq!(declaration.meter.as_ref().map(|m| (m.beats, m.beat_unit)), Some((4, 4)));
    }

    #[test]
    fn recovers_after_bad_declaration() {
        let result = parse("asset bad = nope; timeline { act intro : 0..8 }");
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.script.declarations.len(), 1);
    }
}
