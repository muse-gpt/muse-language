use crate::{SemanticScore, justify_columns, normalize};

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

pub fn layout(score: &muse_music::Score) -> Scene {
    layout_semantic(&normalize(score))
}

pub fn layout_document(score: &muse_music::Score) -> DocumentScene {
    layout_document_semantic(&normalize(score))
}

pub fn layout_document_semantic(score: &SemanticScore) -> DocumentScene {
    const PAGE_WIDTH: f32 = 1200.0;
    const PAGE_HEIGHT: f32 = 820.0;
    const SYSTEMS_PER_PAGE: usize = 2;
    let full = layout_semantic(score);
    let system_height = 430.0;
    let mut pages = Vec::new();
    let systems = score.measures.len().div_ceil(4);
    for page_index in 0..systems.div_ceil(SYSTEMS_PER_PAGE) {
        let system_start = page_index * SYSTEMS_PER_PAGE;
        let source_y = 84.0 + system_start as f32 * system_height;
        let source_end = source_y + SYSTEMS_PER_PAGE as f32 * system_height;
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
                Primitive::Ellipse { cx, cy, rx, ry, filled } if *cy >= source_y - 80.0 && *cy < source_end + 80.0 => {
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
                Primitive::Flag { x, y, up, levels } if *y >= source_y - 80.0 && *y < source_end + 80.0 => primitives
                    .push(Primitive::Flag {
                        x: *x,
                        y: *y - source_y + 92.0,
                        up: *up,
                        levels: *levels,
                    }),
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
            height: PAGE_HEIGHT,
            primitives,
        });
    }
    DocumentScene { pages }
}

pub fn layout_semantic(score: &SemanticScore) -> Scene {
    const STAFF_LEFT: f32 = 180.0;
    const STAFF_RIGHT: f32 = 1100.0;
    const BAR_WIDTH: f32 = (STAFF_RIGHT - STAFF_LEFT) / 4.0;
    let systems = score.measures.len().div_ceil(4);
    let staff_gap = 92.0;
    let system_height = 430.0;
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
    for (si, measures) in score.measures.chunks(4).enumerate() {
        let top = 84.0 + si as f32 * system_height;
        for voice_idx in 0..score.measures[0].voices.len() {
            let staff = top + voice_idx as f32 * staff_gap;
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
                    "P".into()
                } else {
                    "𝄞".into()
                },
            });
            if voice_idx == 0 {
                p.push(Primitive::Text {
                    x: 132.0,
                    y: staff + 15.0,
                    size: 11.0,
                    value: format!("{}/{}", score.meter.beats, score.meter.beat_unit),
                });
                p.push(Primitive::Text {
                    x: 132.0,
                    y: staff + 31.0,
                    size: 10.0,
                    value: score.key.clone(),
                });
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
                        if stem_up {
                            edge = edge.min(y);
                        } else {
                            edge = edge.max(y);
                        }
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
        height: 90.0 + systems as f32 * system_height,
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
    let (first_x, first_y, _, _, _) = group[0];
    let (last_x, last_y, _, _, _) = group[group.len() - 1];
    let dx = (last_x - first_x).max(1.0);
    let target_last_y = first_y + (last_y - first_y).clamp(-dx * 0.2, dx * 0.2);
    for level in 0..group.iter().map(|entry| entry.2).max().unwrap_or(0) {
        let offset = level as f32 * 5.0;
        primitives.push(Primitive::Line {
            x1: first_x,
            y1: first_y - offset,
            x2: last_x,
            y2: target_last_y - offset,
            width: 3.4,
        });
    }
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
}
