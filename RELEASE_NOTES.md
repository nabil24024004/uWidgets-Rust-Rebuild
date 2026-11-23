# uWidgets (Rust Edition) — v1.0.0 Release Notes

> **First Stable Major Release (`v1.0.0`)** — Pure Rust native Win32 + Direct2D + DirectComposition rebuild of uWidgets featuring a real-time **SDF Liquid Glass optical engine**, **4 custom Liquid Glass clock faces**, a dedicated **Weather customization tab** with instant disk-cached loading, a native **Windows Setup Installer**, and **6,400/6,400** oracle-verified parity cases.

---

## Downloads

| Artifact | Size | Description | SHA-256 |
| :--- | :---: | :--- | :--- |
| **`uWidgets-Rust-v1.0.0-Setup.exe`** | **2.28 MB** | Recommended Windows Setup Wizard (no Admin UAC required, shortcuts, startup toggle, Add/Remove Programs uninstaller) | `b04b7a03cabcd9a39136f23886764df8ca2c7eb02e52d1b7e2f8931a2d40e9a3` |
| **`uWidgets-Rust-v1.0.0-Portable.exe`** | **3.44 MB** | Standalone single-file portable executable — drop anywhere and run | `53388ae2220c10f96eaf3ec15f69202f94e59e1db3d2a1672b5cec7b750175ea` |

---

## What's New in v1.0.0

### Real-Time SDF Liquid Glass Optical Engine (`Settings → Appearance`)
- Wallpaper-aligned **Signed Distance Field (SDF)** frosted glass renderer with sub-pixel anti-aliasing and analytic surface normals.
- **4 One-Click Optical Presets**: **Crystal**, **iOS 26** (default), **Deep Frost**, and **Prism** (+ **Reset** & **Enable** controls).
- **8 Live Draggable Sliders**: **Blur Radius** (`0–30 px`, ~4 ms live in-memory re-blur), **Refraction** (`0–200%`), **Bevel Width** (`30–180%`), **Chromatic Dispersion** (`0–250%`), **Specular Rim** (`0–200%`), **Vibrancy** (`50–200%`), **Frost Grain** (`0–250%`), and **Corner Radius** (`0–44 px`).
- **9 Translucent Glass Dye Tints**: **Auto**, **Ice**, **Sky**, **Ocean**, **Violet**, **Rose**, **Sunset**, **Amber**, **Emerald**, and **Obsidian**.
- **Always-On Windows 11 Acrylic Mode**: Native-frame mode preserves full `OpacityLevel` translucency even when desktop widgets are unfocused.

### 4 Redesigned Liquid Glass Clock Faces
- **Analog I**: Minimalist circular frosted optical lens, 4 cardinal dot indices, crisp tapered hands, and red seconds hand.
- **Analog II**: Sculpted elevated bezel ring, recessed center well, 60 precision ticks, bold `12/3/6/9` numerals, and red needle.
- **Analog III**: Squircle-contour 60-tick dial ring, bold `12/3/6/9` numerals, and Apple Watch orange (`#FF9500`) seconds hand.
- **Flip Clock**: Mechanical split-flap Liquid Glass dual tiles (`HH | MM`) with metallic side hinge cylinders and optional seconds/24h/AM-PM.

### Dedicated Weather Tab & Instant Startup (`Settings → Weather`)
- **Zero-Wait Startup**: Forecasts and geocoded city coordinates persist in `weatherCache.json`, rendering live weather on **frame 0 (< 5 ms)** while refreshing asynchronously.
- **Event-Driven Updates**: Background Open-Meteo requests wake desktop widgets the millisecond forecast data arrives via `WM_APP_WEATHER_READY` without blocking on Air Quality.
- **Full Weather Customization**: **1-click IP Auto-Detect**, live **Default City Search**, **Refresh Now**, **Celsius / Fahrenheit**, **km/h / mph / m/s**, **5m / 15m / 30m / 60m** refresh intervals, **4h / 5h / 6h / 8h** hourly slots, **High & Low**, **Condition Label**, **Feels Like**, **Humidity & Wind**, and **24-Hour / 12-Hour** time formatting.

### Native Windows Setup Installer & App Icon
- **2.28 MB Native Win32 Setup Wizard** (`uWidgets-Rust-v1.0.0-Setup.exe`) with dark mode UI, per-user installation (`%LOCALAPPDATA%\uWidgets`, no UAC prompt), Start Menu & Desktop `.lnk` shortcuts, Run-on-Startup toggle, and clean **Windows Installed Apps** uninstaller.
- Supports silent automation flags: `/S` (`--silent`), `--dir="<path>"`, and `--uninstall /S`.
- Multi-resolution (`16, 24, 32, 48, 256 px`) Liquid Glass app icon embedded natively in both executables, titlebars, Taskbar, Alt+Tab, and System Tray.

### Full Settings Reliability & Performance Audit
- **~22–25 MB Private RAM** (vs 135.7 MB on C# / .NET 8 Avalonia — **5.4× lighter**).
- **3.44 MB standalone binary** (vs 61.27 MB — **17.8× smaller**).
- **0.0% idle CPU** via adaptive Win32 timers and single-threaded Direct2D + DirectComposition.
- Fixed Run-on-Startup registry path quoting for paths with spaces, HiDPI scaling for Edit & Update dialogs, DPI-scaled minimum resize constraints, post-snap glass repaint, and default Weather city inheritance for newly spawned widgets.

---

## Installation & Upgrade

1. **Recommended (Setup Wizard):** Download and run **`uWidgets-Rust-v1.0.0-Setup.exe`**. It automatically closes any previous running instance, installs to `%LOCALAPPDATA%\uWidgets`, creates shortcuts, and launches uWidgets.
2. **Portable:** Download **`uWidgets-Rust-v1.0.0-Portable.exe`**, place it in any folder, and double-click to run. Existing `appSettings.json` and `layout.json` files from previous builds migrate automatically.
3. **Silent Install / Uninstall:**
   ```powershell
   .\uWidgets-Rust-v1.0.0-Setup.exe /S
   "%LOCALAPPDATA%\uWidgets\uWidgets-Uninstall.exe" --uninstall /S
   ```
