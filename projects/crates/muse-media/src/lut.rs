//! Gradient-map LUT resolution for Muse scenes.
//!
//! 归属裁决（G7-W2）：停点→条带→查表混合的通用语义归 `media.post::GradeMap`；
//! 本模块只保留 Muse 的创作内容——内置 P1–P5 停点表（品牌主题，不下沉上游）
//! 与「内置 id 或工程相对 PNG 路径」的解析。

use anyhow::{Context, Result, bail};
use media_post::GradeMap;
use std::path::Path;

/// Resolve a LUT id (`P2`) or project-relative PNG path into a 256-entry grade map.
pub fn resolve_lut(name: &str, asset_root: &Path) -> Result<GradeMap> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("lut name must be non-empty");
    }
    if looks_like_path(trimmed) {
        let path = if Path::new(trimmed).is_absolute() {
            Path::new(trimmed).to_path_buf()
        } else {
            asset_root.join(trimmed)
        };
        let mut media = media_codec::MediaLibrary::new();
        let bitmap = media
            .image(&path.to_string_lossy(), crate::compose::DECODE_LIMITS)
            .with_context(|| format!("cannot open lut `{}` ({})", trimmed, path.display()))?;
        return Ok(GradeMap::from_strip(&bitmap.rgba, bitmap.width, bitmap.height));
    }
    builtin_gradmap(trimmed)
}

fn looks_like_path(name: &str) -> bool {
    name.contains('/')
        || name.contains('\\')
        || name.contains('.')
        || name.starts_with("assets")
}

/// 内置渐变色标表（创作内容）：大正/残響/踏切/灯火/食堂。
fn builtin_gradmap(id: &str) -> Result<GradeMap> {
    let key = id.trim().to_ascii_uppercase().replace('-', "_");
    let stops: &[(f32, [u8; 3])] = match key.as_str() {
        "P1" | "P1_OUMAGATOKI" => &[
            (0.0, [12, 8, 18]),
            (0.2, [48, 18, 36]),
            (0.45, [200, 55, 58]),
            (0.7, [255, 165, 82]),
            (1.0, [255, 228, 196]),
        ],
        "P2" | "P2_ZANSHO" => &[
            (0.0, [11, 21, 36]),
            (0.25, [30, 53, 82]),
            (0.45, [63, 98, 132]),
            (0.6, [216, 84, 42]),
            (0.8, [244, 166, 64]),
            (1.0, [233, 217, 184]),
        ],
        "P3" | "P3_FUMIKIRI" => &[
            (0.0, [8, 8, 10]),
            (0.35, [40, 12, 16]),
            (0.55, [200, 40, 48]),
            (1.0, [245, 245, 245]),
        ],
        "P4" | "P4_HOTARUBI" => &[
            (0.0, [6, 12, 28]),
            (0.35, [24, 48, 72]),
            (0.65, [80, 160, 140]),
            (1.0, [220, 240, 200]),
        ],
        "P5" | "P5_TAISHOKU" => &[
            (0.0, [20, 28, 40]),
            (0.4, [90, 110, 130]),
            (0.7, [180, 190, 200]),
            (1.0, [240, 244, 248]),
        ],
        "IDENTITY" | "NONE" => return Ok(GradeMap::identity()),
        other => bail!("unknown built-in lut `{other}` (use P1–P5 or a .png path)"),
    };
    Ok(GradeMap::from_stops(stops))
}

/// 256 格查找表 → 256×1 RGBA 条带（GPU 纹理上传用）。
#[must_use]
pub fn grade_map_strip(map: &GradeMap) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(256, 1);
    for (x, entry) in map.table().iter().enumerate() {
        img.put_pixel(x as u32, 0, image::Rgba([entry[0], entry[1], entry[2], 255]));
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_stops_match_the_creative_tables() {
        let p2 = builtin_gradmap("P2").unwrap();
        assert_eq!(p2.table()[0], [11, 21, 36]);
        assert_eq!(p2.table()[255], [233, 217, 184]);
        // t=0.6 → 216,84,42 停点：153/255=0.6 精确命中。
        assert_eq!(p2.table()[153], [216, 84, 42]);
    }

    #[test]
    fn identity_is_luma_passthrough_table() {
        let map = resolve_lut("IDENTITY", Path::new(".")).unwrap();
        assert_eq!(map.table()[0], [0, 0, 0]);
        assert_eq!(map.table()[128], [128, 128, 128]);
        assert_eq!(map.table()[255], [255, 255, 255]);
    }

    #[test]
    fn unknown_id_fails_explicitly() {
        let error = resolve_lut("P9", Path::new(".")).unwrap_err();
        assert!(error.to_string().contains("unknown built-in lut"), "{error}");
    }

    #[test]
    fn strip_upload_is_256_by_1() {
        let map = resolve_lut("P1", Path::new(".")).unwrap();
        let strip = grade_map_strip(&map);
        assert_eq!((strip.width(), strip.height()), (256, 1));
    }
}
