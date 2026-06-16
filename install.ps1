# nissia installer for Windows
# Usage: irm https://raw.githubusercontent.com/itielsefer23/nissia-browser/master/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "itielsefer23/nissia-browser"
$BinaryName = "nissia.exe"
$InstallDir = "$env:LOCALAPPDATA\nissia\bin"

function Get-LatestVersion {
    # Use GitHub redirect (no API rate limit)
    try {
        $response = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -MaximumRedirection 0 -ErrorAction SilentlyContinue
    } catch {
        $response = $_.Exception.Response
    }
    if ($response.Headers.Location) {
        return ($response.Headers.Location -split '/')[-1]
    }
    # Fallback: API
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    return $release.tag_name
}

Write-Host "==> Detecting platform..." -ForegroundColor Green
$arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }
$assetName = "nissia-windows-$arch"
Write-Host "==> Platform: $assetName" -ForegroundColor Green

$version = if ($env:NISSIA_VERSION) { $env:NISSIA_VERSION } else { Get-LatestVersion }
Write-Host "==> Installing nissia $version" -ForegroundColor Green

$url = "https://github.com/$Repo/releases/download/$version/$assetName.zip"
$tempDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
$zipPath = Join-Path $tempDir "$assetName.zip"

Write-Host "==> Downloading $url" -ForegroundColor Green
Invoke-WebRequest -Uri $url -OutFile $zipPath

Write-Host "==> Extracting" -ForegroundColor Green
Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

# Install
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
Copy-Item -Path (Join-Path $tempDir $BinaryName) -Destination (Join-Path $InstallDir $BinaryName) -Force

# Clean up
Remove-Item -Path $tempDir -Recurse -Force

Write-Host "==> nissia $version installed to $InstallDir\$BinaryName" -ForegroundColor Green
Write-Host ""

# Add to PATH automatically: persist for future terminals AND update the current
# session, so `nissia` works right away without the user editing anything.
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($null -eq $userPath) { $userPath = "" }
if ($userPath -notlike "*$InstallDir*") {
    $newUserPath = if ($userPath) { "$InstallDir;$userPath" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable('PATH', $newUserPath, 'User')
    Write-Host "==> Added $InstallDir to your PATH (User)." -ForegroundColor Green
}
if ($env:PATH -notlike "*$InstallDir*") {
    $env:PATH = "$InstallDir;$env:PATH"  # current session
}

Write-Host ""
Write-Host "  Quick start:"
Write-Host ""
Write-Host "    nissia browser launch --background"
Write-Host "    nissia snap https://example.com"
Write-Host "    nissia click @e1"
Write-Host "    nissia browser stop"
Write-Host ""
Write-Host "  If a different/already-open terminal can't find 'nissia' yet, restart it"
Write-Host "  (or use the full path: $InstallDir\$BinaryName)."
Write-Host "  Full docs: nissia --help"
