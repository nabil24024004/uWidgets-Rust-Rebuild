# Locales & Assets Inventory

All assets were extracted read-only from `C:\uWidgets\uWidgets.exe` and `C:\uWidgets\Widgets\*.dll` and copied to `d:\uWidgets in Rust\assets\`:

## Localization (`assets/locales.json`)
- Extracted from embedded `.resources` streams across `uWidgets.dll`, all 6 widget DLLs, and satellite assemblies:
  - `en`: 167 keys (complete base fallback)
  - `ru`: 167 keys
  - `zh-Hans`: 167 keys
  - `zh-Hant`: 167 keys
  - `de`: 42 keys (fallback to `en` for missing keys)
  - `es`, `fr`, `ja`, `ko`, `pl`, `uk`: satellite placeholders present in original distribution (fallback to `en`)

## Fonts (`assets/fonts/`)
- Extracted from `Avalonia.Fonts.Inter.dll` (`!AvaloniaResources` v2):
  - `Inter-Thin.ttf`, `Inter-ExtraLight.ttf`, `Inter-Light.ttf`, `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf`, `Inter-Bold.ttf`
- Bundled cleanly or loaded via DirectWrite `IDWriteFontCollectionLoader` / `AddFontMemResourceEx` so `Inter` renders crisply without needing OS installation, alongside system `Segoe UI` and `Microsoft YaHei`.

## Application Icon (`assets/uWidgets.ico`)
- Reconstructed directly from `uWidgets.exe` PE `RT_GROUP_ICON` #32512 (REA Evidence `ev_d8a7fbf5…`): 5 RGBA sizes (`16×16`, `24×24`, `32×32`, `48×48`, `256×256`, 99,402 bytes).

## Vector Icons
- All 5 `Monitor` metric icons (`CpuUsage`, `RamUsage`, `DiskUsage`, `NetworkUsage`, `BatteryLevel`), all 15 `Weather` icons (`Clear`, `PartlyClear`, `Cloudy`, `Fog`, `Drizzle`, `Rain`, `HeavyRain`, `Snow`, `HeavySnow`, `Thunderstorm`, `Sunrise`, `Sunset`, `Night`, `Pressure`, `Wind`), all 6 widget category sidebar icons, and all Settings UI icons extracted directly as exact SVG path data strings.
