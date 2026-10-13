use chumsky::{error::Rich, extra, prelude::*, span::SimpleSpan};
use muse_types::{Diagnostic, Span};

use crate::{
    ast::{
        ActDecl, AudioTrackDecl, Block, BlockContent, BlockKind, Declaration, EmotionDecl, LyricDecl,
        LyricLayerDecl, LyricLineDecl, LyricWordDecl, MappingDecl, MeterDecl,
        MusicDecl, OrchestrationDecl, Script, ShotDecl, TempoDecl, VisualAssetKind, VisualDecl,
    },
    diagnostics::from_chumsky,
    lexer::{Token, TokenKind, lex, span},
    time_parse::{parse_time_point, parse_time_range},
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
            | TokenKind::Audio
            | TokenKind::Lyric
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
    let target = if matches!(
        kind,
        BlockKind::Music
            | BlockKind::Audio
            | BlockKind::Lyric
            | BlockKind::Visual
            | BlockKind::Orchestrate
    ) && consume(tokens, index, TokenKind::For)
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
        BlockKind::Audio => BlockContent::Audio {
            tracks: parse_audio_tracks(body, body_span),
        },
        BlockKind::Lyric => BlockContent::Lyric(parse_lyric_decl(body, body_span)),
        _ => {
            let _ = source;
            BlockContent::Raw
        }
    }
}

/// Parse `lang "…"`, optional `from …`, and nested `line { word "…" range }` bodies.
fn parse_lyric_decl(body: &str, span: Span) -> LyricDecl {
    let mut decl = LyricDecl::default();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("lang") {
            if let Some(lang) = quoted_after(trimmed, "lang") {
                decl.lang = Some(lang);
            }
        } else if let Some(rest) = trimmed.strip_prefix("localized") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix("from") {
                let rest = rest.trim().trim_matches(';').trim();
                if let Some(q) = rest.strip_prefix('"').and_then(|s| s.split('"').next()) {
                    decl.localized_from = Some(q.to_owned());
                } else if !rest.is_empty() {
                    decl.localized_from = Some(rest.to_owned());
                }
            }
        } else if trimmed.starts_with("from") {
            let rest = trimmed["from".len()..].trim().trim_matches(';').trim();
            if let Some(q) = rest.strip_prefix('"').and_then(|s| s.split('"').next()) {
                decl.from = Some(q.to_owned());
            } else if !rest.is_empty() {
                decl.from = Some(rest.to_owned());
            }
        }
    }
    decl.lines = parse_lyric_lines(body, span);
    decl.layers = parse_lyric_layers(body, span);
    decl
}

fn advance_utf8(body: &str, i: usize) -> usize {
    body[i..]
        .chars()
        .next()
        .map(|ch| i + ch.len_utf8())
        .unwrap_or_else(|| body.len())
}

fn parse_lyric_layers(body: &str, span: Span) -> Vec<LyricLayerDecl> {
    let mut layers = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if !body[i..].starts_with("layer") {
            i = advance_utf8(body, i);
            continue;
        }
        let after = i + "layer".len();
        if after < bytes.len() && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_') {
            i = advance_utf8(body, i);
            continue;
        }
        i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'{' {
            continue;
        }
        i += 1;
        let inner_start = i;
        let mut depth = 1usize;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            break;
        }
        let inner = &body[inner_start..i];
        i += 1;
        layers.push(parse_lyric_layer_fields(inner, span));
    }
    layers
}

fn parse_lyric_layer_fields(inner: &str, span: Span) -> LyricLayerDecl {
    let mut decl = LyricLayerDecl {
        role: None,
        size: None,
        align: None,
        color: None,
        highlight_color: None,
        safe: None,
        post_group: None,
        span,
    };
    for raw in inner.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(key) = words.next() else {
            continue;
        };
        match key {
            "role" => decl.role = words.next().map(str::to_owned),
            "size" => {
                if let Some(v) = words.next() {
                    decl.size = v.parse().ok();
                }
            }
            "align" => decl.align = words.next().map(str::to_owned),
            "color" => {
                if let Some(rest) = line.split_once("color").map(|(_, r)| r.trim()) {
                    decl.color = quoted_or_token(rest);
                }
            }
            "highlight" => {
                // `highlight color "#ffe08a"`
                let rest = line["highlight".len()..].trim();
                if let Some(rest) = rest.strip_prefix("color") {
                    decl.highlight_color = quoted_or_token(rest.trim());
                }
            }
            "safe" => {
                if let Some(v) = words.next() {
                    decl.safe = v.parse().ok();
                }
            }
            "post_group" => decl.post_group = words.next().map(str::to_owned),
            _ => {}
        }
    }
    decl
}

fn quoted_or_token(rest: &str) -> Option<String> {
    let rest = rest.trim().trim_matches(';').trim();
    if let Some(q) = rest.strip_prefix('"').and_then(|s| s.split('"').next()) {
        Some(q.to_owned())
    } else {
        rest.split_whitespace().next().map(str::to_owned)
    }
}

fn parse_lyric_lines(body: &str, span: Span) -> Vec<LyricLineDecl> {
    let mut lines = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if !body[i..].starts_with("line") {
            i = advance_utf8(body, i);
            continue;
        }
        let after = i + "line".len();
        if after < bytes.len() && (bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_') {
            i = advance_utf8(body, i);
            continue;
        }
        i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'{' {
            continue;
        }
        i += 1;
        let inner_start = i;
        let mut depth = 1usize;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            break;
        }
        let inner = &body[inner_start..i];
        i += 1;
        let words = parse_lyric_words(inner, span);
        if !words.is_empty() {
            lines.push(LyricLineDecl { words, span });
        }
    }
    lines
}

fn parse_lyric_words(inner: &str, span: Span) -> Vec<LyricWordDecl> {
    let mut words = Vec::new();
    let mut rest = inner;
    while let Some(rel) = rest.find("word") {
        if rel > 0 {
            let prev = rest[..rel].chars().last().unwrap_or(' ');
            if prev.is_ascii_alphanumeric() || prev == '_' {
                rest = &rest[rel + 4..];
                continue;
            }
        }
        let after_kw = &rest[rel + 4..];
        if after_kw
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            rest = after_kw;
            continue;
        }
        let after = after_kw.trim_start();
        if let Some(q) = after.strip_prefix('"') {
            let Some(end_quote) = q.find('"') else {
                break;
            };
            let text = q[..end_quote].to_owned();
            let after_text = q[end_quote + 1..].trim_start();
            let range = after_text
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(';')
                .to_owned();
            if text.is_empty() || !range.contains("..") {
                rest = after_text;
                continue;
            }
            let range_start = after_text.find(&range).unwrap_or(0);
            words.push(LyricWordDecl {
                text,
                range: range.clone(),
                span,
            });
            rest = &after_text[range_start + range.len()..];
        } else {
            let mut parts = after.split_whitespace();
            let Some(text) = parts.next() else {
                break;
            };
            let Some(range_raw) = parts.next() else {
                break;
            };
            let range = range_raw.trim_matches(';').to_owned();
            if !range.contains("..") {
                rest = after;
                continue;
            }
            let resume_at = after.find(&range).map(|i| i + range.len()).unwrap_or(after.len());
            words.push(LyricWordDecl {
                text: text.to_owned(),
                range,
                span,
            });
            rest = &after[resume_at..];
        }
    }
    words
}

/// Parse nested `track id { … }` declarations inside an `audio` block body.
fn parse_audio_tracks(body: &str, span: Span) -> Vec<AudioTrackDecl> {
    let mut tracks = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if !body[i..].starts_with("track") {
            // Skip one char / keep scanning for the next track keyword.
            i += 1;
            continue;
        }
        let after_track = i + "track".len();
        if after_track < bytes.len() && (bytes[after_track].is_ascii_alphanumeric() || bytes[after_track] == b'_')
        {
            i += 1;
            continue;
        }
        i = after_track;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let id_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
            i += 1;
        }
        if id_start == i {
            continue;
        }
        let id = body[id_start..i].to_owned();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'{' {
            continue;
        }
        i += 1;
        let inner_start = i;
        let mut depth = 1usize;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 {
            break;
        }
        let inner = &body[inner_start..i];
        i += 1; // consume closing brace
        tracks.push(parse_audio_track_fields(id, inner, span));
    }
    tracks
}

fn parse_audio_track_fields(id: String, inner: &str, span: Span) -> AudioTrackDecl {
    let mut from = String::new();
    let mut role = "song".to_owned();
    let mut offset = None;
    let mut clip = None;
    let mut gain_db = None;
    let mut amplitude = None;
    let mut fade_in = None;
    let mut fade_out = None;
    let mut tail_silence = None;
    for line in inner.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(key) = words.next() else {
            continue;
        };
        match key {
            "from" => {
                if let Some(value) = words.next() {
                    from = value.trim_matches('"').to_owned();
                }
            }
            "role" => {
                if let Some(value) = words.next() {
                    role = value.to_owned();
                }
            }
            "offset" => offset = words.next().map(str::to_owned),
            "clip" => clip = words.next().map(str::to_owned),
            "gain" => {
                if let Some(value) = words.next() {
                    gain_db = value.parse().ok();
                }
            }
            "amplitude" => {
                if let Some(value) = words.next() {
                    amplitude = value.parse().ok();
                }
            }
            "fade_in" => fade_in = words.next().map(str::to_owned),
            "fade_out" => fade_out = words.next().map(str::to_owned),
            "tail_silence" => tail_silence = words.next().map(str::to_owned),
            _ => {}
        }
    }
    AudioTrackDecl {
        id,
        from,
        role,
        offset,
        clip,
        gain_db,
        amplitude,
        fade_in,
        fade_out,
        tail_silence,
        span,
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
            if let Some((start, end)) = parse_time_range(words[index + 3]) {
                acts.push(ActDecl {
                    name: words[index + 1].to_owned(),
                    start,
                    end,
                    span,
                });
                index += 4;
                continue;
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

/// `shot id : start..end scene name [for act] [transition [bars]] [freeze|reverse|melt source]` starting at `words[0]`.
fn parse_shot_at(words: &[&str], span: Span) -> Option<(ShotDecl, usize)> {
    if words.len() < 6 || words[0] != "shot" || words[2] != ":" || words[4] != "scene" {
        return None;
    }
    let (start, end) = parse_time_range(words[3])?;
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
    let mut transition_len = "0".to_owned();
    if index < words.len() {
        match words[index] {
            "cut" => {
                index += 1;
            }
            kind @ ("xfade" | "flash" | "iris" | "slices") => {
                transition = kind.to_owned();
                index += 1;
                let default_bars = match kind {
                    "flash" => "0.25",
                    _ => "0.5",
                };
                if index < words.len() && parse_time_point(words[index]).is_some() {
                    transition_len = words[index].to_owned();
                    index += 1;
                } else {
                    transition_len = default_bars.into();
                }
            }
            _ => {}
        }
    }
    let mut remap_mode = None;
    let mut remap_source = None;
    if index + 1 < words.len() {
        match words[index] {
            mode @ ("freeze" | "reverse" | "melt") => {
                let source = words[index + 1].trim_matches('"');
                if !source.is_empty()
                    && source != "shot"
                    && source != "act"
                    && source != "scene"
                    && source != "for"
                {
                    remap_mode = Some(mode.to_owned());
                    remap_source = Some(source.to_owned());
                    index += 2;
                }
            }
            _ => {}
        }
    }
    Some((
        ShotDecl {
            id: words[1].to_owned(),
            start,
            end,
            scene,
            target,
            transition,
            transition_len,
            remap_mode,
            remap_source,
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
        TokenKind::Audio => BlockKind::Audio,
        TokenKind::Lyric => BlockKind::Lyric,
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
                | TokenKind::Audio
                | TokenKind::Lyric
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
    fn parses_inline_lyric_block() {
        let result = parse(
            r#"lyric for main {
  lang "zh-Hans"
  line {
    word "残" 0s..0.4s
    word "响" 0.4s..0.8s
  }
}"#,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let Declaration::Block(block) = &result.script.declarations[0] else {
            panic!("expected block");
        };
        assert_eq!(block.target.as_deref(), Some("main"));
        let BlockContent::Lyric(lyric) = &block.content else {
            panic!("expected lyric content");
        };
        assert_eq!(lyric.lang.as_deref(), Some("zh-Hans"));
        assert_eq!(lyric.lines.len(), 1);
        assert_eq!(lyric.lines[0].words.len(), 2);
        assert_eq!(lyric.lines[0].words[0].text, "残");
        assert_eq!(lyric.lines[0].words[0].range, "0s..0.4s");
    }

    #[test]
    fn parses_lyric_layer_block() {
        let result = parse(
            r##"lyric for main {
  line { word "亮" 0s..1s }
  layer {
    role foreground
    size 48
    align center
    highlight color "#ffe08a"
    post_group none
  }
}"##,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let Declaration::Block(block) = &result.script.declarations[0] else {
            panic!("expected block");
        };
        let BlockContent::Lyric(lyric) = &block.content else {
            panic!("expected lyric");
        };
        assert_eq!(lyric.layers.len(), 1);
        assert_eq!(lyric.layers[0].role.as_deref(), Some("foreground"));
        assert_eq!(lyric.layers[0].size, Some(48.0));
        assert_eq!(lyric.layers[0].highlight_color.as_deref(), Some("#ffe08a"));
    }

    #[test]
    fn parses_localized_from_without_eating_from() {
        let result = parse(
            r#"lyric for main {
  from chorus_lyric
  localized from chorus_locale
  lang "zh-Hans"
  line { word "a" 0s..1s }
}"#,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let Declaration::Block(block) = &result.script.declarations[0] else {
            panic!("expected block");
        };
        let BlockContent::Lyric(lyric) = &block.content else {
            panic!("expected lyric");
        };
        assert_eq!(lyric.from.as_deref(), Some("chorus_lyric"));
        assert_eq!(lyric.localized_from.as_deref(), Some("chorus_locale"));
        assert_eq!(lyric.lang.as_deref(), Some("zh-Hans"));
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
        assert_eq!(acts[0].start.value, 0.0);
        assert_eq!(acts[0].end.value, 8.0);
        assert_eq!(acts[0].start.unit, crate::ast::TimeUnit::Bars);
        assert!(shots.is_empty());

        let seconds = parse("timeline { act sting : 0s..12.5s }");
        assert!(seconds.diagnostics.is_empty());
        let Declaration::Block(timeline) = &seconds.script.declarations[0] else {
            panic!("expected timeline")
        };
        let BlockContent::Timeline { acts, .. } = &timeline.content else {
            panic!("typed timeline")
        };
        assert_eq!(acts[0].end.unit, crate::ast::TimeUnit::Seconds);
        assert_eq!(acts[0].end.value, 12.5);

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
        assert_eq!(shots[0].start.value, 0.0);
        assert_eq!(shots[0].end.value, 4.0);
        assert_eq!(shots[0].scene, "ember");
        assert_eq!(shots[0].target, None);
        assert_eq!(shots[1].id, "s02");
        assert_eq!(shots[1].scene, "illust");
        assert_eq!(shots[1].target.as_deref(), Some("main"));
    }

    #[test]
    fn lowers_shot_time_remap() {
        let result = parse(
            "timeline { act main : 0..24 shot s01 : 0..8 scene a shot s02 : 8..16 scene b reverse s01 shot s03 : 16..24 scene c freeze s01 }",
        );
        assert!(result.diagnostics.is_empty());
        let Declaration::Block(timeline) = &result.script.declarations[0] else {
            panic!("expected timeline block")
        };
        let BlockContent::Timeline { shots, .. } = &timeline.content else {
            panic!("expected typed timeline content")
        };
        assert_eq!(shots[1].remap_mode.as_deref(), Some("reverse"));
        assert_eq!(shots[1].remap_source.as_deref(), Some("s01"));
        assert_eq!(shots[2].remap_mode.as_deref(), Some("freeze"));
        assert_eq!(shots[2].remap_source.as_deref(), Some("s01"));
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
