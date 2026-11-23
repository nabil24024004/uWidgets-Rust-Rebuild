param(
    [string]$Version = "1.0.0",
    [switch]$SkipSign
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$AppExe = Join-Path $RepoRoot "rust\app\target\release\uwidgets-rs.exe"
$IconPng = Join-Path $RepoRoot "docs\images\icon.png"
$ManifestSrc = Join-Path $RepoRoot "msix\AppxManifest.xml"
$DistDir = Join-Path $RepoRoot "dist"
$StageDir = Join-Path $RepoRoot "dist\msix_stage"

if (-not (Test-Path $AppExe)) {
    throw "Release binary missing at $AppExe. Run 'cargo build --release' in rust/app first."
}
if (-not (Test-Path $IconPng)) {
    throw "Source 256x256 icon PNG missing at $IconPng."
}

$MakeAppx = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "makeappx.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*\x64\*" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1 -ExpandProperty FullName
$SignTool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*\x64\*" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1 -ExpandProperty FullName

if (-not $MakeAppx) { throw "Windows SDK makeappx.exe not found." }
Write-Host "Using makeappx: $MakeAppx"

# Prepare clean staging directory
if (Test-Path $StageDir) { Remove-Item $StageDir -Recurse -Force }
$AssetsDir = Join-Path $StageDir "Assets"
New-Item -ItemType Directory -Force -Path $AssetsDir | Out-Null
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

# Copy executable and manifest
Copy-Item $AppExe (Join-Path $StageDir "uwidgets-rs.exe") -Force
$manifestXml = (Get-Content $ManifestSrc -Raw) -replace 'Version="1\.0\.0\.0"', "Version=`"$Version.0`""
Set-Content -Path (Join-Path $StageDir "AppxManifest.xml") -Value $manifestXml -Encoding UTF8

# Generate all required Microsoft Store & MSIX PNG assets from high-res icon.png
Add-Type -AssemblyName System.Drawing
$srcBmp = [System.Drawing.Bitmap]::FromFile($IconPng)
function Save-TilePng([string]$Name, [int]$W, [int]$H, [double]$IconScale = 0.86) {
    $bmp = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $side = [int][Math]::Round([Math]::Min($W, $H) * $IconScale)
    $ox = [int][Math]::Floor(($W - $side) / 2.0)
    $oy = [int][Math]::Floor(($H - $side) / 2.0)
    $g.DrawImage($srcBmp, $ox, $oy, $side, $side)
    $g.Dispose()
    $bmp.Save((Join-Path $AssetsDir $Name), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

Save-TilePng "StoreLogo.png" 50 50 0.92
Save-TilePng "Square44x44Logo.png" 44 44 0.92
Save-TilePng "Square44x44Logo.targetsize-24_altform-unplated.png" 24 24 0.95
Save-TilePng "Square44x44Logo.targetsize-48_altform-unplated.png" 48 48 0.95
Save-TilePng "Square44x44Logo.targetsize-256_altform-unplated.png" 256 256 0.95
Save-TilePng "Square150x150Logo.png" 150 150 0.72
Save-TilePng "Wide310x150Logo.png" 310 150 0.68
Save-TilePng "Square310x310Logo.png" 310 310 0.64
Save-TilePng "SplashScreen.png" 620 300 0.48
$srcBmp.Dispose()

# Pack unsigned Store-ready MSIX (Microsoft Partner Center signs Store submissions automatically)
$StoreMsix = Join-Path $DistDir "uWidgets-Rust-v$Version-x64-Store.msix"
$SideloadMsix = Join-Path $DistDir "uWidgets-Rust-v$Version-x64-Sideload.msix"
& $MakeAppx pack /o /d $StageDir /p $StoreMsix
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed with exit code $LASTEXITCODE" }

Copy-Item $StoreMsix $SideloadMsix -Force

# Sign Sideload MSIX with self-signed certificate matching Partner Center Publisher CN for local Windows 11 testing
if (-not $SkipSign -and $SignTool) {
    $pubCn = "CN=DFBB08CE-EBCC-4045-96A4-05CEDB00F2BD"
    $cert = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Subject -eq $pubCn -and $_.EnhancedKeyUsageList.ObjectId -contains "1.3.6.1.5.5.7.3.3" } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1
    if (-not $cert) {
        Write-Host "Creating self-signed code-signing certificate ($pubCn) in CurrentUser\My..."
        $cert = New-SelfSignedCertificate `
            -Type Custom `
            -Subject $pubCn `
            -KeyUsage DigitalSignature `
            -FriendlyName "NeoSparkX uWidgets Sideload Dev Cert" `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
    }
    $cerOut = Join-Path $DistDir "uWidgets-TestCert.cer"
    Export-Certificate -Cert $cert -FilePath $cerOut -Force | Out-Null

    & $SignTool sign /fd SHA256 /sha1 $cert.Thumbprint $SideloadMsix
    if ($LASTEXITCODE -ne 0) { throw "signtool sign failed with exit code $LASTEXITCODE" }
    Write-Host "Signed sideload MSIX with thumbprint $($cert.Thumbprint)"
}

# Remove temporary staging folder
Remove-Item $StageDir -Recurse -Force -ErrorAction SilentlyContinue

foreach ($f in @($StoreMsix, $SideloadMsix)) {
    if (Test-Path $f) {
        $item = Get-Item $f
        $hash = (Get-FileHash $f -Algorithm SHA256).Hash.ToLower()
        $sig = (Get-AuthenticodeSignature $f).Status
        Write-Host "$($item.Name) | $([math]::Round($item.Length/1MB, 2)) MB | Sig=$sig | SHA256=$hash"
    }
}
