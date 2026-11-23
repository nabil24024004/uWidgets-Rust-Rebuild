use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::i18n::two_letter_lang;
use crate::models::ForecastModel;
use crate::ui::WM_APP_WEATHER_READY;
use crate::weather_rs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastResponse {
    pub current: Option<CurrentForecast>,
    pub hourly: Option<HourlyForecast>,
    pub daily: Option<DailyForecast>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentForecast {
    pub temperature_2m: f64,
    pub weathercode: i32,
    pub surface_pressure: f64,
    #[serde(default)]
    pub apparent_temperature: Option<f64>,
    #[serde(default)]
    pub relative_humidity_2m: Option<f64>,
    #[serde(default)]
    pub wind_speed_10m: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyForecast {
    pub temperature_2m: Vec<f64>,
    pub uv_index: Vec<f64>,
    pub weathercode: Vec<i32>,
    pub is_day: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    pub time: Vec<String>,
    pub temperature_2m_min: Vec<f64>,
    pub temperature_2m_max: Vec<f64>,
    pub weathercode: Vec<i32>,
    pub sunrise: Vec<String>,
    pub sunset: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AirQualityResponse {
    pub current: Option<CurrentAirQuality>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurrentAirQuality {
    pub european_aqi: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeocodingResponse {
    pub results: Option<Vec<GeoCity>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeoCity {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
}

impl GeoCity {
    pub fn search_name(&self) -> String {
        match &self.country {
            Some(c) if !c.is_empty() => format!("{}, {c}", self.name),
            _ => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WeatherCacheEntry {
    pub forecast: Option<ForecastResponse>,
    pub aqi: Option<f64>,
    pub resolved_lat: f64,
    pub resolved_lon: f64,
    pub fetched_at: Instant,
    pub fetched_unix: i64,
    pub in_flight: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskWeatherEntry {
    forecast: Option<ForecastResponse>,
    aqi: Option<f64>,
    resolved_lat: f64,
    resolved_lon: f64,
    fetched_unix: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DiskWeatherCache {
    #[serde(default)]
    entries: HashMap<String, DiskWeatherEntry>,
    #[serde(default)]
    geo_cache: HashMap<String, (f64, f64)>,
}

struct WeatherInner {
    entries: HashMap<String, WeatherCacheEntry>,
    geo_cache: HashMap<String, (f64, f64)>,
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn build_fast_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(8))
        .timeout_write(Duration::from_secs(5))
        .build()
}

fn notify_ui(hwnd_raw: isize) {
    if hwnd_raw != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(hwnd_raw as *mut _),
                WM_APP_WEATHER_READY,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

fn save_disk_cache(dir: &Path, inner: &WeatherInner) {
    let mut disk = DiskWeatherCache {
        entries: HashMap::with_capacity(inner.entries.len()),
        geo_cache: inner.geo_cache.clone(),
    };
    for (k, v) in &inner.entries {
        if v.forecast.is_some() || v.aqi.is_some() {
            disk.entries.insert(
                k.clone(),
                DiskWeatherEntry {
                    forecast: v.forecast.clone(),
                    aqi: v.aqi,
                    resolved_lat: v.resolved_lat,
                    resolved_lon: v.resolved_lon,
                    fetched_unix: v.fetched_unix,
                },
            );
        }
    }
    if let Ok(json) = serde_json::to_string(&disk) {
        let path = dir.join("weatherCache.json");
        let tmp = dir.join("weatherCache.json.tmp");
        if fs::write(&tmp, json.as_bytes()).is_ok() && fs::rename(&tmp, &path).is_err() {
            let _ = fs::write(&path, json.as_bytes());
            let _ = fs::remove_file(&tmp);
        }
    }
}

fn load_disk_cache(dir: &Path) -> WeatherInner {
    let path = dir.join("weatherCache.json");
    let now_u = now_unix_secs();
    let now_inst = Instant::now();
    let mut inner = WeatherInner {
        entries: HashMap::new(),
        geo_cache: HashMap::new(),
    };
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(disk) = serde_json::from_str::<DiskWeatherCache>(&text)
    {
        inner.geo_cache = disk.geo_cache;
        for (k, d) in disk.entries {
            let age_secs = (now_u - d.fetched_unix).max(0) as u64;
            let fetched_at = now_inst
                .checked_sub(Duration::from_secs(age_secs.min(86_400)))
                .unwrap_or(now_inst);
            inner.entries.insert(
                k,
                WeatherCacheEntry {
                    forecast: d.forecast,
                    aqi: d.aqi,
                    resolved_lat: d.resolved_lat,
                    resolved_lon: d.resolved_lon,
                    fetched_at,
                    fetched_unix: d.fetched_unix,
                    in_flight: false,
                    last_error: None,
                },
            );
        }
    }
    inner
}

#[derive(Clone)]
pub struct WeatherService {
    inner: Arc<Mutex<WeatherInner>>,
    agent: Arc<ureq::Agent>,
    config_dir: PathBuf,
    notify_hwnd: isize,
}

impl WeatherService {
    pub fn new(config_dir: PathBuf, notify_hwnd: HWND) -> Self {
        let inner = load_disk_cache(&config_dir);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            agent: Arc::new(build_fast_agent()),
            config_dir,
            notify_hwnd: notify_hwnd.0 as isize,
        }
    }

    fn key(model: &ForecastModel) -> String {
        format!(
            "{}|{:.4}|{:.4}|{}",
            model.name.trim(),
            model.latitude,
            model.longitude,
            model.temperature_unit.to_ascii_lowercase()
        )
    }

    pub fn peek_entry(&self, model: &ForecastModel) -> Option<WeatherCacheEntry> {
        let k = Self::key(model);
        self.inner.lock().ok()?.entries.get(&k).cloned()
    }

    pub fn get_or_fetch(
        &self,
        model: &ForecastModel,
        lang: &str,
        ttl_secs: u64,
        force: bool,
    ) -> Option<WeatherCacheEntry> {
        let k = Self::key(model);
        let ttl = Duration::from_secs(ttl_secs.clamp(60, 86_400));
        let mut guard = self.inner.lock().ok()?;
        let cached_geo = guard
            .geo_cache
            .get(&model.name.trim().to_ascii_lowercase())
            .copied();

        let needs_fetch = match guard.entries.get(&k) {
            None => true,
            Some(e) => {
                if e.in_flight {
                    false
                } else if force {
                    true
                } else if e.forecast.is_none() {
                    // Retry failed/empty fetches after a short 15s backoff instead of waiting 1 hour!
                    e.fetched_at.elapsed() >= Duration::from_secs(15)
                } else {
                    e.fetched_at.elapsed() >= ttl
                }
            }
        };

        if needs_fetch {
            let entry = guard
                .entries
                .entry(k.clone())
                .or_insert_with(|| WeatherCacheEntry {
                    forecast: None,
                    aqi: None,
                    resolved_lat: model.latitude,
                    resolved_lon: model.longitude,
                    fetched_at: Instant::now(),
                    fetched_unix: 0,
                    in_flight: true,
                    last_error: None,
                });
            entry.in_flight = true;

            let inner_arc = Arc::clone(&self.inner);
            let agent = Arc::clone(&self.agent);
            let m = model.clone();
            let lang2 = two_letter_lang(lang).to_string();
            let config_dir = self.config_dir.clone();
            let hwnd_raw = self.notify_hwnd;

            thread::spawn(move || {
                let mut lat = m.latitude;
                let mut lon = m.longitude;

                // Resolve (0, 0) city coordinates once and persist in geo_cache!
                if lat.abs() < 1e-6 && lon.abs() < 1e-6 && !m.name.trim().is_empty() {
                    if let Some((glat, glon)) = cached_geo {
                        lat = glat;
                        lon = glon;
                    } else if let Ok(cities) = search_cities_with_agent(&agent, &m.name, &lang2)
                        && let Some(first) = cities.first()
                    {
                        lat = first.latitude;
                        lon = first.longitude;
                        if let Ok(mut st) = inner_arc.lock() {
                            st.geo_cache
                                .insert(m.name.trim().to_ascii_lowercase(), (lat, lon));
                        }
                    }
                }

                // Extend Open-Meteo current fields in the same single HTTP call (0 extra latency!)
                let base_url =
                    weather_rs::port::build_forecast_url(lat, lon, m.temperature_unit.clone())
                        .unwrap_or_default();
                let forecast_url = base_url.replace(
                    "current=temperature_2m,weathercode,surface_pressure",
                    "current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weathercode,surface_pressure",
                );

                let (forecast, err_msg) = match agent.get(&forecast_url).call() {
                    Ok(resp) => match resp.into_json::<ForecastResponse>() {
                        Ok(parsed) => (Some(parsed), None),
                        Err(err) => (None, Some(format!("Parse error: {err}"))),
                    },
                    Err(err) => {
                        let _ = fs::write(
                            config_dir.join("weather_crash_log.txt"),
                            format!("{err}\nURL: {forecast_url}"),
                        );
                        (None, Some(format!("Network error: {err}")))
                    }
                };

                // Immediately commit forecast & wake UI without blocking on AQI!
                if let Ok(mut st) = inner_arc.lock() {
                    let e = st
                        .entries
                        .entry(k.clone())
                        .or_insert_with(|| WeatherCacheEntry {
                            forecast: None,
                            aqi: None,
                            resolved_lat: lat,
                            resolved_lon: lon,
                            fetched_at: Instant::now(),
                            fetched_unix: 0,
                            in_flight: false,
                            last_error: None,
                        });
                    if let Some(fc) = forecast {
                        e.forecast = Some(fc);
                        e.fetched_unix = now_unix_secs();
                        e.last_error = None;
                    } else if let Some(msg) = err_msg {
                        e.last_error = Some(msg);
                    }
                    e.resolved_lat = lat;
                    e.resolved_lon = lon;
                    e.fetched_at = Instant::now();
                    e.in_flight = false;
                    save_disk_cache(&config_dir, &st);
                }
                notify_ui(hwnd_raw);

                // Secondary non-blocking AQI fetch for AirQuality metric widgets
                let aqi_url = weather_rs::port::build_air_quality_url(lat, lon).unwrap_or_default();
                if !aqi_url.is_empty()
                    && let Ok(resp) = agent.get(&aqi_url).call()
                    && let Ok(parsed) = resp.into_json::<AirQualityResponse>()
                    && let Some(aqi) = parsed.current.and_then(|c| c.european_aqi)
                {
                    if let Ok(mut st) = inner_arc.lock() {
                        if let Some(e) = st.entries.get_mut(&k) {
                            e.aqi = Some(aqi);
                        }
                        save_disk_cache(&config_dir, &st);
                    }
                    notify_ui(hwnd_raw);
                }
            });
        }
        guard.entries.get(&Self::key(model)).cloned()
    }
}

fn search_cities_with_agent(
    agent: &ureq::Agent,
    query: &str,
    two_letter_lang: &str,
) -> Result<Vec<GeoCity>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let encoded: String = query
        .trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "%20".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = weather_rs::port::build_geocoding_url(encoded, two_letter_lang.to_string())?;
    let resp = agent.get(&url).call().map_err(|e| e.to_string())?;
    let parsed: GeocodingResponse = resp.into_json().map_err(|e| e.to_string())?;
    Ok(parsed.results.unwrap_or_default())
}

pub fn search_cities_sync(query: &str, two_letter_lang: &str) -> Result<Vec<GeoCity>, String> {
    let agent = build_fast_agent();
    search_cities_with_agent(&agent, query, two_letter_lang)
}

#[derive(Deserialize)]
struct IpWhoIsResp {
    success: Option<bool>,
    city: Option<String>,
    country: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize)]
struct IpApiResp {
    status: Option<String>,
    city: Option<String>,
    country: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

/// Fast IP-based auto-location detection for the Settings -> Weather "Auto-Detect" button.
pub fn detect_current_location_sync() -> Result<GeoCity, String> {
    let agent = build_fast_agent();
    if let Ok(resp) = agent.get("https://ipwho.is/").call()
        && let Ok(parsed) = resp.into_json::<IpWhoIsResp>()
        && parsed.success.unwrap_or(false)
        && let (Some(city), Some(lat), Some(lon)) = (parsed.city, parsed.latitude, parsed.longitude)
        && !city.trim().is_empty()
    {
        return Ok(GeoCity {
            name: city,
            latitude: lat,
            longitude: lon,
            country: parsed.country,
        });
    }
    let resp = agent
        .get("http://ip-api.com/json/?fields=status,city,country,lat,lon")
        .call()
        .map_err(|e| e.to_string())?;
    let parsed: IpApiResp = resp.into_json().map_err(|e| e.to_string())?;
    if parsed.status.as_deref() == Some("success")
        && let (Some(city), Some(lat), Some(lon)) = (parsed.city, parsed.lat, parsed.lon)
    {
        return Ok(GeoCity {
            name: city,
            latitude: lat,
            longitude: lon,
            country: parsed.country,
        });
    }
    Err("Could not detect current location".to_string())
}
