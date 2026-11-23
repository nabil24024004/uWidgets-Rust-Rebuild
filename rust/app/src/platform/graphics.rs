use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_CAP_STYLE_ROUND,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_ELLIPSE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_LINE_JOIN_ROUND, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES1, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Bitmap, ID2D1Bitmap1, ID2D1Device,
    ID2D1DeviceContext, ID2D1Factory1, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_SDK_VERSION,
    D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_TEXT_ALIGNMENT,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory5,
    IDWriteFontCollection, IDWriteFontCollection1, IDWriteInMemoryFontFileLoader,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::Win32::Graphics::Imaging::D2D::IWICImagingFactory2;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppPBGRA, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_core::{Interface, PCWSTR, Result};

use crate::icons::parse_svg_path;
use crate::platform::liquid_glass::BackdropManager;
use crate::platform::system::{get_wallpaper_path, to_wide};

const INTER_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Bold.ttf");

type TextFormatKey = (String, u32, i32, i32, i32, bool);

/// Shared GPU/2D context across all widget & UI windows (single device = minimal RAM!).
pub struct SharedGraphics {
    pub _d3d_device: ID3D11Device,
    pub dxgi_factory: IDXGIFactory2,
    pub d2d_factory: ID2D1Factory1,
    pub d2d_device: ID2D1Device,
    pub dcomp_device: IDCompositionDevice,
    pub dwrite: IDWriteFactory5,
    pub _font_loader: Option<IDWriteInMemoryFontFileLoader>,
    pub inter_collection: Option<IDWriteFontCollection>,
    pub round_stroke: ID2D1StrokeStyle,
    pub svg_cache: RefCell<HashMap<String, (ID2D1PathGeometry, D2D_RECT_F)>>,
    pub text_fmt_cache: RefCell<HashMap<TextFormatKey, IDWriteTextFormat>>,
    pub wallpaper_path: String,
    pub wic: RefCell<Option<IWICImagingFactory2>>,
    pub backdrop: BackdropManager,
}

impl SharedGraphics {
    pub fn new() -> Result<Rc<Self>> {
        let mut d3d_device: Option<ID3D11Device> = None;
        let feature_levels = [D3D_FEATURE_LEVEL_11_0];
        // Use single-threaded WARP for Direct2D rasterization + native DWM DirectComposition:
        // avoids loading 60-95 MB of vendor GPU shader-compiler DLLs & driver thread pools
        // while keeping crisp hardware-composited Win11 Acrylic and <1ms widget repaints.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                None,
            )?;
        }
        let d3d_device = d3d_device.expect("D3D11 WARP device created");
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        let dxgi_factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))? };

        let d2d_factory: ID2D1Factory1 = unsafe {
            D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?
        };
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device)? };
        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi_device)? };

        let dwrite: IDWriteFactory5 = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let (font_loader, inter_collection) = Self::load_embedded_inter(&dwrite);

        let round_stroke = unsafe {
            d2d_factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES1 {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 10.0,
                    ..Default::default()
                },
                None,
            )?
        };

        Ok(Rc::new(Self {
            _d3d_device: d3d_device,
            dxgi_factory,
            d2d_factory,
            d2d_device,
            dcomp_device,
            dwrite,
            _font_loader: font_loader,
            inter_collection,
            round_stroke: round_stroke.into(),
            svg_cache: RefCell::new(HashMap::new()),
            text_fmt_cache: RefCell::new(HashMap::new()),
            wallpaper_path: String::new(),
            wic: RefCell::new(None),
            backdrop: BackdropManager::new(),
        }))
    }

    fn load_embedded_inter(
        dwrite: &IDWriteFactory5,
    ) -> (
        Option<IDWriteInMemoryFontFileLoader>,
        Option<IDWriteFontCollection>,
    ) {
        unsafe {
            let Ok(loader) = dwrite.CreateInMemoryFontFileLoader() else {
                return (None, None);
            };
            if dwrite.RegisterFontFileLoader(&loader).is_err() {
                return (None, None);
            }
            let Ok(builder) = dwrite.CreateFontSetBuilder() else {
                return (Some(loader), None);
            };
            for bytes in [INTER_REGULAR, INTER_MEDIUM, INTER_SEMIBOLD, INTER_BOLD] {
                if let Ok(file) = loader.CreateInMemoryFontFileReference(
                    dwrite,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    None,
                ) {
                    let _ = builder.AddFontFile(&file);
                }
            }
            let Ok(font_set) = builder.CreateFontSet() else {
                return (Some(loader), None);
            };
            let Ok(coll1): Result<IDWriteFontCollection1> =
                dwrite.CreateFontCollectionFromFontSet(&font_set)
            else {
                return (Some(loader), None);
            };
            let coll: Option<IDWriteFontCollection> = coll1.cast().ok();
            (Some(loader), coll)
        }
    }

    pub fn text_format(
        &self,
        font_family: &str,
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        h_align: DWRITE_TEXT_ALIGNMENT,
        v_align: DWRITE_PARAGRAPH_ALIGNMENT,
        wrap: bool,
    ) -> Result<IDWriteTextFormat> {
        // Prevent spindly hairline `Segoe UI Light` rendering; clamp minimum weight to Regular (400)
        // for Segoe UI so typography remains crisp and legible over translucent glass.
        let eff_weight = if font_family.eq_ignore_ascii_case("Segoe UI") && weight.0 < 400 {
            windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL
        } else {
            weight
        };
        let size_key = (size.max(4.0) * 10.0).round() as u32;
        let key = (
            font_family.to_string(),
            size_key,
            eff_weight.0,
            h_align.0,
            v_align.0,
            wrap,
        );
        if let Some(fmt) = self.text_fmt_cache.borrow().get(&key) {
            return Ok(fmt.clone());
        }
        let use_inter =
            font_family.eq_ignore_ascii_case("Inter") && self.inter_collection.is_some();
        let family_w = to_wide(if use_inter { "Inter" } else { font_family });
        let locale_w = to_wide("en-us");
        let fmt = unsafe {
            self.dwrite.CreateTextFormat(
                PCWSTR(family_w.as_ptr()),
                if use_inter {
                    self.inter_collection.as_ref()
                } else {
                    None
                },
                eff_weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size.max(4.0),
                PCWSTR(locale_w.as_ptr()),
            )?
        };
        unsafe {
            fmt.SetTextAlignment(h_align)?;
            fmt.SetParagraphAlignment(v_align)?;
            fmt.SetWordWrapping(if wrap {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            })?;
        }
        self.text_fmt_cache.borrow_mut().insert(key, fmt.clone());
        Ok(fmt)
    }

    pub fn get_svg(&self, path: &str) -> Option<(ID2D1PathGeometry, D2D_RECT_F)> {
        if path.is_empty() {
            return None;
        }
        if let Some(hit) = self.svg_cache.borrow().get(path) {
            return Some(hit.clone());
        }
        let geom = parse_svg_path(&self.d2d_factory, path).ok()?;
        let bounds = unsafe { geom.GetBounds(None).ok()? };
        self.svg_cache
            .borrow_mut()
            .insert(path.to_string(), (geom.clone(), bounds));
        Some((geom, bounds))
    }

    pub fn load_wallpaper_bitmap(&self, dc: &ID2D1DeviceContext) -> Option<ID2D1Bitmap> {
        let wp = if self.wallpaper_path.is_empty() {
            get_wallpaper_path()
        } else {
            self.wallpaper_path.clone()
        };
        if wp.is_empty() {
            return None;
        }
        let wic = {
            let mut slot = self.wic.borrow_mut();
            if slot.is_none() {
                *slot = unsafe {
                    CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_INPROC_SERVER).ok()
                };
            }
            slot.clone()?
        };
        let path_w = to_wide(&wp);
        unsafe {
            let decoder = wic
                .CreateDecoderFromFilename(
                    PCWSTR(path_w.as_ptr()),
                    None,
                    windows::Win32::Foundation::GENERIC_READ,
                    WICDecodeMetadataCacheOnLoad,
                )
                .ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let scaler = wic.CreateBitmapScaler().ok()?;
            let mut w = 0u32;
            let mut h = 0u32;
            frame.GetSize(&mut w, &mut h).ok()?;
            if w == 0 || h == 0 {
                return None;
            }
            let target_h = 180u32;
            let target_w = ((w as f64 / h as f64) * target_h as f64).round().max(1.0) as u32;
            scaler
                .Initialize(
                    &frame,
                    target_w,
                    target_h,
                    windows::Win32::Graphics::Imaging::WICBitmapInterpolationModeFant,
                )
                .ok()?;
            let converter = wic.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &scaler,
                    &GUID_WICPixelFormat32bppPBGRA,
                    windows::Win32::Graphics::Imaging::WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    windows::Win32::Graphics::Imaging::WICBitmapPaletteTypeCustom,
                )
                .ok()?;
            let bmp1 = dc.CreateBitmapFromWicBitmap(&converter, None).ok()?;
            bmp1.cast().ok()
        }
    }
}

/// Per-window DirectComposition + Direct2D swapchain surface with premultiplied per-pixel alpha.
pub struct WindowSurface {
    pub gfx: Rc<SharedGraphics>,
    pub dc: ID2D1DeviceContext,
    pub swapchain: IDXGISwapChain1,
    pub _target: IDCompositionTarget,
    pub _visual: IDCompositionVisual,
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
}

impl WindowSurface {
    pub fn new(
        gfx: Rc<SharedGraphics>,
        hwnd: HWND,
        width: u32,
        height: u32,
        dpi: f32,
    ) -> Result<Self> {
        let w = width.max(1);
        let h = height.max(1);
        let dc = unsafe {
            gfx.d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
        };
        unsafe { dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE) };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: w,
            Height: h,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: 0,
        };
        let swapchain = unsafe {
            gfx.dxgi_factory
                .CreateSwapChainForComposition(&gfx._d3d_device, &desc, None)?
        };
        let target = unsafe { gfx.dcomp_device.CreateTargetForHwnd(hwnd, true)? };
        let visual = unsafe { gfx.dcomp_device.CreateVisual()? };
        unsafe {
            visual.SetContent(&swapchain)?;
            target.SetRoot(&visual)?;
            gfx.dcomp_device.Commit()?;
        }

        let mut surf = Self {
            gfx,
            dc,
            swapchain,
            _target: target,
            _visual: visual,
            width: w,
            height: h,
            dpi,
        };
        surf.bind_backbuffer()?;
        Ok(surf)
    }

    fn bind_backbuffer(&mut self) -> Result<()> {
        unsafe {
            let backbuffer: IDXGISurface = self.swapchain.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: self.dpi,
                dpiY: self.dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let target_bmp: ID2D1Bitmap1 = self
                .dc
                .CreateBitmapFromDxgiSurface(&backbuffer, Some(&props))?;
            self.dc.SetTarget(&target_bmp);
            self.dc.SetDpi(self.dpi, self.dpi);
        }
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32, dpi: f32) -> Result<()> {
        let w = width.max(1);
        let h = height.max(1);
        if w == self.width && h == self.height && (dpi - self.dpi).abs() < 0.1 {
            return Ok(());
        }
        self.width = w;
        self.height = h;
        self.dpi = dpi;
        unsafe {
            self.dc.SetTarget(None);
            self.swapchain.ResizeBuffers(
                2,
                w,
                h,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        self.bind_backbuffer()
    }

    pub fn begin(&self) {
        unsafe {
            self.dc.BeginDraw();
            self.dc.SetTransform(&Matrix3x2::identity());
            self.dc.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));
        }
    }

    pub fn end(&self) -> Result<()> {
        unsafe {
            self.dc.EndDraw(None, None)?;
            self.swapchain.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        Ok(())
    }
}

// --- High-level Direct2D Drawing Primitives ---

pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

pub fn brush(dc: &ID2D1DeviceContext, color: D2D1_COLOR_F) -> Result<ID2D1SolidColorBrush> {
    unsafe { dc.CreateSolidColorBrush(&color, None) }
}

pub fn rect(l: f32, t: f32, r: f32, b: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}

pub fn fill_round_rect(dc: &ID2D1DeviceContext, r: D2D_RECT_F, radius: f32, color: D2D1_COLOR_F) {
    if let Ok(b) = brush(dc, color) {
        unsafe {
            dc.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: r,
                    radiusX: radius,
                    radiusY: radius,
                },
                &b,
            );
        }
    }
}

pub fn draw_round_rect(
    dc: &ID2D1DeviceContext,
    r: D2D_RECT_F,
    radius: f32,
    color: D2D1_COLOR_F,
    stroke: f32,
) {
    if let Ok(b) = brush(dc, color) {
        unsafe {
            dc.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: r,
                    radiusX: radius,
                    radiusY: radius,
                },
                &b,
                stroke,
                None,
            );
        }
    }
}

pub fn fill_circle(dc: &ID2D1DeviceContext, cx: f32, cy: f32, r: f32, color: D2D1_COLOR_F) {
    if let Ok(b) = brush(dc, color) {
        unsafe {
            dc.FillEllipse(
                &D2D1_ELLIPSE {
                    point: D2D_POINT_2F { x: cx, y: cy },
                    radiusX: r,
                    radiusY: r,
                },
                &b,
            );
        }
    }
}

pub fn draw_circle(
    dc: &ID2D1DeviceContext,
    cx: f32,
    cy: f32,
    r: f32,
    color: D2D1_COLOR_F,
    stroke: f32,
) {
    if let Ok(b) = brush(dc, color) {
        unsafe {
            dc.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: D2D_POINT_2F { x: cx, y: cy },
                    radiusX: r,
                    radiusY: r,
                },
                &b,
                stroke,
                None,
            );
        }
    }
}

/// Draws a circular arc with round caps (`start_deg` and `sweep_deg` clockwise from 12 o'clock = -90 deg standard).
#[allow(clippy::too_many_arguments)]
pub fn draw_arc(
    gfx: &SharedGraphics,
    dc: &ID2D1DeviceContext,
    cx: f32,
    cy: f32,
    radius: f32,
    start_deg_from_top: f32,
    sweep_deg: f32,
    stroke: f32,
    color: D2D1_COLOR_F,
) {
    if sweep_deg.abs() < 0.1 {
        return;
    }
    if sweep_deg >= 359.5 {
        draw_circle(dc, cx, cy, radius, color, stroke);
        return;
    }
    let start_rad = (start_deg_from_top - 90.0).to_radians();
    let end_rad = (start_deg_from_top + sweep_deg - 90.0).to_radians();
    let p0 = D2D_POINT_2F {
        x: cx + radius * start_rad.cos(),
        y: cy + radius * start_rad.sin(),
    };
    let p1 = D2D_POINT_2F {
        x: cx + radius * end_rad.cos(),
        y: cy + radius * end_rad.sin(),
    };
    if let Ok(geom) = unsafe { gfx.d2d_factory.CreatePathGeometry() }
        && let Ok(sink) = unsafe { geom.Open() }
    {
        unsafe {
            sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: p1,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if sweep_deg > 180.0 {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            if let Ok(b) = brush(dc, color) {
                dc.DrawGeometry(&geom, &b, stroke, &gfx.round_stroke);
            }
        }
    }
}

/// Draws a filled SVG icon scaled & centered inside `target_rect`.
pub fn draw_svg_icon(
    gfx: &SharedGraphics,
    dc: &ID2D1DeviceContext,
    svg_path: &str,
    target: D2D_RECT_F,
    color: D2D1_COLOR_F,
) {
    let Some((geom, bounds)) = gfx.get_svg(svg_path) else {
        return;
    };
    let bw = (bounds.right - bounds.left).max(0.001);
    let bh = (bounds.bottom - bounds.top).max(0.001);
    let tw = (target.right - target.left).max(0.001);
    let th = (target.bottom - target.top).max(0.001);
    let scale = (tw / bw).min(th / bh);
    let dx = target.left + (tw - bw * scale) * 0.5 - bounds.left * scale;
    let dy = target.top + (th - bh * scale) * 0.5 - bounds.top * scale;

    let mut prev = Matrix3x2::identity();
    unsafe { dc.GetTransform(&mut prev) };
    let local = Matrix3x2 {
        M11: scale,
        M12: 0.0,
        M21: 0.0,
        M22: scale,
        M31: dx,
        M32: dy,
    };
    unsafe {
        dc.SetTransform(&(local * prev));
        if let Ok(b) = brush(dc, color) {
            dc.FillGeometry(&geom, &b, None);
        }
        dc.SetTransform(&prev);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text_shadowed(
    gfx: &SharedGraphics,
    dc: &ID2D1DeviceContext,
    text: &str,
    r: D2D_RECT_F,
    font_family: &str,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    h_align: DWRITE_TEXT_ALIGNMENT,
    v_align: DWRITE_PARAGRAPH_ALIGNMENT,
    color: D2D1_COLOR_F,
    shadow_color: Option<D2D1_COLOR_F>,
    wrap: bool,
) {
    if text.is_empty() {
        return;
    }
    if let Some(sc) = shadow_color {
        let sr = rect(r.left, r.top + 1.0, r.right, r.bottom + 1.0);
        draw_text(
            gfx,
            dc,
            text,
            sr,
            font_family,
            size,
            weight,
            h_align,
            v_align,
            sc,
            wrap,
        );
    }
    draw_text(
        gfx,
        dc,
        text,
        r,
        font_family,
        size,
        weight,
        h_align,
        v_align,
        color,
        wrap,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    gfx: &SharedGraphics,
    dc: &ID2D1DeviceContext,
    text: &str,
    r: D2D_RECT_F,
    font_family: &str,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
    h_align: DWRITE_TEXT_ALIGNMENT,
    v_align: DWRITE_PARAGRAPH_ALIGNMENT,
    color: D2D1_COLOR_F,
    wrap: bool,
) {
    if text.is_empty() {
        return;
    }
    if let Ok(fmt) = gfx.text_format(font_family, size, weight, h_align, v_align, wrap)
        && let Ok(b) = brush(dc, color)
    {
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            dc.DrawText(
                &wide,
                &fmt,
                &r,
                &b,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}
