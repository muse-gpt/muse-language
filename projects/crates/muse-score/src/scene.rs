use crate::{SemanticScore, justify_columns, normalize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        filled: bool,
    },
    Text {
        x: f32,
        y: f32,
        size: f32,
        value: String,
    },
    Rest {
        x: f32,
        y: f32,
        duration: crate::DurationValue,
    },
    Flag {
        x: f32,
        y: f32,
        up: bool,
        levels: u8,
    },
    Tie {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        up: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentScene {
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone)]
struct SystemGeometry {
    top: f32,
    height: f32,
    staff_offsets: Vec<f32>,
}

fn voice_extent(measures: &[crate::Measure], voice_index: usize) -> (f32, f32) {
    let mut top: f32 = -8.0;
    // Dynamics, lower accidentals, and below-staff articulations occupy this band.
    let mut bottom: f32 = 70.0;
    for measure in measures {
        let voice = &measure.voices[voice_index];
        if voice.name.eq_ignore_ascii_case("percussion") {
            top = top.min(-16.0);
            continue;
        }
        for column in &voice.columns {
            if column.rest {
                continue;
            }
            let average = column.notes.iter().map(|note| note.key as f32).sum::<f32>() / column.notes.len() as f32;
            for note in &column.notes {
                let y = 24.0 - (note.key as f32 - 60.0) * 2.0;
                if average < 64.0 {
                    top = top.min(y - 30.0);
                } else {
                    bottom = bottom.max(y + 30.0);
                }
                top = top.min(y - 5.0);
                bottom = bottom.max(y + 5.0);
            }
        }
    }
    (top, bottom)
}

fn system_geometries(score: &SemanticScore) -> Vec<SystemGeometry> {
    let mut top = 84.0;
    score
        .measures
        .chunks(4)
        .map(|measures| {
            let extents = (0..measures[0].voices.len())
                .map(|voice| voice_extent(measures, voice))
                .collect::<Vec<_>>();
            let mut offsets = Vec::with_capacity(extents.len());
            let mut cursor = 0.0;
            for (index, (voice_top, _)) in extents.iter().copied().enumerate() {
                if index > 0 {
                    cursor += extents[index - 1].1 - voice_top + 20.0;
                }
                offsets.push(cursor);
            }
            let height = offsets
                .last()
                .zip(extents.last())
                .map_or(120.0, |(offset, (_, bottom))| offset + bottom + 62.0)
                .max(120.0);
            let result = SystemGeometry {
                top,
                height,
                staff_offsets: offsets,
            };
            top += height + 34.0;
            result
        })
        .collect()
}

fn key_signature(key: &str) -> (char, usize) {
    let key = key.trim().to_ascii_lowercase();
    let sharp = [
        ("c major", 0),
        ("a minor", 0),
        ("g major", 1),
        ("e minor", 1),
        ("d major", 2),
        ("b minor", 2),
        ("a major", 3),
        ("f# minor", 3),
        ("e major", 4),
        ("c# minor", 4),
        ("b major", 5),
        ("g# minor", 5),
        ("f# major", 6),
        ("d# minor", 6),
        ("c# major", 7),
        ("a# minor", 7),
    ];
    if let Some((_, count)) = sharp.iter().find(|(name, _)| *name == key) {
        return (crate::music_font::SHARP, *count);
    }
    let flat = [
        ("f major", 1),
        ("d minor", 1),
        ("bb major", 2),
        ("g minor", 2),
        ("eb major", 3),
        ("c minor", 3),
        ("ab major", 4),
        ("f minor", 4),
        ("db major", 5),
        ("bb minor", 5),
        ("gb major", 6),
        ("eb minor", 6),
        ("cb major", 7),
        ("ab minor", 7),
    ];
    flat.iter()
        .find(|(name, _)| *name == key)
        .map_or((crate::music_font::SHARP, 0), |(_, count)| {
            (crate::music_font::FLAT, *count)
        })
}

fn implied_sharp_pitch_classes(key: &str) -> BTreeSet<u8> {
    let (symbol, count) = key_signature(key);
    if symbol != crate::music_font::SHARP {
        return BTreeSet::new();
    }
    [6, 1, 8, 3, 10, 5, 0].into_iter().take(count).collect()
}

fn needs_sharp_accidental(key: u8, implied: &BTreeSet<u8>, seen: &mut BTreeSet<u8>) -> bool {
    let pitch_class = key % 12;
    matches!(pitch_class, 1 | 3 | 6 | 8 | 10) && !implied.contains(&pitch_class) && seen.insert(key)
}

pub fn layout(score: &muse_music::Score) -> Scene {
    layout_semantic(&normalize(score))
}

pub fn layout_document(score: &muse_music::Score) -> DocumentScene {
    layout_document_semantic(&normalize(score))
}

pub fn layout_document_semantic(score: &SemanticScore) -> DocumentScene {
    const PAGE_WIDTH: f32 = 1200.0;
    let full = layout_semantic(score);
    let geometries = system_geometries(score);
    let mut pages = Vec::new();
    for (page_index, geometry) in geometries.iter().enumerate() {
        let source_y = geometry.top;
        let source_end = source_y + geometry.height;
        let mut primitives = Vec::new();
        for primitive in &full.primitives {
            match primitive {
                Primitive::Text { x, y, size, value } if *y < 75.0 => {
                    if page_index == 0 {
                        primitives.push(Primitive::Text {
                            x: *x,
                            y: *y,
                            size: *size,
                            value: value.clone(),
                        });
                    }
                }
                Primitive::Line { x1, y1, x2, y2, width }
                    if *y1 >= source_y && *y1 < source_end && *y2 >= source_y && *y2 < source_end =>
                {
                    primitives.push(Primitive::Line {
                        x1: *x1,
                        y1: *y1 - source_y + 92.0,
                        x2: *x2,
                        y2: *y2 - source_y + 92.0,
                        width: *width,
                    })
                }
                Primitive::Ellipse { cx, cy, rx, ry, filled } if *cy >= source_y - 20.0 && *cy < source_end => {
                    primitives.push(Primitive::Ellipse {
                        cx: *cx,
                        cy: *cy - source_y + 92.0,
                        rx: *rx,
                        ry: *ry,
                        filled: *filled,
                    })
                }
                Primitive::Rest { x, y, duration } if *y >= source_y && *y < source_end => {
                    primitives.push(Primitive::Rest {
                        x: *x,
                        y: *y - source_y + 92.0,
                        duration: *duration,
                    })
                }
                Primitive::Flag { x, y, up, levels } if *y >= source_y - 20.0 && *y < source_end => {
                    primitives.push(Primitive::Flag {
                        x: *x,
                        y: *y - source_y + 92.0,
                        up: *up,
                        levels: *levels,
                    })
                }
                Primitive::Text { x, y, size, value } if *y >= source_y && *y < source_end => {
                    primitives.push(Primitive::Text {
                        x: *x,
                        y: *y - source_y + 92.0,
                        size: *size,
                        value: value.clone(),
                    })
                }
                _ => {}
            }
        }
        pages.push(Page {
            width: PAGE_WIDTH,
            height: geometry.height + 146.0,
            primitives,
        });
    }
    DocumentScene { pages }
}

pub fn layout_semantic(score: &SemanticScore) -> Scene {
    const STAFF_LEFT: f32 = 180.0;
    const STAFF_RIGHT: f32 = 1100.0;
    const BAR_WIDTH: f32 = (STAFF_RIGHT - STAFF_LEFT) / 4.0;
    let geometries = system_geometries(score);
    let mut p = vec![
        Primitive::Text {
            x: 48.0,
            y: 38.0,
            size: 28.0,
            value: score.title.clone(),
        },
        Primitive::Text {
            x: 48.0,
            y: 64.0,
            size: 15.0,
            value: format!("{} - 120 BPM", score.key),
        },
    ];
    for (measures, geometry) in score.measures.chunks(4).zip(geometries.iter()) {
        let top = geometry.top;
        for voice_idx in 0..score.measures[0].voices.len() {
            let staff = top + geometry.staff_offsets[voice_idx];
            for line in 0..5 {
                let y = staff + line as f32 * 8.0;
                p.push(Primitive::Line {
                    x1: STAFF_LEFT,
                    y1: y,
                    x2: STAFF_RIGHT,
                    y2: y,
                    width: 1.2,
                });
            }
            p.push(Primitive::Text {
                x: 48.0,
                y: staff + 12.0,
                size: 14.0,
                value: score.measures[0].voices[voice_idx].name.clone(),
            });
            p.push(Primitive::Text {
                x: 112.0,
                y: staff + 27.0,
                size: 26.0,
                value: if score.measures[0].voices[voice_idx]
                    .name
                    .eq_ignore_ascii_case("percussion")
                {
                    crate::music_font::PERCUSSION_CLEF.into()
                } else if matches!(score.measures[0].voices[voice_idx].role.as_str(), "bass" | "low") {
                    crate::music_font::BASS_CLEF.into()
                } else {
                    crate::music_font::TREBLE_CLEF.into()
                },
            });
            if voice_idx == 0 {
                p.push(Primitive::Text {
                    x: 132.0,
                    y: staff + 15.0,
                    size: 11.0,
                    value: format!("{}/{}", score.meter.beats, score.meter.beat_unit),
                });
            }
            if !score.measures[0].voices[voice_idx]
                .name
                .eq_ignore_ascii_case("percussion")
            {
                let (accidental, count) = key_signature(&score.key);
                let sharp_positions = [0.0, 12.0, 4.0, 16.0, 8.0, 20.0, 12.0];
                let flat_positions = [16.0, 8.0, 20.0, 12.0, 24.0, 16.0, 28.0];
                let positions = if accidental == crate::music_font::SHARP {
                    &sharp_positions
                } else {
                    &flat_positions
                };
                for (index, y) in positions.iter().take(count).enumerate() {
                    p.push(Primitive::Text {
                        x: 158.0 + index as f32 * 7.0,
                        y: staff + y + 7.0,
                        size: 15.0,
                        value: accidental.into(),
                    });
                }
            }
            for (mi, measure) in measures.iter().enumerate() {
                let left = STAFF_LEFT + mi as f32 * BAR_WIDTH;
                p.push(Primitive::Line {
                    x1: left,
                    y1: staff,
                    x2: left,
                    y2: staff + 32.0,
                    width: 1.0,
                });
                let voice = &measure.voices[voice_idx];
                let percussion = voice.name.eq_ignore_ascii_case("percussion");
                let implied_sharps = implied_sharp_pitch_classes(&score.key);
                let mut shown_accidentals = BTreeSet::new();
                if mi == 0 {
                    let velocities = voice
                        .columns
                        .iter()
                        .flat_map(|column| column.notes.iter().map(|note| note.velocity))
                        .collect::<Vec<_>>();
                    if let Some(dynamic) = dynamic_mark(&velocities) {
                        p.push(Primitive::Text {
                            x: left + 10.0,
                            y: staff + 66.0,
                            size: 13.0,
                            value: dynamic.into(),
                        });
                    }
                }
                let mut beam_group = Vec::<(f32, f32, u8, u64, bool)>::new();
                let positions = justify_columns(&voice.columns, left, BAR_WIDTH);
                for (column_index, col) in voice.columns.iter().enumerate() {
                    if col.rest {
                        let x = positions[column_index];
                        p.push(Primitive::Rest {
                            x,
                            y: staff + 18.0,
                            duration: col.duration,
                        });
                        draw_beam_group(&mut p, &beam_group);
                        beam_group.clear();
                        continue;
                    }
                    let x = positions[column_index];
                    let stem_up = percussion
                        || col.notes.iter().map(|n| n.key as f32).sum::<f32>() / (col.notes.len() as f32) < 64.0;
                    let mut edge = if stem_up { staff + 24.0 } else { staff - 20.0 };
                    for note in &col.notes {
                        let y = if percussion {
                            staff + 16.0
                        } else {
                            staff + 24.0 - (note.key as f32 - 60.0) * 2.0
                        };
                        if !percussion && needs_sharp_accidental(note.key, &implied_sharps, &mut shown_accidentals) {
                            p.push(Primitive::Text {
                                x: x - 13.0,
                                y: y + 5.0,
                                size: 15.0,
                                value: crate::music_font::SHARP.into(),
                            });
                        }
                        if !percussion && (y < staff - 4.0 || y > staff + 36.0) {
                            let mut ledger = if y < staff { staff - 8.0 } else { staff + 40.0 };
                            while (y < staff && ledger >= y - 2.0) || (y > staff && ledger <= y + 2.0) {
                                p.push(Primitive::Line {
                                    x1: x - 9.0,
                                    y1: ledger,
                                    x2: x + 9.0,
                                    y2: ledger,
                                    width: 1.0,
                                });
                                ledger += if y < staff { -8.0 } else { 8.0 };
                            }
                        }
                        p.push(Primitive::Ellipse {
                            cx: x,
                            cy: y,
                            rx: 5.5,
                            ry: 3.8,
                            filled: !matches!(col.duration, crate::DurationValue::Whole | crate::DurationValue::Half),
                        });
                        let tie_y = y + if stem_up { 7.0 } else { -7.0 };
                        if note.tie_to_next {
                            p.push(Primitive::Tie {
                                x1: x + 5.0,
                                y1: tie_y,
                                x2: left + BAR_WIDTH - 4.0,
                                y2: tie_y,
                                up: stem_up,
                            });
                        }
                        if note.tie_from_previous {
                            p.push(Primitive::Tie {
                                x1: left + 4.0,
                                y1: tie_y,
                                x2: x - 5.0,
                                y2: tie_y,
                                up: stem_up,
                            });
                        }
                        if stem_up {
                            edge = edge.min(y);
                        } else {
                            edge = edge.max(y);
                        }
                    }
                    if col.notes.iter().any(|note| note.articulation == "accent") {
                        p.push(Primitive::Text {
                            x: x - 3.0,
                            y: if stem_up { staff + 66.0 } else { staff - 8.0 },
                            size: 14.0,
                            value: ">".into(),
                        });
                    }
                    let stem_end = if stem_up { edge - 30.0 } else { edge + 30.0 };
                    p.push(Primitive::Line {
                        x1: x,
                        y1: edge,
                        x2: x,
                        y2: stem_end,
                        width: 1.0,
                    });
                    if col.duration.beam_count() > 0 {
                        let beat =
                            col.start_tick.saturating_sub(measure.start_tick) / u64::from(score.ticks_per_quarter);
                        if beam_group.first().is_some_and(|entry| entry.3 != beat) {
                            draw_beam_group(&mut p, &beam_group);
                            beam_group.clear();
                        }
                        beam_group.push((x, stem_end, col.duration.beam_count(), beat, stem_up));
                    } else {
                        draw_beam_group(&mut p, &beam_group);
                        beam_group.clear();
                    }
                }
                draw_beam_group(&mut p, &beam_group);
            }
            p.push(Primitive::Line {
                x1: STAFF_RIGHT,
                y1: staff,
                x2: STAFF_RIGHT,
                y2: staff + 32.0,
                width: 1.0,
            });
        }
    }
    Scene {
        width: 1200.0,
        height: geometries
            .last()
            .map_or(160.0, |geometry| geometry.top + geometry.height + 34.0),
        primitives: p,
    }
}

fn draw_beam_group(primitives: &mut Vec<Primitive>, group: &[(f32, f32, u8, u64, bool)]) {
    if group.len() == 1 {
        let (x, y, levels, _, up) = group[0];
        primitives.push(Primitive::Flag { x, y, up, levels });
        return;
    }
    if group.is_empty() {
        return;
    }
    let (first_x, first_y, _, _, up) = group[0];
    let (last_x, last_y, _, _, _) = group[group.len() - 1];
    let dx = (last_x - first_x).max(1.0);
    let target_last_y = first_y + (last_y - first_y).clamp(-dx * 0.2, dx * 0.2);
    for (x, stem_y, _, _, _) in group {
        let t = (*x - first_x) / dx;
        primitives.push(Primitive::Line {
            x1: *x,
            y1: *stem_y,
            x2: *x,
            y2: first_y + (target_last_y - first_y) * t,
            width: 1.0,
        });
    }
    let direction = if up { -1.0 } else { 1.0 };
    for level in 0..group.iter().map(|entry| entry.2).max().unwrap_or(0) {
        let mut start = 0;
        while start < group.len() {
            while start < group.len() && group[start].2 <= level {
                start += 1;
            }
            let end = (start..group.len())
                .find(|index| group[*index].2 <= level)
                .unwrap_or(group.len());
            if start == end {
                break;
            }
            let x1 = group[start].0;
            let x2 = group[end - 1].0;
            let y =
                |x: f32| first_y + (target_last_y - first_y) * ((x - first_x) / dx) + direction * level as f32 * 5.0;
            if start + 1 == end {
                let tail = if end < group.len() || start == 0 {
                    x1 + 13.0
                } else {
                    x1 - 13.0
                };
                primitives.push(Primitive::Line {
                    x1,
                    y1: y(x1),
                    x2: tail,
                    y2: y(tail),
                    width: 3.4,
                });
            } else {
                primitives.push(Primitive::Line {
                    x1,
                    y1: y(x1),
                    x2,
                    y2: y(x2),
                    width: 3.4,
                });
            }
            start = end;
        }
    }
}

fn dynamic_mark(velocities: &[u8]) -> Option<&'static str> {
    if velocities.is_empty() {
        return None;
    }
    let average =
        velocities.iter().map(|value| u32::from(*value)).sum::<u32>() / u32::try_from(velocities.len()).ok()?;
    Some(match average {
        0..=45 => "p",
        46..=69 => "mp",
        70..=94 => "mf",
        _ => "f",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_has_stable_canvas() {
        let scene = layout(&muse_music::Score::default());
        assert_eq!(scene.width, 1200.0);
        assert!(!scene.primitives.is_empty());
    }

    #[test]
    fn half_notes_use_open_noteheads() {
        let mut score = muse_music::Score::default();
        score.tracks.push(muse_music::Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "melody".into(),
        });
        score.notes.push(muse_music::Note {
            track: 0,
            channel: 0,
            key: 60,
            velocity: 90,
            start_tick: 0,
            duration_ticks: 960,
            articulation: String::new(),
        });
        assert!(
            layout(&score)
                .primitives
                .iter()
                .any(|primitive| matches!(primitive, Primitive::Ellipse { filled: false, .. }))
        );
    }

    #[test]
    fn high_notes_create_ledger_lines() {
        let mut score = muse_music::Score::default();
        score.tracks.push(muse_music::Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "melody".into(),
        });
        score.notes.push(muse_music::Note {
            track: 0,
            channel: 0,
            key: 84,
            velocity: 90,
            start_tick: 0,
            duration_ticks: 480,
            articulation: String::new(),
        });
        assert!(
            layout(&score)
                .primitives
                .iter()
                .any(|primitive| matches!(primitive, Primitive::Line { x1, x2, .. } if x2 - x1 > 10.0))
        );
    }

    #[test]
    fn beam_slope_is_clamped() {
        let mut primitives = Vec::new();
        draw_beam_group(&mut primitives, &[(0.0, 0.0, 2, 0, true), (100.0, 100.0, 2, 0, true)]);
        let Primitive::Line { x1, y1, x2, y2, .. } = primitives[0] else {
            panic!("expected beam line")
        };
        assert!((y2 - y1).abs() <= (x2 - x1).abs() * 0.2 + f32::EPSILON);
    }

    #[test]
    fn mixed_beam_levels_use_a_partial_beam() {
        let mut primitives = Vec::new();
        draw_beam_group(&mut primitives, &[(0.0, 30.0, 2, 0, true), (28.0, 26.0, 1, 0, true)]);
        assert!(primitives.iter().any(|primitive| matches!(primitive, Primitive::Line { x1, x2, width, .. } if (*x2 - *x1).abs() == 13.0 && *width == 3.4)));
        assert_eq!(
            primitives
                .iter()
                .filter(
                    |primitive| matches!(primitive, Primitive::Line { x1: 0.0, x2: 28.0, width, .. } if *width == 3.4)
                )
                .count(),
            1
        );
    }

    #[test]
    fn skyline_separates_the_next_staff_from_low_stems() {
        let mut score = muse_music::Score::default();
        score.tracks = vec![
            muse_music::Track {
                name: "Low brass".into(),
                channel: 0,
                program: 57,
                role: "bass".into(),
            },
            muse_music::Track {
                name: "Strings".into(),
                channel: 1,
                program: 48,
                role: "pad".into(),
            },
        ];
        score.notes.push(muse_music::Note {
            track: 0,
            channel: 0,
            key: 24,
            velocity: 100,
            start_tick: 0,
            duration_ticks: 480,
            articulation: String::new(),
        });
        let normalized = normalize(&score);
        let first = system_geometries(&normalized)[0].staff_offsets.clone();
        assert!(first[1] > 120.0);
    }

    #[test]
    fn recognizes_major_and_minor_key_signatures() {
        assert_eq!(key_signature("E minor"), (crate::music_font::SHARP, 1));
        assert_eq!(key_signature("Bb major"), (crate::music_font::FLAT, 2));
        assert_eq!(key_signature("C major"), (crate::music_font::SHARP, 0));
    }

    #[test]
    fn empty_voice_has_no_dynamic_and_can_be_exported() {
        assert_eq!(dynamic_mark(&[]), None);
        let score = muse_music::Score::default();
        assert!(!layout(&score).primitives.is_empty());
    }

    #[test]
    fn accidentals_respect_key_signature_and_reset_per_measure() {
        let implied = implied_sharp_pitch_classes("E minor");
        let mut seen = BTreeSet::new();
        assert!(
            !needs_sharp_accidental(66, &implied, &mut seen),
            "F# belongs to E minor"
        );
        assert!(needs_sharp_accidental(61, &implied, &mut seen), "C# needs a mark");
        assert!(
            !needs_sharp_accidental(61, &implied, &mut seen),
            "repeat in measure is suppressed"
        );
        let mut next_measure = BTreeSet::new();
        assert!(
            needs_sharp_accidental(61, &implied, &mut next_measure),
            "new measure resets accidental state"
        );
    }

    #[test]
    fn bass_role_selects_a_bass_clef() {
        let mut score = muse_music::Score::default();
        score.tracks.push(muse_music::Track {
            name: "Brass".into(),
            channel: 0,
            program: 57,
            role: "bass".into(),
        });
        assert!(layout(&score).primitives.iter().any(|primitive| matches!(
            primitive, Primitive::Text { value, .. } if value == &crate::music_font::BASS_CLEF.to_string()
        )));
    }

    #[test]
    fn sustained_note_emits_tie_primitives() {
        let mut score = muse_music::Score::default();
        score.tracks.push(muse_music::Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "motif".into(),
        });
        score.notes.push(muse_music::Note {
            track: 0,
            channel: 0,
            key: 60,
            velocity: 90,
            start_tick: 1_440,
            duration_ticks: 960,
            articulation: String::new(),
        });
        assert!(
            layout(&score)
                .primitives
                .iter()
                .any(|primitive| matches!(primitive, Primitive::Tie { .. }))
        );
    }
}
