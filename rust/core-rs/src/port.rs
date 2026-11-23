//! The Rust port of `Core/`.
//!
//! Faithful port of `Core/CoreLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

fn csharp_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else if a >= b {
        a
    } else {
        b
    }
}

/// Port of `GridGetSize` (`Core/CoreLogic.cs:7`).
pub fn grid_get_size(
    units: i64,
    size: i64,
    margin: i64,
    scaling: f64,
    add_margin: bool,
) -> Result<i64, String> {
    let u = units as i32;
    let s = size as i32;
    let m = margin as i32;
    let sm = s.wrapping_add(m);
    let raw = if add_margin {
        scaling * f64::from(u) * f64::from(sm) + scaling * f64::from(m)
    } else {
        scaling * f64::from(u) * f64::from(sm) - scaling * f64::from(m)
    };
    Ok(i64::from(raw as i32))
}

/// Port of `GridSnapDimension` (`Core/CoreLogic.cs:16`).
pub fn grid_snap_dimension(
    pixels: f64,
    size: i64,
    margin: i64,
    scaling: f64,
    add_margin: bool,
    min_value: i64,
) -> Result<i64, String> {
    let s = size as i32;
    let m = margin as i32;
    let min_v = min_value as i32;
    let step = scaling * f64::from(s.wrapping_add(m));
    if step <= 0.0 {
        return Ok(0);
    }
    let rounded = (pixels / step).round_ties_even();
    let units = csharp_max(f64::from(min_v), rounded) as i32;
    grid_get_size(i64::from(units), size, margin, scaling, add_margin)
}

/// Port of `GridSnapSize` (`Core/CoreLogic.cs:27`).
pub fn grid_snap_size(pixels: f64, size: i64, margin: i64) -> Result<i64, String> {
    grid_snap_dimension(pixels, size, margin, 1.0, false, 1)
}

/// Port of `GridSnapPosition` (`Core/CoreLogic.cs:32`).
pub fn grid_snap_position(
    pixels: f64,
    size: i64,
    margin: i64,
    scaling: f64,
) -> Result<i64, String> {
    grid_snap_dimension(pixels, size, margin, scaling, true, 0)
}

/// Port of `ContentScale` (`Core/CoreLogic.cs:37`).
pub fn content_scale(dimension_size: i64) -> Result<f64, String> {
    Ok(f64::from(dimension_size as i32) / 72.0)
}

/// Port of `GalleryDefaultDimension` (`Core/CoreLogic.cs:42`).
pub fn gallery_default_dimension(size: i64, margin: i64) -> Result<i64, String> {
    let s = size as i32;
    let m = margin as i32;
    Ok(i64::from(s.wrapping_mul(2).wrapping_add(m)))
}

/// Port of `PresetColumns` (`Core/CoreLogic.cs:47`).
pub fn preset_columns(preset_index: i64) -> Result<i64, String> {
    Ok(match preset_index as i32 {
        0 => 2,
        1 => 4,
        2 => 4,
        3 => 8,
        _ => 2,
    })
}

/// Port of `PresetRows` (`Core/CoreLogic.cs:59`).
pub fn preset_rows(preset_index: i64) -> Result<i64, String> {
    Ok(match preset_index as i32 {
        0 => 2,
        1 => 2,
        2 => 4,
        3 => 4,
        _ => 2,
    })
}

/// Port of `ThemeTextOpacity` (`Core/CoreLogic.cs:71`).
pub fn theme_text_opacity(opacity_level: f64) -> Result<f64, String> {
    Ok(if opacity_level < 1.0 { 0.8 } else { 1.0 })
}

/// Port of `ThemeUsesAcrylic` (`Core/CoreLogic.cs:76`).
pub fn theme_uses_acrylic(opacity_level: f64) -> Result<bool, String> {
    Ok(opacity_level < 1.0)
}

/// Port of `ThemePreviewRadius` (`Core/CoreLogic.cs:81`).
pub fn theme_preview_radius(use_native_frame: bool) -> Result<i64, String> {
    Ok(if use_native_frame { 2 } else { 10 })
}

/// Port of `ThemePreviewBorder` (`Core/CoreLogic.cs:86`).
pub fn theme_preview_border(use_native_frame: bool) -> Result<i64, String> {
    Ok(if use_native_frame { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_defaults_match_uwidgets_layout() {
        assert_eq!(grid_get_size(2, 72, 16, 1.0, false).ok(), Some(160));
        assert_eq!(grid_get_size(4, 72, 16, 1.0, false).ok(), Some(336));
        assert_eq!(grid_get_size(8, 72, 16, 1.0, false).ok(), Some(688));
        assert_eq!(gallery_default_dimension(72, 16).ok(), Some(160));
    }
}
