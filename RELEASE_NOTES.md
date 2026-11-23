# uWidgets (Rust Edition) — v1.0.0 Release Notes

> **First Stable Major Release (`v1.0.0`)** — Pure Rust native Win32 + Direct2D + DirectComposition rebuild of uWidgets featuring a real-time **SDF Liquid Glass optical engine**, **4 custom Liquid Glass clock faces**, a dedicated **Weather customization tab** with instant disk-cached loading, a **dual-channel automatic updater** (Microsoft Store + GitHub Releases / NeosparkX), a native **Windows Setup Installer**, and a **Microsoft Store-ready MSIX build**.

---

## Downloads

| Artifact | Size | Description | SHA-256 |
| :--- | :---: | :--- | :--- |
| **`uWidgets-Rust-v1.0.0-Setup.exe`** | **2.31 MB** | Recommended Windows Setup Wizard (no Admin UAC required, shortcuts, startup toggle, Add/Remove Programs uninstaller) | `3b8a33715378ce46036d4ff85f616dd472a1c8b35228963645f0262eaab1339e` |
| **`uWidgets-Rust-v1.0.0-Portable.exe`** | **3.50 MB** | Standalone single-file portable executable — drop anywhere and run | `81342cdb06a4f718a80ca01ffd79c9693d68f1ef7ea220ed1315473ea96ea39f` |
| **`uWidgets-Rust-v1.0.0-x64-Store.msix`** | **1.98 MB** | Microsoft Store Partner Center upload package (`NeoSparkX.uWidgets`, Store ID `9NDGKTPBF48G`) | `586fb4181069e1a4a5a1f97cb92d7b36cee923b3bc17ebca16c9a11805baaeac` |
| **`uWidgets-Rust-v1.0.0-x64-Sideload.msix`** | **1.98 MB** | Signed MSIX package for local Windows 10/11 sideloading (pair with `uWidgets-TestCert.cer`) | `65eced5f3de864b8f49d37b7ac32daf6b937e570eb3a673bbefd6a2e0dbd447b` |

---

## What's New in v1.0.0

### Dual-Channel Automatic Updater (`Settings → About`)
- **Microsoft Store (MSIX) Channel**: Detects packaged identity via `GetCurrentPackageFullName` (`NeoSparkX.uWidgets`, Store ID `9NDGKTPBF48G`), queries `Windows.Services.Store.StoreContext`, and automatically downloads & installs updates silently via `TrySilentDownloadAndInstallStorePackageUpdatesAsync` (with Store deep-link & App Installer fallback for sideloaded MSIX).
- **Direct Release Channel (GitHub Releases + NeosparkX)**: Checks **both** GitHub Releases and `https://neosparkx.com/releases/uwidgets/latest.json` using SemVer comparison, verifies Windows PE header + SHA-256 integrity, performs silent Setup.exe upgrade or atomic portable binary swap, and relaunches cleanly.
- **MSIX-Safe Storage**: Automatically redirects `appSettings.json`, `layout.json`, and `weatherCache.json` to `%LOCALAPPDATA%\uWidgets` when running inside read-only WindowsApps MSIX containers.

### Real-Time SDF Liquid Glass Optical Engine (`Settings → Appearance`)
- Wallpaper-aligned **Signed Distance Field (SDF)** frosted glass renderer with sub-pixel anti-aliasing and analytic surface normals.
- **4 One-Click Optical Presets**: **Crystal**, **iOS 26** (default), **Deep Frost**, and **Prism** (+ **Reset** & **Enable** controls).
- **8 Live Draggable Sliders**: **Blur Radius** (`0–30 px`, ~4 ms live in-memory re-blur), **Refraction** (`0–200%`), **Bevel Width** (`30–180%`), **Chromatic Dispersion** (`0–250%`), **Specular Rim** (`0–200%`), **Vibrancy** (`50–200%`), **Frost Grain** (`0–250%`), and **Corner Radius** (`0–44 px`).
- **9 Translucent Glass Dye Tints**: **Auto**, **Ice**, **Sky**, **Ocean**, **Violet**, **Rose**, **Sunset**, **Amber**, **Emerald**, and **Obsidian**.
- **Always-On Windows 11 Acrylic Mode**: Native-frame mode preserves full `OpacityLevel` translucency even when desktop widgets lose focus.

### 4 Redesigned Liquid Glass Clock Faces
- **Analog I**: Minimalist circular frosted optical lens, 4 cardinal dot indices, crisp tapered hands, and red seconds hand.
- **Analog II**: Sculpted elevated bezel ring, recessed center well, 60 precision ticks, bold `12/3/6/9` numerals, and red needle.
- **Analog III**: Squircle-contour 60-tick dial ring, bold `12/3/6/9` numerals, and Apple Watch orange (`#FF9500`) seconds hand.
- **Flip Clock**: Mechanical split-flap Liquid Glass dual tiles (`HH | MM`) with metallic side hinge cylinders and optional seconds/24h/AM-PM.

### Dedicated Weather Tab & Instant Startup (`Settings → Weather`)
- **Zero-Wait Startup**: Forecasts and geocoded city coordinates persist in `weatherCache.json`, rendering live weather on **frame 0 (< 5 ms)** while refreshing asynchronously.
- **Event-Driven Updates**: Background Open-Meteo requests wake desktop widgets the millisecond forecast data arrives via `WM_APP_WEATHER_READY` without blocking on Air Quality.
- **Full Weather Customization**: **1-click IP Auto-Detect**, live **Default City Search**, **Refresh Now**, **Celsius / Fahrenheit**, **km/h / mph / m/s**, **5m / 15m / 30m / 60m** refresh intervals, **4h / 5h / 6h / 8h** hourly slots, **High & Low**, **Condition Label**, **Feels Like**, **Humidity & Wind**, and **24-Hour / 12-Hour** time formatting.

### Native Windows Setup Installer, MSIX Build & App Icon
- **2.31 MB Native Win32 Setup Wizard** (`uWidgets-Rust-v1.0.0-Setup.exe`) with dark mode UI, per-user install (`%LOCALAPPDATA%\uWidgets`, no UAC), Start Menu & Desktop `.lnk` shortcuts, Run-on-Startup toggle, and Windows Installed Apps uninstaller.
- **1.98 MB MSIX Packages** (`Store.msix` for Partner Center + signed `Sideload.msix` + `uWidgets.appinstaller`) with full Store logo asset suite and native `windows.startupTask`.
- Multi-resolution (`16, 24, 32, 48, 256 px`) Liquid Glass app icon embedded natively across executables, titlebars, Taskbar, Alt+Tab, and System Tray.

---

## Installation & Upgrade

1. **Microsoft Store:** Install directly from **[Microsoft Store (`9NDGKTPBF48G`)](https://apps.microsoft.com/detail/9NDGKTPBF48G)** for automatic silent Store updates.
2. **Desktop Setup Wizard (Recommended for Direct Downloads):** Run **`uWidgets-Rust-v1.0.0-Setup.exe`** (`2.31 MB`). Installs to `%LOCALAPPDATA%\uWidgets` with automatic GitHub / NeosparkX updates.
3. **Portable:** Run **`uWidgets-Rust-v1.0.0-Portable.exe`** (`3.50 MB`) from any folder.
4. **Silent CLI Automation:**
   ```powershell
   .\uWidgets-Rust-v1.0.0-Setup.exe /S
   "%LOCALAPPDATA%\uWidgets\uWidgets-Uninstall.exe" --uninstall /S
   ```
