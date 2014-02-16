//! Backend-neutral access to the bundled SMuFL font.
//!
//! Bravura is an OFL-1.1 font. Its license ships beside the font asset. We use
//! `ttf-parser` only to read outlines, so PDF and SVG receive the same paths and
//! never depend on a font installed on the user's machine.
use ttf_parser::{Face, OutlineBuilder};

const BRAVURA: &[u8] = include_bytes!("../assets/Bravura.otf");

pub const TREBLE_CLEF: char = '\u{E050}';
pub const BASS_CLEF: char = '\u{E062}';
pub const PERCUSSION_CLEF: char = '\u{E069}';
pub const SHARP: char = '\u{E262}';
pub const FLAT: char = '\u{E260}';

pub fn rest_glyph(duration: crate::DurationValue) -> char {
    match duration {
        crate::DurationValue::Whole => '\u{E4E3}',
        crate::DurationValue::Half => '\u{E4E4}',
        crate::DurationValue::Quarter | crate::DurationValue::Other(_) => '\u{E4E5}',
        crate::DurationValue::Eighth => '\u{E4E6}',
        crate::DurationValue::Sixteenth => '\u{E4E7}',
        crate::DurationValue::ThirtySecond => '\u{E4E8}',
    }
}

pub fn svg_glyph(ch: char, x: f32, baseline: f32, size: f32) -> Option<String> {
    let mut path = SvgPath::new(x, baseline, scale(size));
    outline(ch, &mut path)?;
    Some(format!(r##"<path d="{}" fill="#111"/>"##, path.data))
}

pub fn pdf_glyph(ch: char, x: f32, baseline: f32, size: f32) -> Option<String> {
    let mut path = PdfPath::new(x, baseline, scale(size));
    outline(ch, &mut path)?;
    Some(format!("{}f\n", path.data))
}

fn scale(size: f32) -> f32 {
    let face = Face::parse(BRAVURA, 0).expect("bundled Bravura must be valid");
    size / face.units_per_em() as f32
}

fn outline(ch: char, builder: &mut impl OutlineBuilder) -> Option<()> {
    let face = Face::parse(BRAVURA, 0).ok()?;
    let glyph = face.glyph_index(ch)?;
    face.outline_glyph(glyph, builder)?;
    Some(())
}

struct SvgPath {
    data: String,
    x: f32,
    y: f32,
    scale: f32,
}
impl SvgPath {
    fn new(x: f32, y: f32, scale: f32) -> Self {
        Self {
            data: String::new(),
            x,
            y,
            scale,
        }
    }
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y - y * self.scale)
    }
}
impl OutlineBuilder for SvgPath {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.data.push_str(&format!("M{x:.2} {y:.2}"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.data.push_str(&format!("L{x:.2} {y:.2}"));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.data.push_str(&format!("Q{x1:.2} {y1:.2} {x:.2} {y:.2}"));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.data
            .push_str(&format!("C{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2}"));
    }
    fn close(&mut self) {
        self.data.push('Z');
    }
}

struct PdfPath {
    data: String,
    x: f32,
    y: f32,
    scale: f32,
}
impl PdfPath {
    fn new(x: f32, y: f32, scale: f32) -> Self {
        Self {
            data: String::new(),
            x,
            y,
            scale,
        }
    }
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y + y * self.scale)
    }
}
impl OutlineBuilder for PdfPath {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.data.push_str(&format!("{x:.2} {y:.2} m "));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.data.push_str(&format!("{x:.2} {y:.2} l "));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // PDF has cubic curves only; elevate the quadratic using the current
        // point would require retained state. Bravura uses cubics for its glyphs.
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.data
            .push_str(&format!("{x1:.2} {y1:.2} {x1:.2} {y1:.2} {x:.2} {y:.2} c "));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.data
            .push_str(&format!("{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2} c "));
    }
    fn close(&mut self) {
        self.data.push_str("h ");
    }
}
