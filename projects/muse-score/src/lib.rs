//! Pure-Rust deterministic score engraving. Layout is shared by SVG, HTML and PDF.
use anyhow::Result;
use muse_music::{Note, Score};
use std::{fs, path::Path};

const WIDTH: f32 = 1200.0;
const SYSTEM_HEIGHT: f32 = 430.0;
const STAFF_GAP: f32 = 92.0;
const BAR_WIDTH: f32 = 235.0;

pub fn render_svg(score: &Score) -> String {
    let beats = u64::from(score.meter.beats.max(1));
    let bar_ticks = beats * u64::from(score.ticks_per_quarter) * 4 / u64::from(score.meter.beat_unit.max(1));
    let end_tick = score
        .notes
        .iter()
        .map(|n| n.start_tick + n.duration_ticks)
        .max()
        .unwrap_or(bar_ticks);
    let bars = ((end_tick + bar_ticks - 1) / bar_ticks).max(1);
    let systems = (bars + 3) / 4;
    let height = 90.0 + systems as f32 * SYSTEM_HEIGHT;
    let title = esc(score.title.as_deref().unwrap_or("Muse composition"));
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {height}" role="img" aria-label="{title}"><style>text{{font-family:serif;fill:#111}}.staff{{stroke:#222;stroke-width:1.2}}.bar{{stroke:#555;stroke-width:1}}.note{{fill:#111}}.beam{{stroke:#111;stroke-width:3}}</style><rect width="100%" height="100%" fill="white"/><text x="48" y="38" font-size="28">{title}</text><text x="48" y="64" font-size="15">{}</text>"#,
        esc(&format!("{} - {:.0} BPM - {bars} bars", score.key, score.tempo_bpm))
    );
    for system in 0..systems {
        let first_bar = system * 4;
        let top = 84.0 + system as f32 * SYSTEM_HEIGHT;
        out.push_str(&format!(
            r#"<g class="system"><text x="34" y="{}" font-size="11">system {}</text>"#,
            top + 22.0,
            system + 1
        ));
        for (track_i, track) in score.tracks.iter().enumerate() {
            let staff = top + track_i as f32 * STAFF_GAP;
            out.push_str(&format!(
                r#"<text x="48" y="{}" font-size="14">{}</text>"#,
                staff + 12.0,
                esc(&track.name)
            ));
            for line in 0..5 {
                let y = staff + line as f32 * 8.0;
                out.push_str(&format!(
                    r#"<line class="staff" x1="120" y1="{y:.1}" x2="1080" y2="{y:.1}"/>"#
                ));
            }
            for local in 0..=4 {
                let x = 120.0 + local as f32 * BAR_WIDTH;
                out.push_str(&format!(
                    r#"<line class="bar" x1="{x:.1}" y1="{staff:.1}" x2="{x:.1}" y2="{:.1}"/>"#,
                    staff + 32.0
                ));
            }
            let start = first_bar as u64 * bar_ticks;
            let stop = (first_bar as u64 + 4) * bar_ticks;
            let mut events: Vec<(u64, Vec<&Note>)> = Vec::new();
            for note in score
                .notes
                .iter()
                .filter(|n| n.track == track_i && n.start_tick >= start && n.start_tick < stop)
            {
                if let Some((_, notes)) = events.iter_mut().find(|(tick, _)| *tick == note.start_tick) {
                    notes.push(note);
                } else {
                    events.push((note.start_tick, vec![note]));
                }
            }
            for (tick, notes) in events {
                let x = 124.0 + (tick - start) as f32 / bar_ticks as f32 * BAR_WIDTH;
                let avg = notes.iter().map(|n| n.key as f32).sum::<f32>() / notes.len() as f32;
                let stem_up = avg < 64.0;
                let mut stem_y = if stem_up { staff + 24.0 } else { staff - 20.0 };
                for note in notes.iter() {
                    let y = staff + 24.0 - (note.key as f32 - 60.0) * 2.0;
                    out.push_str(&format!(
                        r#"<ellipse class="note" cx="{x:.1}" cy="{y:.1}" rx="5.5" ry="3.8"/>"#
                    ));
                    if stem_up {
                        stem_y = stem_y.min(y);
                    } else {
                        stem_y = stem_y.max(y);
                    }
                }
                let end = if stem_up { stem_y - 30.0 } else { stem_y + 30.0 };
                out.push_str(&format!(
                    r#"<line class="bar" x1="{x:.1}" y1="{stem_y:.1}" x2="{x:.1}" y2="{end:.1}"/>"#
                ));
            }
        }
        out.push_str("</g>");
    }
    out.push_str("</svg>");
    out
}

pub fn render_html(score: &Score) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Muse score</title><style>body{{margin:2rem;background:#eee}}svg{{display:block;margin:auto;background:#fff;max-width:1200px;width:100%;height:auto}}</style>{}",
        render_svg(score)
    )
}
pub fn write_svg(score: &Score, path: &Path) -> Result<()> {
    fs::write(path, render_svg(score))?;
    Ok(())
}
pub fn write_html(score: &Score, path: &Path) -> Result<()> {
    fs::write(path, render_html(score))?;
    Ok(())
}
pub fn write_pdf(score: &Score, path: &Path) -> Result<()> {
    muse_music::write_pdf(score, path)
}
fn esc(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
