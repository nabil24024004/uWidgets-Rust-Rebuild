use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core_rs;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub templates: Vec<Theme>,
    pub layout: LayoutConfig,
    pub dimensions: Dimensions,
    pub region: Region,
    pub run_on_startup: bool,
    pub ignore_update: Option<String>,
    #[serde(default)]
    pub liquid_glass: LiquidGlassConfig,
    #[serde(default)]
    pub weather: WeatherConfig,
    #[serde(default)]
    pub updates: UpdateConfig,
}

fn default_neosparkx_manifest_url() -> String {
    "https://neosparkx.com/releases/uwidgets/latest.json".to_string()
}
fn default_store_product_id() -> Option<String> {
    Some("9NDGKTPBF48G".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateConfig {
    /// Automatically check for updates on launch & periodically in the background.
    #[serde(default = "default_true")]
    pub auto_check: bool,
    /// Automatically download and install updates (Microsoft Store silent update or self-update).
    #[serde(default = "default_true")]
    pub auto_install: bool,
    /// NeosparkX website JSON release manifest URL (checked alongside GitHub Releases).
    #[serde(default = "default_neosparkx_manifest_url")]
    pub neosparkx_manifest_url: String,
    /// Microsoft Store 12-char Product ID (`9NDGKTPBF48G`) for Store deep-links.
    #[serde(default = "default_store_product_id")]
    pub store_product_id: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            auto_install: true,
            neosparkx_manifest_url: default_neosparkx_manifest_url(),
            store_product_id: default_store_product_id(),
        }
    }
}

fn default_weather_city() -> String {
    "Cupertino".to_string()
}
fn default_temp_unit() -> String {
    "celsius".to_string()
}
fn default_wind_unit() -> String {
    "kmh".to_string()
}
fn default_refresh_minutes() -> i32 {
    15
}
fn default_hourly_count() -> i32 {
    6
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WeatherConfig {
    #[serde(default = "default_weather_city")]
    pub default_city: String,
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
    #[serde(default = "default_temp_unit")]
    pub temperature_unit: String,
    #[serde(default = "default_wind_unit")]
    pub wind_unit: String,
    #[serde(default = "default_refresh_minutes")]
    pub refresh_minutes: i32,
    #[serde(default = "default_hourly_count")]
    pub hourly_count: i32,
    #[serde(default = "default_true")]
    pub show_high_low: bool,
    #[serde(default = "default_true")]
    pub show_condition: bool,
    #[serde(default)]
    pub show_feels_like: bool,
    #[serde(default)]
    pub show_humidity_wind: bool,
    #[serde(default = "default_true")]
    pub use_24_hour_clock: bool,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            default_city: default_weather_city(),
            latitude: 37.323,
            longitude: -122.0322,
            temperature_unit: default_temp_unit(),
            wind_unit: default_wind_unit(),
            refresh_minutes: default_refresh_minutes(),
            hourly_count: default_hourly_count(),
            show_high_low: true,
            show_condition: true,
            show_feels_like: false,
            show_humidity_wind: false,
            use_24_hour_clock: true,
        }
    }
}

impl WeatherConfig {
    pub fn to_forecast_model(&self) -> ForecastModel {
        ForecastModel {
            name: self.default_city.clone(),
            latitude: self.latitude,
            longitude: self.longitude,
            temperature_unit: self.temperature_unit.clone(),
        }
    }
}

fn default_lg_blur() -> i32 {
    15
}
fn default_lg_one() -> f64 {
    1.0
}
fn default_lg_vibrancy() -> f64 {
    1.30
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LiquidGlassConfig {
    /// Gaussian frost blur radius in half-res px (0..30, default 15 => ~30px full-screen).
    #[serde(default = "default_lg_blur")]
    pub blur_radius: i32,
    /// Curved glass bevel lens refraction multiplier (0.0..2.0, default 1.0).
    #[serde(default = "default_lg_one")]
    pub refraction: f64,
    /// Curved glass bevel width multiplier (0.3..1.8, default 1.0).
    #[serde(default = "default_lg_one")]
    pub bevel_width: f64,
    /// RGB chromatic dispersion / prism edge fringing multiplier (0.0..2.5, default 1.0).
    #[serde(default = "default_lg_one")]
    pub dispersion: f64,
    /// Dual-angle Fresnel specular rim brightness multiplier (0.0..2.0, default 1.0).
    #[serde(default = "default_lg_one")]
    pub specular: f64,
    /// Backdrop vibrancy chroma saturation multiplier (0.5..2.0, default 1.30).
    #[serde(default = "default_lg_vibrancy")]
    pub vibrancy: f64,
    /// Tactile frost / acrylic micro-grain intensity multiplier (0.0..2.5, default 1.0).
    #[serde(default = "default_lg_one")]
    pub frost_grain: f64,
    /// Optional glass dye tint override hex (e.g. "#4A78FF"); None uses automatic dark/light glass.
    #[serde(default)]
    pub tint_color: Option<String>,
}

impl Default for LiquidGlassConfig {
    fn default() -> Self {
        Self {
            blur_radius: default_lg_blur(),
            refraction: 1.0,
            bevel_width: 1.0,
            dispersion: 1.0,
            specular: 1.0,
            vibrancy: default_lg_vibrancy(),
            frost_grain: 1.0,
            tint_color: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Theme {
    pub dark_mode: Option<bool>,
    pub accent_color: Option<String>,
    pub opacity_level: f64,
    pub monochrome: bool,
    pub use_native_frame: bool,
    pub font_family: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutConfig {
    pub snap_size: bool,
    pub lock_size: bool,
    pub snap_position: bool,
    pub lock_position: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Dimensions {
    pub size: i32,
    pub margin: i32,
    pub radius: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Region {
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WidgetLayout {
    #[serde(rename = "Type")]
    pub widget_type: String,
    pub sub_type: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub settings: Option<Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme {
                dark_mode: Some(true),
                accent_color: None,
                opacity_level: 0.45,
                monochrome: false,
                use_native_frame: true,
                font_family: "Inter".to_string(),
            },
            templates: default_templates(),
            layout: LayoutConfig {
                snap_size: true,
                lock_size: false,
                snap_position: true,
                lock_position: false,
            },
            dimensions: Dimensions {
                size: 72,
                margin: 16,
                radius: 24,
            },
            region: Region {
                language: "en".to_string(),
            },
            run_on_startup: false,
            ignore_update: None,
            liquid_glass: LiquidGlassConfig::default(),
            weather: WeatherConfig::default(),
            updates: UpdateConfig::default(),
        }
    }
}

pub fn default_templates() -> Vec<Theme> {
    vec![
        Theme {
            dark_mode: Some(false),
            accent_color: None,
            opacity_level: 0.4,
            monochrome: true,
            use_native_frame: false,
            font_family: "Inter".to_string(),
        },
        Theme {
            dark_mode: Some(false),
            accent_color: None,
            opacity_level: 0.4,
            monochrome: true,
            use_native_frame: true,
            font_family: "Segoe UI".to_string(),
        },
        Theme {
            dark_mode: Some(false),
            accent_color: None,
            opacity_level: 1.0,
            monochrome: true,
            use_native_frame: false,
            font_family: "Inter".to_string(),
        },
        Theme {
            dark_mode: Some(false),
            accent_color: None,
            opacity_level: 1.0,
            monochrome: true,
            use_native_frame: true,
            font_family: "Segoe UI".to_string(),
        },
        Theme {
            dark_mode: Some(true),
            accent_color: None,
            opacity_level: 0.4,
            monochrome: true,
            use_native_frame: false,
            font_family: "Inter".to_string(),
        },
        Theme {
            dark_mode: Some(true),
            accent_color: None,
            opacity_level: 0.4,
            monochrome: true,
            use_native_frame: true,
            font_family: "Segoe UI".to_string(),
        },
        Theme {
            dark_mode: Some(true),
            accent_color: None,
            opacity_level: 1.0,
            monochrome: true,
            use_native_frame: false,
            font_family: "Inter".to_string(),
        },
        Theme {
            dark_mode: Some(true),
            accent_color: None,
            opacity_level: 1.0,
            monochrome: true,
            use_native_frame: true,
            font_family: "Segoe UI".to_string(),
        },
    ]
}

// --- Widget-specific settings models (exact PascalCase JSON schema parity) ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MonthCalendarModel {
    pub first_day_of_week: i32, // 0=Sunday .. 6=Saturday, default 1=Monday
}

impl Default for MonthCalendarModel {
    fn default() -> Self {
        Self {
            first_day_of_week: 1,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClockModel {
    #[serde(default = "default_true")]
    pub show_seconds: bool,
    #[serde(default)]
    pub show_date: bool,
    #[serde(default)]
    pub use_24_hours: bool,
    pub time_zone_id: Option<String>,
}

impl Default for ClockModel {
    fn default() -> Self {
        Self {
            show_seconds: true,
            show_date: false,
            use_24_hours: false,
            time_zone_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldClockModel {
    pub time_zone_ids: Vec<Option<String>>,
}

impl Default for WorldClockModel {
    fn default() -> Self {
        Self {
            time_zone_ids: vec![None, None, None, None],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SingleMetricModel {
    pub metric: i32, // 0=Cpu, 1=Ram, 2=Disk, 3=Network, 4=Battery
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NoteModel {
    pub title: Option<String>,
    pub content: Option<String>,
    pub updated: Option<String>, // ISO-8601 DateTime string compatible with System.Text.Json
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ReminderModel {
    pub completed: bool,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RemindersListModel {
    pub list_name: Option<String>,
    pub reminders: Vec<ReminderModel>,
}

impl Default for RemindersListModel {
    fn default() -> Self {
        Self {
            list_name: Some("Reminders".to_string()),
            reminders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ForecastModel {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub temperature_unit: String,
}

impl Default for ForecastModel {
    fn default() -> Self {
        Self {
            name: "Cupertino".to_string(),
            latitude: 37.323,
            longitude: -122.0322,
            temperature_unit: "celsius".to_string(),
        }
    }
}

impl WidgetLayout {
    pub fn parse_settings<T: for<'de> Deserialize<'de> + Default>(&self) -> T {
        self.settings
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    pub fn set_settings<T: Serialize>(&mut self, model: &T) {
        self.settings = serde_json::to_value(model).ok();
    }
}

pub fn config_dir() -> PathBuf {
    // Inside a Microsoft Store / MSIX package, the executable folder (WindowsApps) is read-only!
    // Store all mutable settings, layout, and weather cache cleanly under %LOCALAPPDATA%\uWidgets.
    if crate::updater::is_packaged_msix()
        && let Ok(local) = std::env::var("LOCALAPPDATA")
    {
        let dir = PathBuf::from(local).join("uWidgets");
        let _ = fs::create_dir_all(&dir);
        return dir;
    }

    // Prefer executable directory if appSettings.json exists there, otherwise current dir, otherwise exe dir.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("appSettings.json").exists()
    {
        return dir.to_path_buf();
    }
    if let Ok(cwd) = std::env::current_dir()
        && cwd.join("appSettings.json").exists()
    {
        return cwd;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn load_app_settings(dir: &Path) -> AppSettings {
    let path = dir.join("appSettings.json");
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(parsed) = serde_json::from_str::<AppSettings>(&text)
    {
        return parsed;
    }
    let default = AppSettings::default();
    let _ = save_app_settings(dir, &default);
    default
}

pub fn save_app_settings(dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = dir.join("appSettings.json");
    let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    atomic_write(&path, json.as_bytes())
}

pub fn load_layout(dir: &Path) -> Vec<WidgetLayout> {
    let path = dir.join("layout.json");
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(parsed) = serde_json::from_str::<Vec<WidgetLayout>>(&text)
    {
        return parsed;
    }
    Vec::new()
}

pub fn save_layout(dir: &Path, layout: &[WidgetLayout]) -> Result<(), String> {
    let path = dir.join("layout.json");
    let json = serde_json::to_string(layout).map_err(|e| e.to_string())?;
    atomic_write(&path, json.as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, path).is_err() {
        fs::write(path, bytes).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedTheme {
    pub is_dark: bool,
    pub accent_rgb: (f32, f32, f32),
    pub accent_effective: (f32, f32, f32),
    pub bg_rgb: (f32, f32, f32),
    pub bg_alpha: f32,
    pub fg_rgb: (f32, f32, f32),
    pub fg_muted: (f32, f32, f32),
    pub text_opacity: f32,
    pub uses_acrylic: bool,
    pub monochrome: bool,
    pub use_native_frame: bool,
    pub radius: f32,
}

pub fn parse_hex_color(s: &str) -> Option<(f32, f32, f32)> {
    let hex = s.trim().trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ),
        8 => (
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some((
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

pub fn resolve_theme(
    theme: &Theme,
    dims: &Dimensions,
    system_dark: bool,
    system_accent: (f32, f32, f32),
) -> ResolvedTheme {
    let is_dark = theme.dark_mode.unwrap_or(system_dark);
    let accent_rgb = theme
        .accent_color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(system_accent);

    let bg_rgb = if is_dark {
        (
            0x2E as f32 / 255.0,
            0x2E as f32 / 255.0,
            0x2E as f32 / 255.0,
        )
    } else {
        (1.0, 1.0, 1.0)
    };
    let fg_rgb = if is_dark {
        (1.0, 1.0, 1.0)
    } else {
        (0.08, 0.08, 0.08)
    };
    let fg_muted = if is_dark {
        (0.70, 0.70, 0.72)
    } else {
        (0.42, 0.42, 0.45)
    };
    let text_opacity = core_rs::port::theme_text_opacity(theme.opacity_level).unwrap_or(1.0) as f32;
    let uses_acrylic = core_rs::port::theme_uses_acrylic(theme.opacity_level).unwrap_or(false);
    let accent_effective = if theme.monochrome { fg_rgb } else { accent_rgb };

    ResolvedTheme {
        is_dark,
        accent_rgb,
        accent_effective,
        bg_rgb,
        bg_alpha: (theme.opacity_level as f32).clamp(0.05, 1.0),
        fg_rgb,
        fg_muted,
        text_opacity,
        uses_acrylic,
        monochrome: theme.monochrome,
        use_native_frame: theme.use_native_frame,
        radius: if theme.use_native_frame {
            8.0
        } else {
            dims.radius as f32
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_user_app_settings_and_layout() {
        let settings_json = r#"{"Theme":{"DarkMode":true,"AccentColor":null,"OpacityLevel":0.45,"Monochrome":false,"UseNativeFrame":true,"FontFamily":"Inter"},"Templates":[{"DarkMode":false,"AccentColor":null,"OpacityLevel":0.4,"Monochrome":true,"UseNativeFrame":false,"FontFamily":"Inter"}],"Layout":{"SnapSize":true,"LockSize":true,"SnapPosition":false,"LockPosition":true},"Dimensions":{"Size":72,"Margin":16,"Radius":24},"Region":{"Language":"en"},"RunOnStartup":true,"IgnoreUpdate":null}"#;
        let parsed: AppSettings = serde_json::from_str(settings_json).expect("valid appSettings");
        assert_eq!(parsed.dimensions.size, 72);
        assert_eq!(parsed.theme.font_family, "Inter");
        // Missing LiquidGlass & Weather sections fall back to defaults cleanly
        assert_eq!(parsed.liquid_glass, LiquidGlassConfig::default());
        assert_eq!(parsed.weather, WeatherConfig::default());

        // Custom LiquidGlass + Weather config round-trips cleanly in PascalCase JSON
        let mut custom = parsed.clone();
        custom.liquid_glass.blur_radius = 22;
        custom.liquid_glass.refraction = 1.45;
        custom.liquid_glass.bevel_width = 1.25;
        custom.liquid_glass.dispersion = 1.80;
        custom.liquid_glass.specular = 1.35;
        custom.liquid_glass.vibrancy = 1.55;
        custom.liquid_glass.frost_grain = 0.60;
        custom.liquid_glass.tint_color = Some("#3B82F6".to_string());
        custom.weather.default_city = "Rangpur".to_string();
        custom.weather.latitude = 25.7439;
        custom.weather.longitude = 89.2752;
        custom.weather.temperature_unit = "fahrenheit".to_string();
        custom.weather.wind_unit = "mph".to_string();
        custom.weather.refresh_minutes = 5;
        custom.weather.hourly_count = 8;
        custom.weather.show_feels_like = true;
        custom.weather.show_humidity_wind = true;
        custom.weather.use_24_hour_clock = false;
        let json_out = serde_json::to_string(&custom).expect("serialize");
        assert!(json_out.contains("\"LiquidGlass\""));
        assert!(json_out.contains("\"Weather\""));
        assert!(json_out.contains("\"BlurRadius\":22"));
        assert!(json_out.contains("\"TintColor\":\"#3B82F6\""));
        assert!(json_out.contains("\"DefaultCity\":\"Rangpur\""));
        let roundtrip: AppSettings = serde_json::from_str(&json_out).expect("roundtrip");
        assert_eq!(roundtrip.liquid_glass, custom.liquid_glass);
        assert_eq!(roundtrip.weather, custom.weather);

        let layout_json = r#"[{"Type":"Calendar","SubType":"Date","X":1692,"Y":191,"Width":160,"Height":160,"Settings":null},{"Type":"Clock","SubType":"AnalogIII","X":1478,"Y":192,"Width":160,"Height":160,"Settings":null},{"Type":"Weather","SubType":"Forecast","X":1478,"Y":399,"Width":336,"Height":160,"Settings":{"Name":"Lalmonirhat Airport","Latitude":0,"Longitude":0,"TemperatureUnit":"celsius"}},{"Type":"Calendar","SubType":"Month","X":1478,"Y":605,"Width":336,"Height":160,"Settings":null}]"#;
        let layout: Vec<WidgetLayout> = serde_json::from_str(layout_json).expect("valid layout");
        assert_eq!(layout.len(), 4);
        let fm: ForecastModel = layout[2].parse_settings();
        assert_eq!(fm.name, "Lalmonirhat Airport");
        assert_eq!(fm.temperature_unit, "celsius");
    }
}
