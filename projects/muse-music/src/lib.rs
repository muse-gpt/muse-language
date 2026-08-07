//! Muse's neutral musical score, deterministic composition template, and exporters.
use anyhow::{Context, Result, bail};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
};
use muse_parser::{BlockKind, Declaration, parse};
use rusty_mp3::{Mp3Decoder, Mp3Encoder, Mp3EncoderConfig};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{
    collections::BTreeMap,
    f32::consts::TAU,
    fs::{self, File},
    io::BufWriter,
    path::Path,
    sync::Arc,
};

mod model;
pub use model::{Meter, Note, SAMPLE_RATE, Score, TICKS_PER_QUARTER, TempoEvent, Track};

/// Compose a deterministic, playable score from Muse's timeline and music blocks.
pub fn compose(source: &str) -> Result<Score> {
    let parsed = parse(source);
    if let Some(error) = parsed
        .diagnostics
        .iter()
        .find(|d| matches!(d.severity, muse_types::Severity::Error))
    {
        bail!(
            "cannot compose invalid Muse script at {}:{}: {}",
            error.span.line,
            error.span.column,
            error.message
        );
    }
    let mut acts = Vec::<ActSpec>::new();
    let mut tempo_lines = Vec::new();
    let mut music = BTreeMap::<String, MusicSpec>::new();
    let mut title = None;
    let mut intents = String::new();
    for declaration in parsed.script.declarations {
        match declaration {
            Declaration::Namespace { name, .. } => title = Some(name),
            Declaration::Block(block) => match block.kind {
                BlockKind::Timeline => acts.extend(parse_acts(&block.body)),
                BlockKind::Tempo => tempo_lines.push(block.body),
                BlockKind::Intent | BlockKind::Orchestrate => {
                    intents.push_str(&strip_comments(&block.body));
                    intents.push('\n');
                }
                BlockKind::Music => {
                    if let Some(target) = block.target {
                        music.insert(target, parse_music(&block.body));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    if acts.is_empty() {
        bail!("a timeline with at least one act is required to compose music")
    }
    acts.sort_by_key(|act| act.start_bar);
    let default_spec = music.values().next().cloned().unwrap_or_default();
    let meter = music.values().find_map(|spec| spec.meter.clone()).unwrap_or_default();
    let mut score = Score {
        title,
        meter,
        key: default_spec.key.clone().unwrap_or_else(|| "E minor".into()),
        soundfont: music.values().find_map(|spec| spec.soundfont.clone()),
        tracks: default_tracks(),
        ..Score::default()
    };
    let beats_per_bar = u64::from(score.meter.beats) * 4 / u64::from(score.meter.beat_unit.max(1));
    score.tempo_events = parse_tempi(&tempo_lines.join("\n"), beats_per_bar);
    score.tempo_bpm = score.tempo_events.first().map(|event| event.bpm).unwrap_or(120.0);

    for act in acts {
        let spec = music.get(&act.name).cloned().unwrap_or_else(|| default_spec.clone());
        let mood = mood_for(&act.name, &intents, &spec);
        compose_act(&mut score, &act, &spec, mood, beats_per_bar);
    }
    score.notes.sort_by_key(|note| (note.start_tick, note.track, note.key));
    Ok(score)
}

#[derive(Debug, Clone)]
struct ActSpec {
    name: String,
    start_bar: u32,
    end_bar: u32,
}

#[derive(Debug, Clone, Default)]
struct MusicSpec {
    key: Option<String>,
    meter: Option<Meter>,
    motif: Vec<u8>,
    density: f32,
    soundfont: Option<String>,
    template: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mood {
    Assault,
    Fracture,
    Lament,
    Awakening,
    FinalCharge,
    Neutral,
}

fn parse_acts(body: &str) -> Vec<ActSpec> {
    strip_comments(body)
        .lines()
        .filter_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            let range = parts
                .iter()
                .position(|value| *value == ":")
                .and_then(|index| parts.get(index + 1))?;
            let (start_bar, end_bar) = range.split_once("..")?;
            if parts.first()? != &"act" {
                return None;
            }
            Some(ActSpec {
                name: parts.get(1)?.to_string(),
                start_bar: start_bar.parse().ok()?,
                end_bar: end_bar.parse().ok()?,
            })
        })
        .collect()
}

fn parse_tempi(body: &str, beats_per_bar: u64) -> Vec<TempoEvent> {
    let mut result = Vec::new();
    for line in strip_comments(body).lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        let bpm_index = words.iter().position(|word| *word == "bpm");
        let Some(index) = bpm_index else { continue };
        let bpm = words
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|i| words.get(i)))
            .and_then(|value| value.parse::<f64>().ok());
        let Some(bpm) = bpm.filter(|value| *value > 0.0) else {
            continue;
        };
        let bar = words
            .iter()
            .position(|word| *word == "at")
            .and_then(|i| words.get(i + 1))
            .or_else(|| words.first())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        result.push(TempoEvent {
            start_tick: bar * beats_per_bar * u64::from(TICKS_PER_QUARTER),
            bpm,
        });
    }
    if result.is_empty() {
        result.push(TempoEvent {
            start_tick: 0,
            bpm: 120.0,
        });
    }
    result.sort_by_key(|event| event.start_tick);
    result.dedup_by_key(|event| event.start_tick);
    result
}

fn parse_music(body: &str) -> MusicSpec {
    let clean = strip_comments(body);
    let mut spec = MusicSpec {
        density: 0.65,
        ..MusicSpec::default()
    };
    for line in clean.lines() {
        let line = line.trim();
        if let Some(value) = quoted_after(line, "key") {
            spec.key = Some(value);
        }
        if let Some(value) = quoted_after(line, "soundfont") {
            spec.soundfont = Some(value);
        }
        if let Some(value) = quoted_after(line, "template") {
            spec.template = Some(value);
        }
        if let Some(value) = quoted_after(line, "motif") {
            spec.motif = value.split_whitespace().filter_map(parse_note_name).collect();
        }
        if let Some(value) = line.strip_prefix("meter").map(str::trim)
            && let Some((beats, unit)) = value.split_once('/')
            && let (Ok(beats), Ok(beat_unit)) = (beats.trim().parse(), unit.trim().parse())
        {
            spec.meter = Some(Meter { beats, beat_unit });
        }
        if let Some(value) = line.strip_prefix("density").and_then(|value| value.trim().parse().ok()) {
            spec.density = value;
        }
    }
    spec
}

fn quoted_after(line: &str, keyword: &str) -> Option<String> {
    line.strip_prefix(keyword)?.split('"').nth(1).map(str::to_owned)
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                string = false;
            }
            continue;
        }
        if ch == '"' {
            string = true;
            out.push(ch);
            continue;
        }
        if ch == '#' {
            for next in chars.by_ref() {
                if next == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn mood_for(act: &str, intents: &str, spec: &MusicSpec) -> Mood {
    let text = format!("{} {} {}", act, intents, spec.template.clone().unwrap_or_default()).to_ascii_lowercase();
    let act_name = act.to_ascii_lowercase();
    if act_name.contains("fracture") {
        Mood::Fracture
    } else if act_name.contains("lament") {
        Mood::Lament
    } else if act_name.contains("awakening") {
        Mood::Awakening
    } else if act_name.contains("final") || act_name.contains("charge") {
        Mood::FinalCharge
    } else if act_name.contains("assault") || act_name.contains("battle") {
        Mood::Assault
    } else if text.contains("fracture") || text.contains("shock") || text.contains("emptiness") {
        Mood::Fracture
    } else if text.contains("lament") || text.contains("grief") || text.contains("sad") {
        Mood::Lament
    } else if text.contains("awakening") || text.contains("resolve") || text.contains("hope") {
        Mood::Awakening
    } else if text.contains("final") || text.contains("triumph") || text.contains("victory") {
        Mood::FinalCharge
    } else if text.contains("assault") || text.contains("violent") || text.contains("battle") {
        Mood::Assault
    } else {
        Mood::Neutral
    }
}

fn default_tracks() -> Vec<Track> {
    vec![
        Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "motif".into(),
        },
        Track {
            name: "Strings".into(),
            channel: 1,
            program: 48,
            role: "harmony".into(),
        },
        Track {
            name: "Brass".into(),
            channel: 2,
            program: 61,
            role: "accent".into(),
        },
        Track {
            name: "Percussion".into(),
            channel: 9,
            program: 0,
            role: "pulse".into(),
        },
    ]
}

fn compose_act(score: &mut Score, act: &ActSpec, spec: &MusicSpec, mood: Mood, beats_per_bar: u64) {
    let bar_ticks = beats_per_bar * u64::from(score.ticks_per_quarter);
    let root = key_root(spec.key.as_deref().unwrap_or(&score.key));
    let motif = if spec.motif.is_empty() {
        vec![root + 12, root + 15, root + 14, root + 12]
    } else {
        spec.motif.clone()
    };
    for bar in act.start_bar..act.end_bar {
        let start = u64::from(bar) * bar_ticks;
        let local = bar - act.start_bar;
        let chord = chord_for(root, mood, local);
        let melody = transform_motif(&motif, mood, local);
        let note_len = (bar_ticks / melody.len().max(1) as u64).max(1);
        for (index, key) in melody.into_iter().enumerate() {
            push_note(
                score,
                0,
                key,
                match mood {
                    Mood::Lament => 74,
                    Mood::FinalCharge => 108,
                    _ => 92,
                },
                start + index as u64 * note_len,
                note_len.saturating_mul(9) / 10,
                if mood == Mood::Lament { "legato" } else { "marcato" },
            );
        }
        for key in chord {
            push_note(
                score,
                1,
                key,
                if mood == Mood::Lament { 58 } else { 76 },
                start,
                bar_ticks.saturating_sub(12),
                "sustain",
            );
        }
        if !matches!(mood, Mood::Lament | Mood::Fracture) {
            push_note(
                score,
                2,
                root.saturating_add(if mood == Mood::FinalCharge { 24 } else { 12 }),
                85,
                start + bar_ticks / 2,
                bar_ticks / 3,
                "accent",
            );
        }
        match mood {
            Mood::Assault | Mood::FinalCharge | Mood::Awakening | Mood::Neutral => {
                let subdivisions = if mood == Mood::FinalCharge { 8 } else { 4 };
                for step in 0..subdivisions {
                    let tick = start + bar_ticks * step / subdivisions;
                    push_note(
                        score,
                        3,
                        if step % 2 == 0 { 36 } else { 42 },
                        if step % 2 == 0 { 105 } else { 68 },
                        tick,
                        bar_ticks / subdivisions / 2,
                        "percussion",
                    );
                }
            }
            Mood::Fracture => {
                if local == 0 {
                    push_note(score, 3, 49, 100, start, bar_ticks / 4, "crash");
                }
            }
            Mood::Lament => {}
        }
    }
}

fn push_note(
    score: &mut Score,
    track: usize,
    key: u8,
    velocity: u8,
    start_tick: u64,
    duration_ticks: u64,
    articulation: &str,
) {
    score.notes.push(Note {
        track,
        channel: score.tracks[track].channel,
        key,
        velocity,
        start_tick,
        duration_ticks,
        articulation: articulation.into(),
    });
}

fn chord_for(root: u8, mood: Mood, bar: u32) -> Vec<u8> {
    let offsets: &[i8] = match mood {
        Mood::Lament => &[0, -3, -5, -7],
        Mood::FinalCharge => &[0, 5, 7, 12],
        Mood::Fracture => &[0, -1, 6],
        _ => &[0, 3, 7, 10],
    };
    let transpose = [0i8, 5, 7, 3][bar as usize % 4];
    offsets
        .iter()
        .map(|offset| (i16::from(root) + i16::from(*offset) + i16::from(transpose)).clamp(24, 108) as u8)
        .collect()
}

fn transform_motif(motif: &[u8], mood: Mood, bar: u32) -> Vec<u8> {
    let mut result = motif.to_vec();
    match mood {
        Mood::Lament => {
            result.reverse();
            for key in &mut result {
                *key = key.saturating_sub(5);
            }
        }
        Mood::Awakening => {
            for key in &mut result {
                *key = key.saturating_add((bar % 4) as u8);
            }
        }
        Mood::FinalCharge => {
            for key in &mut result {
                *key = key.saturating_add(12);
            }
        }
        Mood::Fracture => result.truncate(2),
        _ => {}
    }
    result
}

fn key_root(key: &str) -> u8 {
    let key = key.trim().to_ascii_uppercase();
    let base = match key.as_bytes().first().copied().unwrap_or(b'E') {
        b'C' => 48,
        b'D' => 50,
        b'E' => 52,
        b'F' => 53,
        b'G' => 55,
        b'A' => 57,
        b'B' => 59,
        _ => 52,
    };
    if key.as_bytes().get(1) == Some(&b'#') {
        base + 1
    } else if key.as_bytes().get(1) == Some(&b'B') {
        base - 1
    } else {
        base
    }
}

fn parse_note_name(value: &str) -> Option<u8> {
    let bytes = value.as_bytes();
    let letter = *bytes.first()? as char;
    let base = match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let (accidental, octave_offset) = match bytes.get(1).copied() {
        Some(b'#') => (1, 2),
        Some(b'b') => (-1, 2),
        _ => (0, 1),
    };
    let octave = value.get(octave_offset..)?.parse::<i16>().ok()?;
    Some((12 * (octave + 1) + base + accidental).clamp(0, 127) as u8)
}

pub fn write_midi(score: &Score, path: &Path) -> Result<()> {
    let mut tracks = Vec::new();
    let mut conductor = vec![TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Muse Tempo")),
    }];
    let mut last = 0u64;
    for event in &score.tempo_events {
        let delta = event.start_tick.saturating_sub(last).min(u64::from(u32::MAX)) as u32;
        let micros = (60_000_000.0 / event.bpm).round().clamp(1.0, 16_777_215.0) as u32;
        conductor.push(TrackEvent {
            delta: u28::new(delta),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(micros))),
        });
        last = event.start_tick;
    }
    conductor.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::TimeSignature(
            score.meter.beats,
            score.meter.beat_unit.ilog2() as u8,
            24,
            8,
        )),
    });
    conductor.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    tracks.push(conductor);
    for (track_index, track) in score.tracks.iter().enumerate() {
        let mut raw = Vec::<(u64, bool, u8, u8)>::new();
        for note in score.notes.iter().filter(|note| note.track == track_index) {
            raw.push((note.start_tick, true, note.key, note.velocity));
            raw.push((note.start_tick + note.duration_ticks, false, note.key, 0));
        }
        raw.sort_by_key(|event| (event.0, event.1));
        let mut events = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(track.name.as_bytes())),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(track.channel),
                    message: MidiMessage::ProgramChange {
                        program: u7::new(track.program),
                    },
                },
            },
        ];
        let mut tick = 0u64;
        for (at, on, key, velocity) in raw {
            let delta = at.saturating_sub(tick).min(u64::from(u32::MAX)) as u32;
            let message = if on {
                MidiMessage::NoteOn {
                    key: u7::new(key),
                    vel: u7::new(velocity),
                }
            } else {
                MidiMessage::NoteOff {
                    key: u7::new(key),
                    vel: u7::new(0),
                }
            };
            events.push(TrackEvent {
                delta: u28::new(delta),
                kind: TrackEventKind::Midi {
                    channel: u4::new(track.channel),
                    message,
                },
            });
            tick = at;
        }
        events.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        tracks.push(events);
    }
    let smf = Smf {
        header: Header {
            format: Format::Parallel,
            timing: Timing::Metrical(u15::new(score.ticks_per_quarter.min(u32::from(u16::MAX)) as u16)),
        },
        tracks,
    };
    let mut output = BufWriter::new(File::create(path)?);
    smf.write_std(&mut output)?;
    Ok(())
}

pub fn write_musicxml(score: &Score, path: &Path) -> Result<()> {
    let divisions = score.ticks_per_quarter;
    let bar_ticks = u64::from(score.meter.beats) * u64::from(divisions) * 4 / u64::from(score.meter.beat_unit);
    let total_ticks = score
        .notes
        .iter()
        .map(|note| note.start_tick + note.duration_ticks)
        .max()
        .unwrap_or(bar_ticks);
    let bars = total_ticks.div_ceil(bar_ticks);
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n");
    xml.push_str(&format!(
        "<work><work-title>{}</work-title></work>\n",
        xml_escape(score.title.as_deref().unwrap_or("Muse composition"))
    ));
    xml.push_str("<part-list>\n");
    for (index, track) in score.tracks.iter().enumerate() {
        xml.push_str(&format!(
            "<score-part id=\"P{}\"><part-name>{}</part-name></score-part>\n",
            index + 1,
            xml_escape(&track.name)
        ));
    }
    xml.push_str("</part-list>\n");
    for (index, _track) in score.tracks.iter().enumerate() {
        xml.push_str(&format!("<part id=\"P{}\">\n", index + 1));
        for bar in 0..bars {
            let begin = bar * bar_ticks;
            let end = begin + bar_ticks;
            xml.push_str(&format!("<measure number=\"{}\">", bar + 1));
            if bar == 0 {
                xml.push_str(&format!("<attributes><divisions>{}</divisions><time><beats>{}</beats><beat-type>{}</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{:.0}</per-minute></metronome></direction-type><sound tempo=\"{:.2}\"/></direction>", divisions, score.meter.beats, score.meter.beat_unit, score.tempo_bpm, score.tempo_bpm));
            }
            let mut groups = BTreeMap::<u64, Vec<&Note>>::new();
            for note in score
                .notes
                .iter()
                .filter(|note| note.track == index && note.start_tick >= begin && note.start_tick < end)
            {
                groups.entry(note.start_tick).or_default().push(note);
            }
            let mut cursor = begin;
            for (start, notes) in groups {
                if start > cursor {
                    xml.push_str(&xml_rest(start - cursor));
                }
                for (note_index, note) in notes.iter().enumerate() {
                    xml.push_str(&xml_note(note, note_index > 0));
                }
                cursor = cursor.max(start + notes.iter().map(|note| note.duration_ticks).max().unwrap_or(0));
            }
            if cursor < end {
                xml.push_str(&xml_rest(end - cursor));
            }
            xml.push_str("</measure>\n");
        }
        xml.push_str("</part>\n");
    }
    xml.push_str("</score-partwise>\n");
    fs::write(path, xml)?;
    Ok(())
}

fn xml_note(note: &Note, chord: bool) -> String {
    let (step, alter, octave) = note_name(note.key);
    format!(
        "<note>{}<pitch><step>{}</step>{}<octave>{}</octave></pitch><duration>{}</duration><velocity>{}</velocity><notations><technical><other-technical>{}</other-technical></technical></notations></note>",
        if chord { "<chord/>" } else { "" },
        step,
        alter
            .map(|value| format!("<alter>{}</alter>", value))
            .unwrap_or_default(),
        octave,
        note.duration_ticks,
        note.velocity,
        xml_escape(&note.articulation)
    )
}

fn xml_rest(duration: u64) -> String {
    format!("<note><rest/><duration>{duration}</duration></note>")
}

fn note_name(key: u8) -> (&'static str, Option<i8>, u8) {
    const NAMES: [(&str, Option<i8>); 12] = [
        ("C", None),
        ("C", Some(1)),
        ("D", None),
        ("E", Some(-1)),
        ("E", None),
        ("F", None),
        ("F", Some(1)),
        ("G", None),
        ("A", Some(-1)),
        ("A", None),
        ("B", Some(-1)),
        ("B", None),
    ];
    let (step, alter) = NAMES[(key % 12) as usize];
    (step, alter, key / 12 - 1)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn write_pdf(score: &Score, path: &Path) -> Result<()> {
    let width = 842.0f32;
    let height = 595.0f32;
    let mut content = format!(
        "BT /F1 18 Tf 48 560 Td ({}) Tj ET\nBT /F1 10 Tf 48 542 Td ({} - {} BPM) Tj ET\n",
        pdf_escape(score.title.as_deref().unwrap_or("Muse composition")),
        pdf_escape(&score.key),
        score.tempo_bpm.round()
    );
    let bar_ticks =
        u64::from(score.meter.beats) * u64::from(score.ticks_per_quarter) * 4 / u64::from(score.meter.beat_unit);
    for (track_index, track) in score.tracks.iter().enumerate() {
        let base = 480.0 - track_index as f32 * 112.0;
        content.push_str(&format!(
            "BT /F1 10 Tf 48 {} Td ({}) Tj ET\n",
            base + 30.0,
            pdf_escape(&track.name)
        ));
        for line in 0..5 {
            let y = base + line as f32 * 8.0;
            content.push_str(&format!("48 {y:.1} m 790 {y:.1} l S\n"));
        }
        for bar in 0..9 {
            let x = 100.0 + bar as f32 * 84.0;
            content.push_str(&format!("{x:.1} {base:.1} m {x:.1} {:.1} l S\n", base + 32.0));
        }
        for note in score
            .notes
            .iter()
            .filter(|note| note.track == track_index && note.start_tick < bar_ticks * 8)
        {
            let x = 100.0 + note.start_tick as f32 / bar_ticks as f32 * 672.0;
            let y = base + 12.0 + (note.key as i16 - 60) as f32 * 2.0;
            content.push_str(&format!(
                "{x:.1} {y:.1} 7 5 re f\n{:.1} {:.1} m {:.1} {:.1} l S\n",
                x + 6.0,
                y + 2.0,
                x + 6.0,
                y + 27.0
            ));
        }
    }
    let stream = content.into_bytes();
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            String::from_utf8_lossy(&stream)
        ),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(path, pdf)?;
    Ok(())
}

fn pdf_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .chars()
        .filter(|ch| ch.is_ascii())
        .collect()
}

pub fn write_wav(score: &Score, path: &Path) -> Result<()> {
    let pcm = render_audio(score)?;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in pcm {
        writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

pub fn write_mp3(score: &Score, path: &Path) -> Result<()> {
    let pcm = render_audio(score)?;
    let mut encoder = Mp3Encoder::new(Mp3EncoderConfig {
        bitrate_kbps: 192,
        vbr_quality: None,
    });
    encoder.push_pcm_f32(&pcm, 2, SAMPLE_RATE)?;
    encoder.finish();
    let mut bytes = Vec::new();
    while let Ok(packet) = encoder.next_packet() {
        bytes.extend_from_slice(&packet);
    }
    fs::write(path, bytes)?;
    Ok(())
}

pub fn mp3_is_decodable(path: &Path) -> Result<bool> {
    let mut decoder = Mp3Decoder::new();
    decoder.push(&fs::read(path)?);
    decoder.flush();
    Ok(decoder.next_frame().is_ok())
}

fn render_audio(score: &Score) -> Result<Vec<f32>> {
    if let Some(path) = score.soundfont.as_deref().filter(|path| Path::new(path).exists()) {
        return render_soundfont(score, Path::new(path));
    }
    let seconds = score_end_seconds(score) + 1.5;
    let frames = (seconds * SAMPLE_RATE as f64).ceil() as usize;
    let mut pcm = vec![0.0f32; frames * 2];
    for note in &score.notes {
        let start = (tick_seconds(score, note.start_tick) * SAMPLE_RATE as f64) as usize;
        let end = (tick_seconds(score, note.start_tick + note.duration_ticks) * SAMPLE_RATE as f64) as usize;
        let frequency = 440.0 * 2.0f32.powf((note.key as f32 - 69.0) / 12.0);
        let pan = [-0.35, -0.05, 0.32, 0.0][note.track.min(3)];
        let gain = note.velocity as f32 / 127.0 * 0.16;
        for frame in start.min(frames)..end.min(frames) {
            let age = (frame - start) as f32 / SAMPLE_RATE as f32;
            let length = ((end.saturating_sub(start)).max(1)) as f32 / SAMPLE_RATE as f32;
            let envelope = if age < 0.015 {
                age / 0.015
            } else {
                ((length - age) / 0.10).clamp(0.0, 1.0).max(0.22)
            };
            let phase = TAU * frequency * age;
            let sample = synth_voice(note.track, phase, frame as u32) * envelope * gain;
            pcm[frame * 2] += sample * (1.0 - pan) * 0.5;
            pcm[frame * 2 + 1] += sample * (1.0 + pan) * 0.5;
        }
    }
    let peak = pcm.iter().fold(0.0f32, |value, sample| value.max(sample.abs()));
    if peak > 0.9 {
        for sample in &mut pcm {
            *sample *= 0.9 / peak;
        }
    }
    Ok(pcm)
}

fn synth_voice(track: usize, phase: f32, seed: u32) -> f32 {
    match track {
        0 => phase.sin() * 0.8 + (phase * 2.01).sin() * 0.22 + (phase * 3.0).sin() * 0.08,
        1 => (phase.sin() * 0.5 + (phase * 0.997).sin() * 0.25 + (phase * 1.004).sin() * 0.25) * 0.75,
        2 => {
            if phase.sin() >= 0.0 {
                0.65
            } else {
                -0.65
            }
        }
        _ => {
            let noise = ((seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223) >> 16) as f32 / 65535.0) * 2.0 - 1.0;
            noise * 0.45 + (phase * 0.25).sin() * 0.55
        }
    }
}

fn render_soundfont(score: &Score, path: &Path) -> Result<Vec<f32>> {
    let mut file = File::open(path).with_context(|| format!("cannot open SoundFont {}", path.display()))?;
    let font = Arc::new(SoundFont::new(&mut file)?);
    let settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
    let mut synth = Synthesizer::new(&font, &settings)?;
    for track in &score.tracks {
        synth.process_midi_message(track.channel as i32, 0xC0, track.program as i32, 0);
    }
    let frames = ((score_end_seconds(score) + 1.5) * SAMPLE_RATE as f64).ceil() as usize;
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    let mut events = Vec::<(usize, bool, &Note)>::new();
    for note in &score.notes {
        events.push((
            (tick_seconds(score, note.start_tick) * SAMPLE_RATE as f64) as usize,
            true,
            note,
        ));
        events.push((
            (tick_seconds(score, note.start_tick + note.duration_ticks) * SAMPLE_RATE as f64) as usize,
            false,
            note,
        ));
    }
    events.sort_by_key(|event| (event.0, event.1));
    let mut cursor = 0usize;
    for (at, on, note) in events {
        let at = at.min(frames);
        if at > cursor {
            synth.render(&mut left[cursor..at], &mut right[cursor..at]);
            cursor = at;
        }
        if on {
            synth.note_on(note.channel as i32, note.key as i32, note.velocity as i32);
        } else {
            synth.note_off(note.channel as i32, note.key as i32);
        }
    }
    if cursor < frames {
        synth.render(&mut left[cursor..], &mut right[cursor..]);
    }
    Ok(left.into_iter().zip(right).flat_map(|(l, r)| [l, r]).collect())
}

fn tick_seconds(score: &Score, target: u64) -> f64 {
    let mut seconds = 0.0;
    let mut tick = 0u64;
    let mut bpm = score.tempo_events.first().map(|event| event.bpm).unwrap_or(120.0);
    for event in score.tempo_events.iter().skip(1) {
        if event.start_tick >= target {
            break;
        }
        seconds += (event.start_tick - tick) as f64 * 60.0 / (bpm * score.ticks_per_quarter as f64);
        tick = event.start_tick;
        bpm = event.bpm;
    }
    seconds + target.saturating_sub(tick) as f64 * 60.0 / (bpm * score.ticks_per_quarter as f64)
}

fn score_end_seconds(score: &Score) -> f64 {
    tick_seconds(
        score,
        score
            .notes
            .iter()
            .map(|note| note.start_tick + note.duration_ticks)
            .max()
            .unwrap_or(0),
    )
}

/// Read a MIDI or MusicXML score for the existing `muse score info` command.
pub fn load_score(path: &Path) -> Result<Score> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mid" | "midi" => load_midi(path),
        "xml" | "musicxml" => load_musicxml(path),
        other => bail!("unsupported score format `{other}`; expected .mid, .midi, .xml, or .musicxml"),
    }
}

fn load_midi(path: &Path) -> Result<Score> {
    let bytes = fs::read(path)?;
    let smf = Smf::parse(&bytes)?;
    let ticks_per_quarter = match smf.header.timing {
        Timing::Metrical(value) => value.as_int() as u32,
        _ => TICKS_PER_QUARTER,
    };
    let mut score = Score {
        ticks_per_quarter,
        tempo_events: Vec::new(),
        ..Score::default()
    };
    let mut active = BTreeMap::<(u8, u8), (u64, u8)>::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += u64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(value)) => score.tempo_events.push(TempoEvent {
                    start_tick: tick,
                    bpm: 60_000_000.0 / value.as_int() as f64,
                }),
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel },
                } if vel.as_int() > 0 => {
                    active.insert((channel.as_int(), key.as_int()), (tick, vel.as_int()));
                }
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOff { key, .. },
                }
                | TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, .. },
                } => {
                    if let Some((start, velocity)) = active.remove(&(channel.as_int(), key.as_int())) {
                        score.notes.push(Note {
                            track: 0,
                            channel: channel.as_int(),
                            key: key.as_int(),
                            velocity,
                            start_tick: start,
                            duration_ticks: tick.saturating_sub(start),
                            articulation: "imported".into(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    score.tempo_events.sort_by_key(|event| event.start_tick);
    score.tempo_events.dedup_by_key(|event| event.start_tick);
    score.tempo_bpm = score.tempo_events.first().map(|event| event.bpm).unwrap_or(120.0);
    Ok(score)
}

fn load_musicxml(path: &Path) -> Result<Score> {
    use quick_xml::{Reader, events::Event};
    let xml = fs::read_to_string(path).with_context(|| format!("cannot read MusicXML {}", path.display()))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut score = Score {
        tracks: vec![Track {
            name: "Imported score".into(),
            channel: 0,
            program: 0,
            role: "imported".into(),
        }],
        ..Score::default()
    };
    let mut current = String::new();
    let mut step = None;
    let mut octave: Option<u8> = None;
    let mut duration = 1u64;
    let mut divisions = TICKS_PER_QUARTER;
    let mut cursor = 0u64;
    let mut rest = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                current = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                if current == "rest" {
                    rest = true;
                }
            }
            Ok(Event::Text(text)) => {
                let value = text.unescape().unwrap_or_default();
                match current.as_str() {
                    "work-title" | "movement-title" => score.title = Some(value.into_owned()),
                    "divisions" => divisions = value.parse().unwrap_or(TICKS_PER_QUARTER),
                    "step" => step = value.chars().next(),
                    "octave" => octave = value.parse().ok(),
                    "duration" => duration = value.parse().unwrap_or(1),
                    _ => {}
                }
            }
            Ok(Event::End(tag)) if tag.name().as_ref() == b"note" => {
                if !rest {
                    if let (Some(step), Some(octave)) = (step.take(), octave.take()) {
                        let pitch_class = match step {
                            'C' => 0,
                            'D' => 2,
                            'E' => 4,
                            'F' => 5,
                            'G' => 7,
                            'A' => 9,
                            'B' => 11,
                            _ => 0,
                        };
                        score.notes.push(Note {
                            track: 0,
                            channel: 0,
                            key: 12 * (octave + 1) + pitch_class,
                            velocity: 96,
                            start_tick: cursor,
                            duration_ticks: duration * u64::from(score.ticks_per_quarter) / u64::from(divisions.max(1)),
                            articulation: "imported".into(),
                        });
                    }
                } else {
                    rest = false;
                    step = None;
                    octave = None;
                }
                cursor += duration * u64::from(score.ticks_per_quarter) / u64::from(divisions.max(1));
                duration = 1;
            }
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid MusicXML: {error}"),
            _ => {}
        }
    }
    score.ticks_per_quarter = divisions;
    Ok(score)
}

pub fn audio_sample_rate(path: &Path) -> Result<u32> {
    use symphonia::core::{
        codecs::CodecParameters,
        formats::{FormatOptions, TrackType, probe::Hint},
        io::MediaSourceStream,
        meta::MetadataOptions,
    };
    let file = File::open(path)?;
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().probe(
        &hint,
        MediaSourceStream::new(Box::new(file), Default::default()),
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = probed
        .default_track(TrackType::Audio)
        .context("audio track not found")?;
    if let Some(CodecParameters::Audio(parameters)) = &track.codec_params {
        return parameters.sample_rate.context("audio sample rate missing");
    }
    bail!("selected track is not audio")
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCRIPT: &str = r#"
# 中文注释：同一输入必须得到同一首曲子
namespace battle;
timeline { act assault : 0..2 act lament : 2..4 act final_charge : 4..6 }
tempo { at 0 bpm 156 at 2 bpm 76 at 4 bpm 168 }
intent { emotion "violent urgency" for assault emotion "grief" for lament emotion "triumph" for final_charge }
music for assault { key "E minor" meter 4/4 motif "E4 G4 F#4 E4" template "game_battle" }
music for lament { key "E minor" meter 4/4 motif "E4 G4 F#4 E4" }
music for final_charge { key "E major" meter 4/4 motif "E4 G4 F#4 E4" }
"#;
    #[test]
    fn composition_is_deterministic() {
        assert_eq!(compose(SCRIPT).unwrap(), compose(SCRIPT).unwrap());
    }
    #[test]
    fn exports_are_nonempty() {
        let score = compose(SCRIPT).unwrap();
        let dir = std::env::temp_dir();
        let midi = dir.join("muse-test.mid");
        let xml = dir.join("muse-test.musicxml");
        let pdf = dir.join("muse-test.pdf");
        let wav = dir.join("muse-test.wav");
        let mp3 = dir.join("muse-test.mp3");
        write_midi(&score, &midi).unwrap();
        write_musicxml(&score, &xml).unwrap();
        write_pdf(&score, &pdf).unwrap();
        write_wav(&score, &wav).unwrap();
        write_mp3(&score, &mp3).unwrap();
        for file in [&midi, &xml, &pdf, &wav, &mp3] {
            assert!(fs::metadata(file).unwrap().len() > 16);
        }
        assert!(mp3_is_decodable(&mp3).unwrap());
    }
}
