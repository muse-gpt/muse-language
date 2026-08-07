//! Pure-Rust deterministic score engraving. Layout is shared by SVG, HTML and PDF.
use anyhow::Result;
use muse_music::Score;
use std::{fs, path::Path};

mod semantic;
pub use semantic::{DurationValue, Measure, NoteColumn, NoteRef, SemanticScore, VoiceMeasure, normalize};
mod scene;
pub use scene::{
    DocumentScene, Page, Primitive, Scene, layout, layout_document, layout_document_semantic, layout_semantic,
};
mod spacing;
pub use spacing::justify_columns;

pub fn render_svg(score: &Score) -> String {
    let scene = layout(score);
    let title = esc(score.title.as_deref().unwrap_or("Muse composition"));
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.1} {:.1}" role="img" aria-label="{title}"><style>text{{font-family:serif;fill:#111}}line{{stroke:#222;stroke-linecap:round}}ellipse{{fill:#111}}</style><rect width="100%" height="100%" fill="white"/>"#,
        scene.width, scene.height
    );
    for primitive in scene.primitives {
        match primitive {
            Primitive::Line { x1, y1, x2, y2, width } => out.push_str(&format!(
                r#"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke-width="{width:.1}"/>"#
            )),
            Primitive::Ellipse { cx, cy, rx, ry, filled } => out.push_str(&format!(
                r##"<ellipse cx="{cx:.1}" cy="{cy:.1}" rx="{rx:.1}" ry="{ry:.1}" fill="{}" stroke="#111"/>"##,
                if filled { "#111" } else { "white" }
            )),
            Primitive::Text { x, y, size, value } => out.push_str(&format!(
                r#"<text x="{x:.1}" y="{y:.1}" font-size="{size:.1}">{}</text>"#,
                esc(&value)
            )),
            Primitive::Rest { x, y, duration } => out.push_str(&svg_rest(x, y, duration)),
            Primitive::Flag { x, y, up, levels } => out.push_str(&svg_flag(x, y, up, levels)),
        }
    }
    out.push_str("</svg>");
    out
}

pub fn render_html(score: &Score) -> String {
    let pages = layout_document(score)
        .pages
        .into_iter()
        .map(render_page_svg)
        .collect::<String>();
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Muse score</title><style>body{{margin:2rem;background:#e7e7e7}}.page{{display:block;margin:0 auto 2rem;background:#fff;box-shadow:0 1px 7px #999;max-width:1200px;width:100%;height:auto}}</style>{pages}"
    )
}
fn render_page_svg(page: Page) -> String {
    let mut out = format!(
        r#"<svg class="page" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.1} {:.1}"><style>text{{font-family:serif;fill:#111}}line{{stroke:#222;stroke-linecap:round}}ellipse{{fill:#111}}</style><rect width="100%" height="100%" fill="white"/>"#,
        page.width, page.height
    );
    for primitive in page.primitives {
        match primitive {
            Primitive::Line { x1, y1, x2, y2, width } => out.push_str(&format!(
                r#"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke-width="{width:.1}"/>"#
            )),
            Primitive::Ellipse { cx, cy, rx, ry, filled } => out.push_str(&format!(
                r##"<ellipse cx="{cx:.1}" cy="{cy:.1}" rx="{rx:.1}" ry="{ry:.1}" fill="{}" stroke="#111"/>"##,
                if filled { "#111" } else { "white" }
            )),
            Primitive::Text { x, y, size, value } => out.push_str(&format!(
                r#"<text x="{x:.1}" y="{y:.1}" font-size="{size:.1}">{}</text>"#,
                esc(&value)
            )),
            Primitive::Rest { x, y, duration } => out.push_str(&svg_rest(x, y, duration)),
            Primitive::Flag { x, y, up, levels } => out.push_str(&svg_flag(x, y, up, levels)),
        }
    }
    out.push_str("</svg>");
    out
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
    let document = layout_document(score);
    let page_refs: Vec<usize> = (0..document.pages.len()).map(|index| 3 + index * 2).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            page_refs
                .iter()
                .map(|reference| format!("{reference} 0 R"))
                .collect::<Vec<_>>()
                .join(" "),
            page_refs.len()
        ),
    ];
    for (index, page) in document.pages.iter().enumerate() {
        let page_ref = 3 + index * 2;
        let content_ref = page_ref + 1;
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.1} {:.1}] /Resources << /Font << /F1 {} 0 R >> >> /Contents {} 0 R >>", page.width, page.height, 3 + document.pages.len() * 2, content_ref));
        let content = pdf_content(page);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            content.len(),
            content
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>".to_owned());
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

fn pdf_content(page: &Page) -> String {
    let mut out = String::new();
    for primitive in &page.primitives {
        match primitive {
            Primitive::Line { x1, y1, x2, y2, width } => out.push_str(&format!(
                "{width:.2} w {x1:.2} {:.2} m {x2:.2} {:.2} l S\n",
                page.height - y1,
                page.height - y2
            )),
            Primitive::Ellipse { cx, cy, rx, ry, filled } => {
                let y = page.height - cy;
                let k = 0.552_284_8;
                out.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {}\n", cx + rx, y, cx + rx, y + ry*k, cx + rx*k, y + ry, cx, y + ry, cx - rx*k, y + ry, cx-rx, y+ry*k, cx-rx, y, cx-rx, y-ry*k, cx-rx*k, y-ry, cx, y-ry, cx+rx*k, y-ry, cx+rx, y-ry*k, cx+rx, y, if *filled { "f" } else { "S" }));
            }
            Primitive::Text { x, y, size, value } => out.push_str(&format!(
                "BT /F1 {size:.1} Tf {x:.1} {:.1} Td ({}) Tj ET\n",
                page.height - y,
                pdf_escape(&pdf_text(value))
            )),
            Primitive::Rest { x, y, .. } => {
                out.push_str(&format!("BT /F1 14 Tf {x:.1} {:.1} Td (r) Tj ET\n", page.height - y))
            }
            Primitive::Flag { x, y, up, levels } => {
                let sign = if *up { 1.0 } else { -1.0 };
                for level in 0..*levels {
                    let offset = level as f32 * 5.0;
                    out.push_str(&format!(
                        "2.5 w {x:.1} {:.1} m {:.1} {:.1} l S\n",
                        page.height - y - sign * offset,
                        x + 10.0,
                        page.height - y - sign * (12.0 + offset)
                    ));
                }
            }
        }
    }
    out
}
fn esc(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn svg_rest(x: f32, y: f32, duration: DurationValue) -> String {
    let symbol = match duration {
        DurationValue::Whole => "𝄻",
        DurationValue::Half => "𝄼",
        DurationValue::Quarter => "𝄽",
        DurationValue::Eighth => "𝄾",
        DurationValue::Sixteenth => "𝄿",
        DurationValue::ThirtySecond => "𝅀",
        DurationValue::Other(_) => "𝄽",
    };
    format!(r#"<text x="{x:.1}" y="{y:.1}" font-size="15" font-family="Bravura, Noto Music, serif">{symbol}</text>"#)
}
fn svg_flag(x: f32, y: f32, up: bool, levels: u8) -> String {
    let sign = if up { 1.0 } else { -1.0 };
    (0..levels)
        .map(|level| {
            let offset = level as f32 * 5.0;
            format!(
                r##"<path d="M {x:.1} {y:.1} q 10 {:.1} 7 {:.1}" fill="none" stroke="#111" stroke-width="2.5"/>"##,
                sign * (6.0 + offset),
                sign * (13.0 + offset)
            )
        })
        .collect()
}
fn pdf_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .chars()
        .filter(char::is_ascii)
        .collect()
}
fn pdf_text(value: &str) -> String {
    value.replace("𝄞", "G")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_uses_one_page_per_document_scene_page() {
        let mut score = Score::default();
        score.tracks.push(muse_music::Track {
            name: "Piano".into(),
            channel: 0,
            program: 0,
            role: "melody".into(),
        });
        for bar in 0..12 {
            score.notes.push(muse_music::Note {
                track: 0,
                channel: 0,
                key: 60,
                velocity: 90,
                start_tick: bar * 1_920,
                duration_ticks: 480,
                articulation: String::new(),
            });
        }
        let path = std::env::temp_dir().join("muse-score-pages.pdf");
        write_pdf(&score, &path).unwrap();
        let pdf = std::fs::read(&path).unwrap();
        assert_eq!(
            pdf.windows(b"/Type /Page ".len())
                .filter(|item| *item == b"/Type /Page ")
                .count(),
            layout_document(&score).pages.len()
        );
        let _ = std::fs::remove_file(path);
    }
}
