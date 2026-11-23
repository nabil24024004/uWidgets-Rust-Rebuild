//! Real-time SDF Liquid Glass optical engine (inspired by Apple iOS 26 / macOS Tahoe,
//! Kyant0's AndroidLiquidGlass, and liquidGL).
//!
//! Pipeline per widget card (cached to an `ID2D1Bitmap`, recomputed only on move/resize/theme/wallpaper change):
//! 1. Exact rounded-box Signed Distance Field (SDF) + smooth subpixel AA coverage.
//! 2. Analytic outward SDF surface normals + convex curved glass bevel profile.
//! 3. Physical lens refraction along the curved glass rim with RGB chromatic dispersion.
//! 4. True multi-pass Gaussian frosted backdrop transmission + Apple vibrancy saturation boost.
//! 5. Translucent glass tint absorption + top-to-bottom internal depth sheen + subtle micro-frost grain.
//! 6. Dual-angle specular Fresnel bevel rim highlights (primary top-left gloss + opposing bottom-right bounce).

use std::cell::RefCell;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1, ID2D1DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::D2D::IWICImagingFactory2;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
    WICRect,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
use windows_core::PCWSTR;

use crate::models::{LiquidGlassConfig, ResolvedTheme, parse_hex_color};
use crate::platform::system::{get_wallpaper_path, to_wide};

/// Cached half-resolution desktop wallpaper with pre-blurred frosted + micro-softened refraction buffers.
/// Total memory for a 1080p screen: ~4.6 MB shared across all desktop widgets.
pub struct DesktopBackdrop {
    pub screen_w: i32,
    pub screen_h: i32,
    pub buf_w: usize,
    pub buf_h: usize,
    pub generation: u64,
    base_rgb: Vec<u8>, // 3 bytes/pixel (R, G, B) — unblurred wallpaper for instant re-blur
    sharp_rgb: Vec<u8>, // 3 bytes/pixel (R, G, B) — slightly softened for crisp rim refraction
    frosted_rgb: Vec<u8>, // 3 bytes/pixel (R, G, B) — silky Gaussian blur for frosted glass interior
    frosted_radius: usize,
}

impl DesktopBackdrop {
    pub fn load(generation: u64, initial_blur: usize) -> Option<Self> {
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(640);
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(480);
        let buf_w = ((screen_w as usize) / 2).clamp(320, 1440);
        let buf_h = ((screen_h as usize) / 2).clamp(200, 900);

        let path = get_wallpaper_path();
        if path.is_empty() {
            return None;
        }
        let path_w = to_wide(&path);

        let bgra = unsafe {
            let wic: IWICImagingFactory2 =
                CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_INPROC_SERVER).ok()?;
            let decoder = wic
                .CreateDecoderFromFilename(
                    PCWSTR(path_w.as_ptr()),
                    None,
                    windows::Win32::Foundation::GENERIC_READ,
                    WICDecodeMetadataCacheOnLoad,
                )
                .ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let mut img_w = 0u32;
            let mut img_h = 0u32;
            frame.GetSize(&mut img_w, &mut img_h).ok()?;
            if img_w == 0 || img_h == 0 {
                return None;
            }

            // Match Windows "Fill" wallpaper scaling & center crop so refraction aligns pixel-for-pixel.
            let scale = (f64::from(buf_w as u32) / f64::from(img_w))
                .max(f64::from(buf_h as u32) / f64::from(img_h));
            let scaled_w = ((f64::from(img_w) * scale).round() as u32).max(buf_w as u32);
            let scaled_h = ((f64::from(img_h) * scale).round() as u32).max(buf_h as u32);

            let scaler = wic.CreateBitmapScaler().ok()?;
            scaler
                .Initialize(&frame, scaled_w, scaled_h, WICBitmapInterpolationModeFant)
                .ok()?;

            let converter = wic.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &scaler,
                    &GUID_WICPixelFormat32bppBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeCustom,
                )
                .ok()?;

            let crop_x = ((scaled_w - buf_w as u32) / 2) as i32;
            let crop_y = ((scaled_h - buf_h as u32) / 2) as i32;
            let crop_rc = WICRect {
                X: crop_x,
                Y: crop_y,
                Width: buf_w as i32,
                Height: buf_h as i32,
            };
            let stride = (buf_w * 4) as u32;
            let mut bgra = vec![0u8; buf_w * buf_h * 4];
            converter.CopyPixels(&crop_rc, stride, &mut bgra).ok()?;
            bgra
        };

        let mut base_rgb = vec![0u8; buf_w * buf_h * 3];
        for i in 0..(buf_w * buf_h) {
            base_rgb[i * 3] = bgra[i * 4 + 2];
            base_rgb[i * 3 + 1] = bgra[i * 4 + 1];
            base_rgb[i * 3 + 2] = bgra[i * 4];
        }

        // Sharp refraction layer: subtle 2px optical smoothing (keeps curved rim lens distortion crisp)
        let sharp_rgb = gaussian_blur_rgb(&base_rgb, buf_w, buf_h, 2);
        // Frosted transmission layer: user-configurable Gaussian frosted blur
        let r_clamped = initial_blur.min(32);
        let frosted_rgb = gaussian_blur_rgb(&base_rgb, buf_w, buf_h, r_clamped);

        Some(Self {
            screen_w,
            screen_h,
            buf_w,
            buf_h,
            generation,
            base_rgb,
            sharp_rgb,
            frosted_rgb,
            frosted_radius: r_clamped,
        })
    }

    pub fn ensure_blur_radius(&mut self, radius: usize) {
        let r = radius.min(32);
        if self.frosted_radius != r {
            self.frosted_rgb = gaussian_blur_rgb(&self.base_rgb, self.buf_w, self.buf_h, r);
            self.frosted_radius = r;
        }
    }

    #[inline]
    fn sample_bilinear(buf: &[u8], w: usize, h: usize, u: f32, v: f32) -> (f32, f32, f32) {
        let fx = u.clamp(0.0, (w - 1) as f32);
        let fy = v.clamp(0.0, (h - 1) as f32);
        let x0 = fx as usize;
        let y0 = fy as usize;
        let x1 = (x0 + 1).min(w - 1);
        let y1 = (y0 + 1).min(h - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let i00 = (y0 * w + x0) * 3;
        let i10 = (y0 * w + x1) * 3;
        let i01 = (y1 * w + x0) * 3;
        let i11 = (y1 * w + x1) * 3;

        let w00 = (1.0 - tx) * (1.0 - ty);
        let w10 = tx * (1.0 - ty);
        let w01 = (1.0 - tx) * ty;
        let w11 = tx * ty;

        let r = f32::from(buf[i00]) * w00
            + f32::from(buf[i10]) * w10
            + f32::from(buf[i01]) * w01
            + f32::from(buf[i11]) * w11;
        let g = f32::from(buf[i00 + 1]) * w00
            + f32::from(buf[i10 + 1]) * w10
            + f32::from(buf[i01 + 1]) * w01
            + f32::from(buf[i11 + 1]) * w11;
        let b = f32::from(buf[i00 + 2]) * w00
            + f32::from(buf[i10 + 2]) * w10
            + f32::from(buf[i01 + 2]) * w01
            + f32::from(buf[i11 + 2]) * w11;
        (r / 255.0, g / 255.0, b / 255.0)
    }

    #[inline]
    fn sample_channel(buf: &[u8], w: usize, h: usize, u: f32, v: f32, ch: usize) -> f32 {
        let fx = u.clamp(0.0, (w - 1) as f32);
        let fy = v.clamp(0.0, (h - 1) as f32);
        let x0 = fx as usize;
        let y0 = fy as usize;
        let x1 = (x0 + 1).min(w - 1);
        let y1 = (y0 + 1).min(h - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let c00 = f32::from(buf[(y0 * w + x0) * 3 + ch]);
        let c10 = f32::from(buf[(y0 * w + x1) * 3 + ch]);
        let c01 = f32::from(buf[(y1 * w + x0) * 3 + ch]);
        let c11 = f32::from(buf[(y1 * w + x1) * 3 + ch]);

        let top = c00 + (c10 - c00) * tx;
        let bot = c01 + (c11 - c01) * tx;
        (top + (bot - top) * ty) / 255.0
    }
}

/// Fast 3-pass separable sliding-window box blur (O(N) regardless of radius; converges to true Gaussian).
fn gaussian_blur_rgb(src: &[u8], w: usize, h: usize, radius: usize) -> Vec<u8> {
    if radius == 0 || w < 2 || h < 2 {
        return src.to_vec();
    }
    let r = radius.min(w.min(h) / 3).max(1);
    let mut a = src.to_vec();
    let mut b = vec![0u8; src.len()];
    for _ in 0..3 {
        box_blur_h(&a, &mut b, w, h, r);
        box_blur_v(&b, &mut a, w, h, r);
    }
    a
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let div = (2 * r + 1) as u32;
    let half = div / 2;
    for y in 0..h {
        let row = y * w;
        let mut sr = u32::from(src[row * 3]) * (r as u32 + 1);
        let mut sg = u32::from(src[row * 3 + 1]) * (r as u32 + 1);
        let mut sb = u32::from(src[row * 3 + 2]) * (r as u32 + 1);
        for k in 1..=r {
            let idx = (row + k.min(w - 1)) * 3;
            sr += u32::from(src[idx]);
            sg += u32::from(src[idx + 1]);
            sb += u32::from(src[idx + 2]);
        }
        for x in 0..w {
            let out = (row + x) * 3;
            dst[out] = ((sr + half) / div).min(255) as u8;
            dst[out + 1] = ((sg + half) / div).min(255) as u8;
            dst[out + 2] = ((sb + half) / div).min(255) as u8;

            let add_x = (x + r + 1).min(w - 1);
            let sub_x = x.saturating_sub(r);
            let ai = (row + add_x) * 3;
            let si = (row + sub_x) * 3;
            sr = sr + u32::from(src[ai]) - u32::from(src[si]);
            sg = sg + u32::from(src[ai + 1]) - u32::from(src[si + 1]);
            sb = sb + u32::from(src[ai + 2]) - u32::from(src[si + 2]);
        }
    }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let div = (2 * r + 1) as u32;
    let half = div / 2;
    for x in 0..w {
        let mut sr = u32::from(src[x * 3]) * (r as u32 + 1);
        let mut sg = u32::from(src[x * 3 + 1]) * (r as u32 + 1);
        let mut sb = u32::from(src[x * 3 + 2]) * (r as u32 + 1);
        for k in 1..=r {
            let idx = (k.min(h - 1) * w + x) * 3;
            sr += u32::from(src[idx]);
            sg += u32::from(src[idx + 1]);
            sb += u32::from(src[idx + 2]);
        }
        for y in 0..h {
            let out = (y * w + x) * 3;
            dst[out] = ((sr + half) / div).min(255) as u8;
            dst[out + 1] = ((sg + half) / div).min(255) as u8;
            dst[out + 2] = ((sb + half) / div).min(255) as u8;

            let add_y = (y + r + 1).min(h - 1);
            let sub_y = y.saturating_sub(r);
            let ai = (add_y * w + x) * 3;
            let si = (sub_y * w + x) * 3;
            sr = sr + u32::from(src[ai]) - u32::from(src[si]);
            sg = sg + u32::from(src[ai + 1]) - u32::from(src[si + 1]);
            sb = sb + u32::from(src[ai + 2]) - u32::from(src[si + 2]);
        }
    }
}

/// Cache key determining when a widget's rendered Liquid Glass plate must be regenerated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlassCacheKey {
    pub screen_x: i32,
    pub screen_y: i32,
    pub phys_w: u32,
    pub phys_h: u32,
    pub dpi_x10: u32,
    pub radius_x10: u32,
    pub opacity_x100: u32,
    pub is_dark: bool,
    pub use_native_frame: bool,
    pub blur_radius: i32,
    pub refraction_x100: u32,
    pub bevel_x100: u32,
    pub dispersion_x100: u32,
    pub specular_x100: u32,
    pub vibrancy_x100: u32,
    pub grain_x100: u32,
    pub tint_hex: Option<String>,
    pub wallpaper_gen: u64,
}

pub struct WidgetGlassCache {
    key: Option<GlassCacheKey>,
    bitmap: Option<ID2D1Bitmap1>,
}

impl WidgetGlassCache {
    pub fn new() -> Self {
        Self {
            key: None,
            bitmap: None,
        }
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.bitmap = None;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_or_build(
        &mut self,
        dc: &ID2D1DeviceContext,
        backdrop: Option<&DesktopBackdrop>,
        screen_x: i32,
        screen_y: i32,
        phys_w: u32,
        phys_h: u32,
        dpi: f32,
        radius_dip: f32,
        theme: ResolvedTheme,
        lg: &LiquidGlassConfig,
    ) -> Option<ID2D1Bitmap1> {
        let w = phys_w.max(2);
        let h = phys_h.max(2);
        let wgen = backdrop.map_or(0, |b| b.generation);
        let key = GlassCacheKey {
            screen_x,
            screen_y,
            phys_w: w,
            phys_h: h,
            dpi_x10: (dpi * 10.0).round() as u32,
            radius_x10: (radius_dip * 10.0).round() as u32,
            opacity_x100: (theme.bg_alpha * 100.0).round() as u32,
            is_dark: theme.is_dark,
            use_native_frame: theme.use_native_frame,
            blur_radius: lg.blur_radius.clamp(0, 32),
            refraction_x100: (lg.refraction.clamp(0.0, 2.0) * 100.0).round() as u32,
            bevel_x100: (lg.bevel_width.clamp(0.3, 1.8) * 100.0).round() as u32,
            dispersion_x100: (lg.dispersion.clamp(0.0, 2.5) * 100.0).round() as u32,
            specular_x100: (lg.specular.clamp(0.0, 2.0) * 100.0).round() as u32,
            vibrancy_x100: (lg.vibrancy.clamp(0.5, 2.0) * 100.0).round() as u32,
            grain_x100: (lg.frost_grain.clamp(0.0, 2.5) * 100.0).round() as u32,
            tint_hex: lg.tint_color.clone(),
            wallpaper_gen: wgen,
        };
        if self.key.as_ref() == Some(&key)
            && let Some(bmp) = &self.bitmap
        {
            return Some(bmp.clone());
        }

        let pixels = synthesize_liquid_glass_pbgra(
            backdrop, screen_x, screen_y, w, h, dpi, radius_dip, theme, lg,
        );
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            ..Default::default()
        };
        let bmp = unsafe {
            dc.CreateBitmap(
                D2D_SIZE_U {
                    width: w,
                    height: h,
                },
                Some(pixels.as_ptr().cast()),
                w * 4,
                &props,
            )
            .ok()?
        };
        self.key = Some(key);
        self.bitmap = Some(bmp.clone());
        Some(bmp)
    }
}

/// Core optical SDF Liquid Glass synthesizer producing premultiplied BGRA pixels.
#[allow(clippy::too_many_arguments)]
pub fn synthesize_liquid_glass_pbgra(
    backdrop: Option<&DesktopBackdrop>,
    screen_x: i32,
    screen_y: i32,
    w: u32,
    h: u32,
    dpi: f32,
    radius_dip: f32,
    theme: ResolvedTheme,
    lg: &LiquidGlassConfig,
) -> Vec<u8> {
    let wi = w as usize;
    let hi = h as usize;
    let mut out = vec![0u8; wi * hi * 4];

    let scale = (dpi / 96.0).max(1.0);
    let wf = w as f32;
    let hf = h as f32;
    let cx = wf * 0.5;
    let cy = hf * 0.5;
    let max_r = (wf.min(hf) * 0.48).max(1.0);

    // User-customizable optical parameters
    let lg_refraction = lg.refraction.clamp(0.0, 2.0) as f32;
    let lg_bevel = lg.bevel_width.clamp(0.3, 1.8) as f32;
    let lg_dispersion = lg.dispersion.clamp(0.0, 2.5) as f32;
    let lg_specular = lg.specular.clamp(0.0, 2.0) as f32;
    let lg_vibrancy = lg.vibrancy.clamp(0.5, 2.0) as f32;
    let lg_grain = lg.frost_grain.clamp(0.0, 2.5) as f32;

    // Native Win11 Acrylic mode vs Apple SDF Liquid Glass mode
    let is_acrylic = theme.use_native_frame;
    let eff_radius_dip = if is_acrylic { 8.0 } else { radius_dip };
    let r = (eff_radius_dip * scale).clamp(2.0, max_r);
    let bx = cx - r;
    let by = cy - r;

    // Bevel geometry (Kyant / liquidGL style curved glass rim for Apple; flat Acrylic for Windows)
    let bevel_w = if is_acrylic {
        4.0 * scale
    } else {
        ((r * 0.92).clamp(18.0 * scale, 34.0 * scale) * lg_bevel).clamp(6.0 * scale, max_r * 1.45)
    };
    let glassiness = (1.0 - theme.bg_alpha).clamp(0.0, 0.95);
    let refract_px = if is_acrylic {
        0.0
    } else {
        (18.5 * scale) * (0.45 + 0.55 * glassiness) * lg_refraction
    };
    let rim_w = if is_acrylic { 1.2 * scale } else { 1.6 * scale };
    let caustic_w = if is_acrylic {
        0.0
    } else {
        10.0 * scale * (0.5 + 0.5 * lg_bevel)
    };

    // Directional Apple specular lighting vectors (upper-left primary key + bottom-right bounce)
    let l1x = -0.60_f32;
    let l1y = -0.80_f32;
    let l2x = 0.60_f32;
    let l2y = 0.80_f32;

    // Tint parameters: strictly track `theme.bg_alpha` (5% -> 99%) + optional custom glass tint dye
    let (base_tr, base_tg, base_tb, base_ta) = if is_acrylic {
        if theme.is_dark {
            (
                0.155_f32,
                0.16_f32,
                0.175_f32,
                (theme.bg_alpha * 0.82).clamp(0.04, 0.96),
            )
        } else {
            (
                0.96_f32,
                0.965_f32,
                0.98_f32,
                (theme.bg_alpha * 0.80).clamp(0.04, 0.96),
            )
        }
    } else if theme.is_dark {
        (
            0.09_f32,
            0.105_f32,
            0.13_f32,
            (theme.bg_alpha * 0.72).clamp(0.04, 0.95),
        )
    } else {
        (
            0.96_f32,
            0.975_f32,
            1.00_f32,
            (theme.bg_alpha * 0.68).clamp(0.05, 0.95),
        )
    };
    let (tint_r, tint_g, tint_b, tint_a) =
        if let Some((cr, cg, cb)) = lg.tint_color.as_deref().and_then(parse_hex_color) {
            let mix = if theme.is_dark { 0.80_f32 } else { 0.72_f32 };
            (
                base_tr * (1.0 - mix) + cr * mix,
                base_tg * (1.0 - mix) + cg * mix,
                base_tb * (1.0 - mix) + cb * mix,
                (base_ta * 1.08).clamp(0.08, 0.95),
            )
        } else {
            (base_tr, base_tg, base_tb, base_ta)
        };

    let (sx_ratio, sy_ratio) = match backdrop {
        Some(b) => (
            b.buf_w as f32 / b.screen_w.max(1) as f32,
            b.buf_h as f32 / b.screen_h.max(1) as f32,
        ),
        None => (0.5, 0.5),
    };

    for py in 0..hi {
        let fy = py as f32 + 0.5;
        let rel_y = fy - cy;
        let qy = rel_y.abs() - by;
        let v_norm = fy / hf; // 0 at top, 1 at bottom

        for px in 0..wi {
            let fx = px as f32 + 0.5;
            let rel_x = fx - cx;
            let qx = rel_x.abs() - bx;

            // 1. Exact IQ Rounded-Box Signed Distance Field
            let ox = qx.max(0.0);
            let oy = qy.max(0.0);
            let outside_len = (ox * ox + oy * oy).sqrt();
            let inside_d = qx.max(qy).min(0.0);
            let d = outside_len + inside_d - r;

            // Smooth sub-pixel anti-aliased edge coverage
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov <= 0.001 {
                continue;
            }
            let dist_in = (-d).max(0.0);

            // 2. Analytic outward SDF unit normal (nx, ny)
            let (nx, ny) = if outside_len > 1e-4 {
                (
                    (ox / outside_len) * rel_x.signum(),
                    (oy / outside_len) * rel_y.signum(),
                )
            } else if qx > qy {
                (rel_x.signum(), 0.0)
            } else {
                (0.0, rel_y.signum())
            };

            // 3. Convex curved glass bevel profile (0 in flat center, 1 at curved outer rim)
            let t_bevel = (1.0 - (dist_in / bevel_w).clamp(0.0, 1.0)).powf(2.15);

            // 4. Sample frosted + refracted + chromatically dispersed desktop wallpaper
            let (mut br, mut bg, mut bb) = if let Some(bd) = backdrop {
                let base_u = (screen_x as f32 + fx) * sx_ratio;
                let base_v = (screen_y as f32 + fy) * sy_ratio;

                // Core silky frosted transmission (gentle interior magnification)
                let core_disp = t_bevel * refract_px * 0.45;
                let (fr, fg, fb) = DesktopBackdrop::sample_bilinear(
                    &bd.frosted_rgb,
                    bd.buf_w,
                    bd.buf_h,
                    base_u - nx * core_disp * sx_ratio,
                    base_v - ny * core_disp * sy_ratio,
                );

                // Curved rim optical refraction + RGB chromatic dispersion (Apple Liquid Glass mode)
                let lens_strength = (lg_refraction * 0.85 + lg_dispersion * 0.15).clamp(0.0, 1.25);
                if !is_acrylic && t_bevel > 0.01 && lens_strength > 0.001 {
                    let disp = t_bevel * refract_px;
                    let chromatic_px = (refract_px * 0.22 + 2.0 * scale) * lg_dispersion * t_bevel;
                    let du_g = nx * disp * sx_ratio;
                    let dv_g = ny * disp * sy_ratio;
                    let du_c = nx * chromatic_px * sx_ratio;
                    let dv_c = ny * chromatic_px * sy_ratio;
                    // Red bends slightly more, Blue slightly less -> physical prism dispersion
                    let rr = DesktopBackdrop::sample_channel(
                        &bd.sharp_rgb,
                        bd.buf_w,
                        bd.buf_h,
                        base_u - (du_g + du_c),
                        base_v - (dv_g + dv_c),
                        0,
                    );
                    let rg = DesktopBackdrop::sample_channel(
                        &bd.sharp_rgb,
                        bd.buf_w,
                        bd.buf_h,
                        base_u - du_g,
                        base_v - dv_g,
                        1,
                    );
                    let rb = DesktopBackdrop::sample_channel(
                        &bd.sharp_rgb,
                        bd.buf_w,
                        bd.buf_h,
                        base_u - (du_g - du_c),
                        base_v - (dv_g - dv_c),
                        2,
                    );
                    // Crisp optical refraction intensifies right on the curved glass bevel!
                    let rim_clarity = (t_bevel * 0.76 * (0.45 + 0.55 * glassiness) * lens_strength)
                        .clamp(0.0, 0.88);
                    (
                        fr + (rr - fr) * rim_clarity,
                        fg + (rg - fg) * rim_clarity,
                        fb + (rb - fb) * rim_clarity,
                    )
                } else {
                    (fr, fg, fb)
                }
            } else if theme.is_dark {
                (0.16, 0.19, 0.24)
            } else {
                (0.86, 0.90, 0.96)
            };

            // 5. Vibrancy saturation boost (user-configurable chroma saturation)
            let luma = 0.2126 * br + 0.7152 * bg + 0.0722 * bb;
            let sat_boost = if is_acrylic {
                (1.0 + (lg_vibrancy - 1.0) * 0.6).clamp(0.5, 1.8)
            } else {
                lg_vibrancy
            };
            br = (luma + (br - luma) * sat_boost).clamp(0.0, 1.0);
            bg = (luma + (bg - luma) * sat_boost).clamp(0.0, 1.0);
            bb = (luma + (bb - luma) * sat_boost).clamp(0.0, 1.0);

            // 6. Glass tint absorption + subtle top-to-bottom 3D curved dome sheen
            br = br * (1.0 - tint_a) + tint_r * tint_a;
            bg = bg * (1.0 - tint_a) + tint_g * tint_a;
            bb = bb * (1.0 - tint_a) + tint_b * tint_a;

            if !is_acrylic {
                let dome_spec = 0.30 + 0.70 * lg_specular.min(1.5);
                let top_sheen = ((1.0 - v_norm).powf(2.4)
                    * if theme.is_dark { 0.075 } else { 0.12 })
                    * (0.35 + 0.65 * glassiness)
                    * dome_spec;
                let bot_shade = (v_norm.powf(2.2) * if theme.is_dark { 0.045 } else { 0.03 })
                    * (0.35 + 0.65 * glassiness);
                br = (br + top_sheen - bot_shade).clamp(0.0, 1.0);
                bg = (bg + top_sheen - bot_shade).clamp(0.0, 1.0);
                bb = (bb + top_sheen - bot_shade).clamp(0.0, 1.0);
            }

            // 7. Rim lighting: Fluent top-lit hairline on Windows Acrylic, Dual-angle Specular Fresnel on Apple Liquid Glass
            let rim_band = (1.0 - (dist_in / rim_w).clamp(0.0, 1.0)).powf(1.5);
            let (prism_r, prism_g, prism_b) = if is_acrylic {
                let top_edge = (-ny).max(0.0);
                let edge_alpha = if theme.is_dark {
                    rim_band * (0.09 + 0.10 * top_edge)
                } else {
                    rim_band * (0.14 + 0.14 * top_edge)
                } * lg_specular;
                (edge_alpha, edge_alpha, edge_alpha)
            } else {
                let inner_caustic = if caustic_w > 0.1 {
                    (1.0 - (dist_in / caustic_w).clamp(0.0, 1.0)).powf(2.8)
                } else {
                    0.0
                };
                let key_dot = (nx * l1x + ny * l1y).max(0.0);
                let bounce_dot = (nx * l2x + ny * l2y).max(0.0);
                let spec_intensity = if theme.is_dark {
                    rim_band * (0.10 + 0.48 * key_dot.powf(1.8) + 0.22 * bounce_dot.powf(2.4))
                        + inner_caustic
                            * (0.03 + 0.11 * key_dot.powf(2.0) + 0.05 * bounce_dot.powf(2.2))
                } else {
                    rim_band * (0.22 + 0.48 * key_dot.powf(1.6) + 0.22 * bounce_dot.powf(2.2))
                        + inner_caustic * (0.05 + 0.13 * key_dot.powf(2.0))
                } * lg_specular;
                (
                    spec_intensity * (1.0 + 0.08 * lg_dispersion * bounce_dot),
                    spec_intensity,
                    spec_intensity * (1.0 + 0.10 * lg_dispersion * key_dot),
                )
            };

            // Subtle deterministic Acrylic / frost grain to eliminate banding
            let h_seed = ((px as u32).wrapping_mul(374_761_393)
                ^ (py as u32).wrapping_mul(668_265_263))
            .wrapping_mul(1_274_126_177);
            let grain_amp = (if is_acrylic { 0.022 } else { 0.012 }) * lg_grain;
            let grain = (((h_seed >> 24) as f32 / 255.0) - 0.5) * grain_amp;

            let fr = (br + prism_r + grain).clamp(0.0, 1.0);
            let fg = (bg + prism_g + grain).clamp(0.0, 1.0);
            let fb = (bb + prism_b + grain).clamp(0.0, 1.0);

            // Premultiply by anti-aliased rounded-corner coverage
            let idx = (py * wi + px) * 4;
            out[idx] = (fb * cov * 255.0).round() as u8;
            out[idx + 1] = (fg * cov * 255.0).round() as u8;
            out[idx + 2] = (fr * cov * 255.0).round() as u8;
            out[idx + 3] = (cov * 255.0).round() as u8;
        }
    }

    out
}

/// Shared holder inside `SharedGraphics` so wallpaper decoding happens lazily once & refreshes on change.
pub struct BackdropManager {
    backdrop: RefCell<Option<DesktopBackdrop>>,
    generation: RefCell<u64>,
}

impl BackdropManager {
    pub fn new() -> Self {
        Self {
            backdrop: RefCell::new(None),
            generation: RefCell::new(0),
        }
    }

    pub fn invalidate(&self) {
        *self.backdrop.borrow_mut() = None;
    }

    pub fn with_backdrop<R, F: FnOnce(Option<&DesktopBackdrop>) -> R>(
        &self,
        blur_radius: usize,
        f: F,
    ) -> R {
        let r = blur_radius.min(32);
        let needs_load = self.backdrop.borrow().is_none();
        if needs_load {
            let mut gen_slot = self.generation.borrow_mut();
            *gen_slot += 1;
            let g = *gen_slot;
            drop(gen_slot);
            let loaded = DesktopBackdrop::load(g, r);
            *self.backdrop.borrow_mut() = loaded;
        } else if let Some(bd) = self.backdrop.borrow_mut().as_mut() {
            bd.ensure_blur_radius(r);
        }
        let guard = self.backdrop.borrow();
        f(guard.as_ref())
    }
}
