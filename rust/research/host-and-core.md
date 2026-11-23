# Host & Core Specification (REA Evidence Ledger)

## Target Artifact Identity
- **Container**: `C:\uWidgets\uWidgets.exe` (45,038,319 B, PE32+ x86_64, SHA-256 `d4908f6e37ca9ddf325ba0b6bdcdbd034ee89a953d4b71b38013d303d6d323fa`) — .NET 8 Single-File Bundle v6.0 (51 uncompressed files). REA Evidence: `ev_b6f14eafa431fbb55624f12f97c8331b3731cc10f994fbe75a5c03c7b2eb76a7`, `ev_d8a7fbf55b7e9a94d6c0204505084cafb4fda0ba0ce530bc1b9b0d9274a7c9ab`.
- **Core Assembly**: `uWidgets.Core.dll` v0.5.0.0 (77,824 B, SHA-256 `dd23b4762398617af85eed3f956b50af91c9c1019deb7aabec32757d30b3f170`). REA Evidence: `ev_5e6e07073f1d5924ecdbe8a443b96abf64f996e2604b80f0f111be6700a2021e`, `ev_9d7e80765ff4b52dbe5edcb315a849bf25ae68de49b0bf37e3623eee4685bb7d`.
- **Host Assembly**: `uWidgets.dll` v0.6.0.0 (700,416 B, SHA-256 `89b6a89f0caa160c83cc3daa2de02fd7cf7fd75480a235099be17b7bf1dfcc8a`). REA Evidence: `ev_261e8b2fc69873bc5717c4f1b6485b226f21bba33fe07ddb947e39903262d34f`, `ev_2760493a5819f51f1e5cd2e5d117552b55e7fbc6d39f21d3d5bb8a3b7666ce4e`, `ev_419d3d1bc5869ae7b0360c53b22462953f563f886bcb8c74e6ca63471638b281`.

## Exact Core Models & JSON Schemas
- `appSettings.json` (`AppSettings`):
  - `Theme`: `{ DarkMode: bool?, AccentColor: string?, OpacityLevel: f64, Monochrome: bool, UseNativeFrame: bool, FontFamily: string }`
  - `Templates`: `Theme[8]` (4 light + 4 dark combinations of Opacity 0.4/1.0 and NativeFrame false/true with Inter/Segoe UI)
  - `Layout`: `{ SnapSize: bool, LockSize: bool, SnapPosition: bool, LockPosition: bool }`
  - `Dimensions`: `{ Size: i32 (36..144, default 72), Margin: i32 (0..32, default 16), Radius: i32 (0..48, default 24) }`
  - `Region`: `{ Language: string }`
  - `RunOnStartup`: `bool`
  - `IgnoreUpdate`: `string?`
- `layout.json` (`Vec<WidgetLayout>`):
  - `{ Type: string, SubType: string, X: i32, Y: i32, Width: i32, Height: i32, Settings: Value? }`

## Exact Host Algorithms
1. **GridService**:
   - `GetSize(units, scaling, addMargin)`:
     - `addMargin == true`: `(int)(scaling * units * (Size + Margin) + scaling * Margin)`
     - `addMargin == false`: `(int)(scaling * units * (Size + Margin) - scaling * Margin)`
   - `SnapDimension(pixels, scaling, addMargin, minValue)`:
     - `units = max(minValue, round_half_to_even(pixels / (scaling * (Size + Margin))))`
     - returns `GetSize(units, scaling, addMargin)`
   - `SnapSize`: `SnapDimension(dim, 1.0, false, 1)`
   - `SnapPosition`: `SnapDimension(pos, scaling, true, 0)`
2. **Widget Resize Presets & Content Scaling**:
   - `ResizeSmall`: `(2, 2)`, `ResizeMedium`: `(4, 2)`, `ResizeLarge`: `(4, 4)`, `ResizeExtraLarge`: `(8, 4)`
   - Content scale factor: `scale = Dimensions.Size / 72.0` (base logical size = `Width / scale`, `Height / scale`).
   - New Gallery widget size: `2 * Dimensions.Size + Dimensions.Margin` square.
   - Min size when unlocked: `Dimensions.Size`.
3. **ThemeService & Styling**:
   - DarkMode: `null` = Windows system theme, `true` = Dark (`#2E2E2E` surface), `false` = Light (`#FFFFFF` surface).
   - Opacity < 1.0 enables `Transparent` mode: Acrylic blur backdrop + `TextOpacity = 0.8` (otherwise `1.0`).
   - `Monochrome = true`: replaces accent color with foreground monochrome color.
   - `UseNativeFrame = true`: native Win11 DWM border/shadow/rounded corners; `false`: borderless rounded card with `Dimensions.Radius`.
   - Fonts: `["Inter", "Segoe UI", "Microsoft YaHei"]`.
4. **Native Win32 Boundaries (`InteropService` & `StartupService`)**:
   - `GetWallpaperPath`: `SystemParametersInfoW(SPI_GETDESKWALLPAPER = 115, 260, buf, 0)`
   - `RemoveWindowFromAltTab`: `SetWindowLongPtrW(hwnd, GWL_EXSTYLE (-20), exStyle | WS_EX_TOOLWINDOW (0x80))`
   - `StartupService`: `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` value `"uWidgets"` = process executable path.
5. **Lifecycle & Timers**:
   - CLI `--settings` or 0 widgets -> show Settings window.
   - `TimerService`: shared timers at `100ms`, `1s`, `5s`, `1m`, `5m`, `1h`, `1d`; triggered immediately on Windows `SessionUnlock`.
   - Settings `SplitView.IsPaneOpen = Width >= 800.0`; Easter egg keydown sets title to `"UwUidgets"`.
   - `ClickThroughTextBox`: single-click drags widget; double-click enters text edit; `Enter`/`Tab`/`Escape` commits & clears focus.
