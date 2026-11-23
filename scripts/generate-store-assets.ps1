$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$RepoRoot = Split-Path -Parent $PSScriptRoot
$BrainDir = "C:\Users\capta\.gemini\antigravity\brain\28682132-b0b9-4826-b73c-5ccd75374e5b"
$DocsImg = Join-Path $RepoRoot "docs\images"
$OutDir = Join-Path $RepoRoot "store-listing\assets"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# Load real desktop wallpaper if available, else synthesize a rich aurora gradient
$WallPath = (Get-ItemProperty "HKCU:\Control Panel\Desktop" -Name WallPaper -ErrorAction SilentlyContinue).WallPaper

function New-Canvas([int]$W, [int]$H) {
    $bmp = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
    return @($bmp, $g)
}

function Draw-Backdrop([System.Drawing.Graphics]$g, [int]$W, [int]$H, [double]$DimAlpha = 0.42) {
    if ($WallPath -and (Test-Path $WallPath)) {
        $wBmp = [System.Drawing.Bitmap]::FromFile($WallPath)
        $scale = [Math]::Max([double]$W / $wBmp.Width, [double]$H / $wBmp.Height)
        $sw = [int][Math]::Ceiling($wBmp.Width * $scale)
        $sh = [int][Math]::Ceiling($wBmp.Height * $scale)
        $ox = [int](($W - $sw) / 2)
        $oy = [int](($H - $sh) / 2)
        $g.DrawImage($wBmp, $ox, $oy, $sw, $sh)
        $wBmp.Dispose()
    } else {
        $rect = [System.Drawing.Rectangle]::new(0, 0, $W, $H)
        $lg = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, [System.Drawing.Color]::FromArgb(255, 14, 24, 44), [System.Drawing.Color]::FromArgb(255, 18, 48, 58), 35.0)
        $g.FillRectangle($lg, $rect)
        $lg.Dispose()
    }
    if ($DimAlpha -gt 0.0) {
        $a = [int][Math]::Round($DimAlpha * 255)
        $br = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb($a, 10, 14, 22))
        $g.FillRectangle($br, 0, 0, $W, $H)
        $br.Dispose()
    }
}

function New-RoundPath([int]$X, [int]$Y, [int]$W, [int]$H, [int]$R) {
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $R * 2
    if ($d -le 0) {
        $path.AddRectangle([System.Drawing.Rectangle]::new($X, $Y, $W, $H))
        return $path
    }
    $path.AddArc($X, $Y, $d, $d, 180, 90)
    $path.AddArc($X + $W - $d, $Y, $d, $d, 270, 90)
    $path.AddArc($X + $W - $d, $Y + $H - $d, $d, $d, 0, 90)
    $path.AddArc($X, $Y + $H - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

function Draw-ShadowedImage([System.Drawing.Graphics]$g, [string]$ImgPath, [int]$X, [int]$Y, [int]$W, [int]$H, [int]$Radius = 24) {
    if (-not (Test-Path $ImgPath)) { return }
    $im = [System.Drawing.Bitmap]::FromFile($ImgPath)
    for ($s = 3; $s -ge 1; $s--) {
        $pad = $s * 7
        $sp = New-RoundPath ($X - $pad/2) ($Y + $s*4) ($W + $pad) ($H + $pad) ($Radius + $s*3)
        $sb = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb([int](36 / $s), 0, 0, 0))
        $g.FillPath($sb, $sp)
        $sb.Dispose(); $sp.Dispose()
    }
    $clip = New-RoundPath $X $Y $W $H $Radius
    $state = $g.Save()
    $g.SetClip($clip)
    $g.DrawImage($im, $X, $Y, $W, $H)
    $g.Restore($state)
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(75, 255, 255, 255), 1.5)
    $g.DrawPath($pen, $clip)
    $pen.Dispose(); $clip.Dispose(); $im.Dispose()
}

function Draw-BadgePill([System.Drawing.Graphics]$g, [string]$Text, [int]$X, [int]$Y, [int]$W, [int]$H) {
    $p = New-RoundPath $X $Y $W $H ([int]($H / 2))
    $b = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(65, 37, 99, 235))
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(165, 96, 165, 250), 1.5)
    $g.FillPath($b, $p)
    $g.DrawPath($pen, $p)
    $f = New-Object System.Drawing.Font("Segoe UI", 12.5, [System.Drawing.FontStyle]::Bold)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $tb = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 191, 219, 254))
    $g.DrawString($Text, $f, $tb, [System.Drawing.RectangleF]::new($X, $Y, $W, $H), $sf)
    $tb.Dispose(); $sf.Dispose(); $f.Dispose(); $pen.Dispose(); $b.Dispose(); $p.Dispose()
}

$IconPath = Join-Path $DocsImg "icon.png"
$FontTitle = New-Object System.Drawing.Font("Segoe UI", 40, [System.Drawing.FontStyle]::Bold)
$FontSub = New-Object System.Drawing.Font("Segoe UI", 18.5, [System.Drawing.FontStyle]::Regular)
$FontCardTitle = New-Object System.Drawing.Font("Segoe UI", 16, [System.Drawing.FontStyle]::Bold)
$WhiteBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 248, 250, 252))
$MutedBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(225, 203, 213, 225))
$AccentBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 96, 165, 250))

# ==============================================================================
# 1. Screenshot 1 (1920x1080): Hero Desktop Widgets Showcase
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.28
Draw-ShadowedImage $g $IconPath 100 120 108 108 26
Draw-BadgePill $g "PURE RUST  •  DIRECT2D  •  ~23 MB RAM" 100 264 380 42
$g.DrawString("uWidgets for Windows", $FontTitle, $WhiteBrush, 92, 326)
$g.DrawString("Real-time SDF Liquid Glass desktop widgets with curved`nbevel refraction, instant weather, and 0% idle CPU.", $FontSub, $MutedBrush, 98, 412)

$bullets = @(
    "✓  16 interactive desktop widgets across 6 categories",
    "✓  Apple iOS 26–inspired Liquid Glass & Windows 11 Acrylic",
    "✓  5.5× lower memory footprint than .NET/Avalonia",
    "✓  Instant disk-cached Open-Meteo weather & global search"
)
$by = 530
$fBul = New-Object System.Drawing.Font("Segoe UI", 16, [System.Drawing.FontStyle]::Regular)
foreach ($line in $bullets) {
    $g.DrawString($line, $fBul, $WhiteBrush, 102, $by)
    $by += 46
}
$fBul.Dispose()

# Right side: 4 live Liquid Glass desktop widgets arranged in a crisp 2-column grid
$wClock3 = Join-Path $DocsImg "clock-analog-3.png"
$wDate = if (Test-Path "$BrainDir\lg_Calendar_Date.png") { "$BrainDir\lg_Calendar_Date.png" } else { Join-Path $DocsImg "clock-flip.png" }
$wWeather = Join-Path $DocsImg "weather-widget.png"
$wMonth = if (Test-Path "$BrainDir\lg_Calendar_Month.png") { "$BrainDir\lg_Calendar_Month.png" } else { Join-Path $DocsImg "weather-widget.png" }

Draw-ShadowedImage $g $wClock3 1080 120 310 310 44
Draw-ShadowedImage $g $wDate 1430 120 310 310 44
Draw-ShadowedImage $g $wWeather 1080 465 660 260 40
Draw-ShadowedImage $g $wMonth 1080 760 660 250 40

$bmp.Save((Join-Path $OutDir "Screenshot-01-Desktop-Widgets-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 2. Screenshot 2 (1920x1080): Full Liquid Glass Customization (Settings -> Appearance)
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.45
Draw-BadgePill $g "LIVE OPTICAL ENGINE CUSTOMIZER" 96 68 330 40
$g.DrawString("Customize Every Liquid Glass Parameter Live", $FontTitle, $WhiteBrush, 90, 122)
$g.DrawString("Tune Blur Radius, Bevel Refraction, Chromatic Dispersion, Fresnel Specular Rim, Vibrancy, and 9 Glass Dye Tints.", $FontSub, $MutedBrush, 96, 198)

Draw-ShadowedImage $g (Join-Path $DocsImg "liquid-glass-settings.png") 96 270 840 730 18
Draw-ShadowedImage $g (Join-Path $DocsImg "liquid-glass-tint.png") 984 270 840 730 18

$bmp.Save((Join-Path $OutDir "Screenshot-02-Liquid-Glass-Settings-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 3. Screenshot 3 (1920x1080): 4 Bespoke Liquid Glass Clock Designs
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.40
Draw-BadgePill $g "BESPOKE LIQUID GLASS TIMEPIECES" 760 76 400 42
$sfC = New-Object System.Drawing.StringFormat; $sfC.Alignment = [System.Drawing.StringAlignment]::Center
$g.DrawString("Four Iconic Liquid Glass Clock Faces", $FontTitle, $WhiteBrush, [System.Drawing.RectangleF]::new(0, 134, 1920, 64), $sfC)
$g.DrawString("From minimalist frosted lenses and sculpted bezels to mechanical split-flap Liquid Glass tiles.", $FontSub, $MutedBrush, [System.Drawing.RectangleF]::new(0, 206, 1920, 44), $sfC)

$clocks = @(
    @{ Path = (Join-Path $DocsImg "clock-analog-1.png"); Title = "Analog I — Minimal Lens"; Sub = "Circular frosted lens & red sweep" },
    @{ Path = (Join-Path $DocsImg "clock-analog-2.png"); Title = "Analog II — Sculpted Bezel"; Sub = "Recessed dial well & 60 precision ticks" },
    @{ Path = (Join-Path $DocsImg "clock-analog-3.png"); Title = "Analog III — Squircle Contour"; Sub = "Squircle tick ring & Watch orange needle" },
    @{ Path = (Join-Path $DocsImg "clock-flip.png"); Title = "Flip Clock — Split-Flap"; Sub = "Dual glass tiles & metallic side hinges" }
)
$startX = 110
foreach ($ck in $clocks) {
    Draw-ShadowedImage $g $ck.Path $startX 320 380 380 54
    $g.DrawString($ck.Title, $FontCardTitle, $WhiteBrush, [System.Drawing.RectangleF]::new($startX, 735, 380, 34), $sfC)
    $fS = New-Object System.Drawing.Font("Segoe UI", 13.5, [System.Drawing.FontStyle]::Regular)
    $g.DrawString($ck.Sub, $fS, $MutedBrush, [System.Drawing.RectangleF]::new($startX, 772, 380, 32), $sfC)
    $fS.Dispose()
    $startX += 440
}

$bmp.Save((Join-Path $OutDir "Screenshot-03-Clock-Collection-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 4. Screenshot 4 (1920x1080): Dedicated Weather Tab & Instant Forecast
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.42
Draw-BadgePill $g "ZERO-WAIT DISK-CACHED WEATHER" 96 68 350 40
$g.DrawString("Instant Weather Forecast & Global City Search", $FontTitle, $WhiteBrush, 90, 122)
$g.DrawString("1-click Auto-Detect location, Celsius/Fahrenheit, wind speed units, 4–8h hourly slots, Feels Like, Humidity & Wind.", $FontSub, $MutedBrush, 96, 198)

Draw-ShadowedImage $g (Join-Path $DocsImg "weather-settings.png") 96 270 1050 730 18
Draw-ShadowedImage $g (Join-Path $DocsImg "weather-widget.png") 1200 320 624 296 42
if (Test-Path "$BrainDir\settings_gallery_weather.png") {
    Draw-ShadowedImage $g "$BrainDir\settings_gallery_weather.png" 1200 650 624 350 18
}

$bmp.Save((Join-Path $OutDir "Screenshot-04-Weather-Customization-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 5. Screenshot 5 (1920x1080): Ultra-Light Performance & Auto-Updates
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.45
Draw-BadgePill $g "ENGINEERED IN PURE RUST" 96 68 290 40
$g.DrawString("5.5× Lighter RAM  •  18× Smaller Footprint", $FontTitle, $WhiteBrush, 90, 122)
$g.DrawString("Native Win32 + Direct2D + DirectComposition architecture with automatic Microsoft Store updates.", $FontSub, $MutedBrush, 96, 198)

$aboutImg = if (Test-Path "$BrainDir\settings_about_updates.png") { "$BrainDir\settings_about_updates.png" } else { Join-Path $DocsImg "weather-settings.png" }
Draw-ShadowedImage $g $aboutImg 96 270 1040 730 18

# Stat cards on right
$stats = @(
    @{ Val = "~23.5 MB"; Lbl = "Private Memory (vs 135.7 MB in C#)" },
    @{ Val = "1.98 MB"; Lbl = "Total MSIX Package Size (Zero .NET Runtime)" },
    @{ Val = "0.0% CPU"; Lbl = "Event-Driven Idle Desktop Footprint" },
    @{ Val = "12 Languages"; Lbl = "English, German, Chinese, Spanish, French & more" }
)
$sy = 270
$fVal = New-Object System.Drawing.Font("Segoe UI", 32, [System.Drawing.FontStyle]::Bold)
$fLbl = New-Object System.Drawing.Font("Segoe UI", 14.5, [System.Drawing.FontStyle]::Regular)
foreach ($st in $stats) {
    $p = New-RoundPath 1185 $sy 640 162 20
    $b = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(195, 24, 30, 42))
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(80, 255, 255, 255), 1.5)
    $g.FillPath($b, $p); $g.DrawPath($pen, $p)
    $g.DrawString($st.Val, $fVal, $AccentBrush, 1225, ($sy + 26))
    $g.DrawString($st.Lbl, $fLbl, $WhiteBrush, 1227, ($sy + 96))
    $pen.Dispose(); $b.Dispose(); $p.Dispose()
    $sy += 188
}
$fVal.Dispose(); $fLbl.Dispose()

$bmp.Save((Join-Path $OutDir "Screenshot-05-Performance-And-Updates-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 6. Windows and Xbox Super Hero Art (1920x1080) — NO PRODUCT TITLE TEXT!
#    (Partner Center requirement: "Does not include the product title")
# ==============================================================================
$c = New-Canvas 1920 1080; $bmp = $c[0]; $g = $c[1]
Draw-Backdrop $g 1920 1080 0.20
Draw-ShadowedImage $g (Join-Path $DocsImg "clock-analog-2.png") 160 150 340 340 48
Draw-ShadowedImage $g (Join-Path $DocsImg "clock-analog-3.png") 540 150 340 340 48
Draw-ShadowedImage $g (Join-Path $DocsImg "clock-flip.png") 160 530 340 340 48
Draw-ShadowedImage $g $wDate 540 530 340 340 48
Draw-ShadowedImage $g $wWeather 940 150 820 340 46
Draw-ShadowedImage $g $wMonth 940 530 820 340 46
$bmp.Save((Join-Path $OutDir "Store-Hero-Art-NoText-1920x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# ==============================================================================
# 7. Store Logos: 1:1 Box Art (1080x1080), 2:3 Poster Art (720x1080), App Tiles (300/150/71)
# ==============================================================================
# 1:1 Box Art (1080x1080)
$c = New-Canvas 1080 1080; $bmp = $c[0]; $g = $c[1]
$rect = [System.Drawing.Rectangle]::new(0, 0, 1080, 1080)
$lg = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, [System.Drawing.Color]::FromArgb(255, 15, 23, 42), [System.Drawing.Color]::FromArgb(255, 12, 34, 68), 45.0)
$g.FillRectangle($lg, $rect); $lg.Dispose()
$ic = [System.Drawing.Bitmap]::FromFile($IconPath)
$g.DrawImage($ic, 160, 110, 760, 760)
$fBox = New-Object System.Drawing.Font("Segoe UI", 48, [System.Drawing.FontStyle]::Bold)
$g.DrawString("uWidgets", $fBox, $WhiteBrush, [System.Drawing.RectangleF]::new(0, 890, 1080, 100), $sfC)
$fBox.Dispose()
$bmp.Save((Join-Path $OutDir "Store-Logo-BoxArt-1x1-1080x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# 2:3 Poster Art (720x1080)
$c = New-Canvas 720 1080; $bmp = $c[0]; $g = $c[1]
$rect = [System.Drawing.Rectangle]::new(0, 0, 720, 1080)
$lg = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, [System.Drawing.Color]::FromArgb(255, 15, 23, 42), [System.Drawing.Color]::FromArgb(255, 14, 42, 84), 60.0)
$g.FillRectangle($lg, $rect); $lg.Dispose()
$g.DrawImage($ic, 120, 160, 480, 480)
$fPos1 = New-Object System.Drawing.Font("Segoe UI", 46, [System.Drawing.FontStyle]::Bold)
$fPos2 = New-Object System.Drawing.Font("Segoe UI", 19, [System.Drawing.FontStyle]::Regular)
$g.DrawString("uWidgets", $fPos1, $WhiteBrush, [System.Drawing.RectangleF]::new(0, 680, 720, 80), $sfC)
$g.DrawString("Liquid Glass Desktop Widgets", $fPos2, $MutedBrush, [System.Drawing.RectangleF]::new(0, 765, 720, 50), $sfC)
$fPos1.Dispose(); $fPos2.Dispose()
$bmp.Save((Join-Path $OutDir "Store-Logo-PosterArt-2x3-720x1080.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# 1:1 App Tile Icons (300x300, 150x150, 71x71)
foreach ($sz in @(300, 150, 71)) {
    $c = New-Canvas $sz $sz; $bmp = $c[0]; $g = $c[1]
    $g.Clear([System.Drawing.Color]::Transparent)
    $pad = [int][Math]::Round($sz * 0.04)
    $g.DrawImage($ic, $pad, $pad, ($sz - $pad*2), ($sz - $pad*2))
    $bmp.Save((Join-Path $OutDir "Store-Tile-1x1-${sz}x${sz}.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
$ic.Dispose()
$sfC.Dispose(); $FontTitle.Dispose(); $FontSub.Dispose(); $FontCardTitle.Dispose()
$WhiteBrush.Dispose(); $MutedBrush.Dispose(); $AccentBrush.Dispose()

Get-ChildItem $OutDir | Select-Object Name, Length | Format-Table -AutoSize
