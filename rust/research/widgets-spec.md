# Widgets Specification (All 6 Packs, 16 Subtypes — REA Verified)

## 1. Calendar (`Calendar.dll` v1.0.7.0, REA `ev_4722f030…`)
- **Date (`Calendar / Date`)**:
  - Refresh: `Timer5Minutes`.
  - Fields: `Month = AbbreviatedMonthName.replace(".", "")`, `DayOfWeek = AbbreviatedDayName.replace(".", "")`, `Day = day.to_string()`.
- **Month (`Calendar / Month`)**:
  - Settings (`MonthCalendarModel`): `{ "FirstDayOfWeek": 0..6 }` (default `1` = Monday; 0=Sunday..6=Saturday).
  - Header: `Month = FullMonthName.to_uppercase()`.
  - 7 weekday headers starting at `FirstDayOfWeek` (`ShortestDayName`), weekend flag for `Saturday | Sunday`.
  - Leading empty day count: C# formula `(firstOfMonthDow + firstDayOfWeek + 5) % 7` (see ADR-006 regarding Sunday parity vs Monday).
  - Days `1..=DaysInMonth`, with `IsWeekend` and `IsToday`.
  - Responsive sizing: `TextSize = min(w, h) / 12.0`, `MonthMarginLeft = -min(w, h) / 30.0`.

## 2. Clock (`Clock.dll` v1.1.1.0, REA `ev_3217823d…`)
- **Settings**:
  - `ClockModel`: `{ ShowSeconds: bool (false), ShowDate: bool (false), Use24Hours: bool (false), TimeZoneId: string? (null) }`
  - `WorldClockModel`: `{ TimeZoneIds: [string?; 4] }`
- **AnalogI, AnalogII, AnalogIII (`Clock / AnalogI`, `AnalogII`, `AnalogIII`)**:
  - Refresh: `Timer100Ms` if `ShowSeconds` else `Timer5Seconds`.
  - Dial space: `1000×1000` centered at `(500, 500)`, margin `8px`.
  - Angles (degrees clockwise from 12 o'clock):
    - `HoursAngle = ((hour % 12) + minute / 60.0) * 30.0` (length `190`, `M 500,420 V 230`)
    - `MinutesAngle = (minute + second / 60.0) * 6.0` (length `365`, `M 500,420 V 55`)
    - `SecondsAngle = (second + millis / 1000.0) * 6.0` (length `460`, `M 500,420 V -40`, tail `M 500,520 V 575`)
  - Center pivot ring (`60×60` at `470,470`, stroke 12) + stem `M 500,470 V 436`.
  - `AnalogI`: 1–12 numerals + tick strokes; `AnalogII`: minimalist strokes (`M 500,50 V 150`); `AnalogIII`: dual-ring dial (`520,520,960,960` & `570,570,860,860`).
- **Digital (`Clock / Digital`)**:
  - Refresh: `Timer1Second` if `ShowSeconds` else `Timer1Minute`.
  - Format: `(Use24Hours ? "HH" : "hh") + ":mm" + (ShowSeconds ? ":ss" : "") + (Use24Hours ? "" : " tt")`.
  - Optional localized long date (`DateText`) when `ShowDate == true`.
- **World (`Clock / World`)**:
  - 4 mini analog clocks (`First`..`Fourth`).
  - Responsive breakpoint: `aspect >= 1.5` -> `4×1` horizontal row with `ShowCityName = true` (truncated > 11 chars to `first 9 + "…"`), else `2×2` grid with `ShowCityAbbreviation = true` (3 uppercase letters).
  - Shows relative day (`Yesterday`/`Today`/`Tomorrow`) and `TimeZoneDiff` (`+HH:mm` / `-HH:mm` / `00:00`).

## 3. Monitor (`Monitor.dll` v1.1.0.0, REA `ev_23f031ef…`)
- **SingleMetric (`Monitor / SingleMetric`)**:
  - Refresh: `Timer1Second`.
  - Settings (`SingleMetricModel`): `{ "Metric": 0..4 }` (`0=CpuUsage`, `1=RamUsage`, `2=DiskUsage`, `3=NetworkUsage`, `4=BatteryLevel`).
  - Gauge: `110×110` canvas, circle center `(55, 55)`, radius `50`, stroke thickness `10`, background track opacity `0.2`, `StrokeDashOffset = 31.4 * (1.0 - value)`, text `"{value:P0}"`.
  - Responsive breakpoint: `height < 100` -> hide text & margin `6`, else 2-row grid (ring + text) & margin `12`.
  - Native Windows collectors (replacing WMI): `GetSystemTimes` (CPU), `GlobalMemoryStatusEx` (RAM), PDH `\LogicalDisk(_Total)\% Disk Time` (Disk), `GetIfTable2` bytes/sec vs link speed (Network), `GetSystemPowerStatus` (Battery).

## 4. Notes (`Notes.dll` v1.0.7.0, REA `ev_c55b207c…`)
- **Note (`Notes / Note`)**:
  - Settings (`NoteModel`): `{ "Title": string?, "Content": string?, "Updated": DateTime? }`.
  - Inline editable Title and multi-line Content via click-through double-click editing; edits stamp `Updated = Local::now()` (formatted `g`) and persist immediately to `layout.json`.

## 5. Reminders (`Reminders.dll` v1.1.3.0, REA `ev_3c92ecb7…`)
- **List (`Reminders / List`)**:
  - Settings (`RemindersListModel`): `{ "ListName": string?, "Reminders": [{ "Completed": bool, "Title": string }] }`.
  - Responsive breakpoints:
    - `width < 200 && height < 200` -> `ListSmall`
    - else `aspect >= 1.5` -> `ListWide`
    - else -> `ListLarge`
  - Interactions: edit `ListName`, toggle item `Completed`, edit item title (empty title removes item!), add new item at bottom, Edit dialog buttons `DeleteCompleted` and `DeleteAll`.

## 6. Weather (`Weather.dll` v1.4.0.0, REA `ev_55fb126e…`)
- **Settings (`ForecastModel`)**: `{ "Name": "Cupertino", "Latitude": 37.323, "Longitude": -122.0322, "TemperatureUnit": "celsius" | "fahrenheit" }`.
- **Open-Meteo Endpoints** (lat/lon `{:.4}`, refresh `Timer1Hour`, error log `weather_crash_log.txt`):
  - Forecast: `https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&temperature_unit={unit}&current=temperature_2m,weathercode,surface_pressure&hourly=temperature_2m,weathercode,uv_index,is_day&daily=temperature_2m_min,temperature_2m_max,weathercode,sunrise,sunset&timezone=auto`
  - Air Quality: `https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat:.4}&longitude={lon:.4}&current=european_aqi`
  - Geocoding: `https://geocoding-api.open-meteo.com/v1/search?name={city}&language={lang2}`
- **Subtypes (6)**:
  - `Forecast`: responsive breakpoints: `w > 230 && h > 230` -> `ForecastLarge`, `w > 230 && h > 150` -> `ForecastWide`, `w > 150 && h > 150` -> `ForecastSmall`, else `ForecastTiny`. Shows city, current temp (`{temp:.0}°`), day/night icon, condition string (`Weather_Code_{code}`), daily min/max (`{max:.0}°  {min:.0}°`), 24 hourly slots, 7 daily slots with 3-star temperature range bar `[min - globalMin, max - min, globalMax - max]`.
  - `Temperature`: circular gauge `(dailyMin.min(), dailyMax.max(), currentTemp, icon=None)` displaying min/max.
  - `UVIndex`: circular gauge `(0.0, 10.0, hourlyUV[hour], Clear icon)`.
  - `Pressure`: circular gauge `(960.0, 1050.0, currentPressure, Pressure icon)`.
  - `AirQuality`: circular gauge `(0.0, 100.0, europeanAqi, Wind icon)`.
  - `SunriseSunset`: next upcoming event (`now < sunrise[0]` -> Sunrise[0]; `now < sunset[0]` -> Sunset[0]; else Sunrise[1]) formatted `HH:mm`.
- **Gauge Math (`Weather.MetricViewModel`)**:
  - `DisplayValue = round(value)`
  - `FontSize = len <= 2 ? 50 : 100 / len`
  - `StrokeDashOffset = (clamp(value, min, max) - min) / (max - min) * 21.0`
  - Margin: `min(w, h) >= 150 ? min(w, h) * 0.1 : 6.0`.
