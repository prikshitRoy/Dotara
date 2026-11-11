<#
.SYNOPSIS
  Prepares the VLC runtime that is bundled into Dotara (src-tauri/vlc/).

.DESCRIPTION
  Copies libvlc.dll, libvlccore.dll and the playback plugins from a VLC 3.x (64-bit)
  installation - or from the official portable zip, downloaded if no install is
  found - and regenerates the plugin cache (plugins.dat) for fast startup.

  Modules the app never uses (VLC's own GUI, Lua scripts, streaming/transcoding
  output, network service discovery, remote-control interfaces) are left out.

  Idempotent: does nothing if src-tauri/vlc already exists, unless -Force is given.

.PARAMETER VlcDir
  Folder of an existing VLC install. Defaults to the standard install locations.

.PARAMETER Version
  VLC version to download when no install is found.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/prepare-vlc.ps1 -Force
#>
param(
    [string]$VlcDir,
    [string]$Version = "3.0.21",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\vlc"

if ((Test-Path (Join-Path $dest "libvlc.dll")) -and -not $Force) {
    Write-Host "[prepare-vlc] VLC runtime already present at $dest (use -Force to refresh)"
    exit 0
}

# --- locate a VLC source -----------------------------------------------------
if (-not $VlcDir) {
    $VlcDir = @(
        "$env:ProgramFiles\VideoLAN\VLC",
        "$env:ProgramW6432\VideoLAN\VLC"
    ) | Where-Object { $_ -and (Test-Path (Join-Path $_ "libvlc.dll")) } | Select-Object -First 1
}

if (-not $VlcDir) {
    $zipUrl = "https://download.videolan.org/pub/videolan/vlc/$Version/win64/vlc-$Version-win64.zip"
    $tmp = Join-Path $env:TEMP "dotara-vlc-$Version"
    $zip = "$tmp.zip"
    Write-Host "[prepare-vlc] No VLC install found, downloading $zipUrl"
    Invoke-WebRequest -Uri $zipUrl -OutFile $zip -UseBasicParsing
    if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
    Expand-Archive $zip -DestinationPath $tmp
    $VlcDir = (Get-ChildItem $tmp -Directory | Select-Object -First 1).FullName
}

$arch = (Get-Item (Join-Path $VlcDir "libvlc.dll")).VersionInfo
Write-Host "[prepare-vlc] Using VLC $($arch.ProductVersion) from $VlcDir"

# --- copy the runtime --------------------------------------------------------
if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
New-Item -ItemType Directory -Force $dest | Out-Null

foreach ($f in "libvlc.dll", "libvlccore.dll", "COPYING.txt", "AUTHORS.txt") {
    Copy-Item (Join-Path $VlcDir $f) $dest
}

$excludedPluginDirs = @(
    "gui",                # VLC's own Qt / skins interface
    "lua",                # Lua extensions & playlist scripts
    "control",            # hotkeys, remote-control interfaces
    "access_output",      # streaming output
    "stream_out",         # transcoding / streaming
    "mux",                # muxers (only used by stream_out)
    "services_discovery"  # UPnP, SAP, etc.
)

$srcPlugins = Join-Path $VlcDir "plugins"
$dstPlugins = Join-Path $dest "plugins"
New-Item -ItemType Directory -Force $dstPlugins | Out-Null
Get-ChildItem $srcPlugins -Directory |
    Where-Object { $excludedPluginDirs -notcontains $_.Name } |
    ForEach-Object { Copy-Item $_.FullName $dstPlugins -Recurse }

# --- regenerate the plugin cache for the trimmed set -------------------------
$cacheGen = Join-Path $VlcDir "vlc-cache-gen.exe"
if (Test-Path $cacheGen) {
    & $cacheGen $dstPlugins
    if ($LASTEXITCODE -ne 0) { Write-Warning "vlc-cache-gen exited with $LASTEXITCODE" }
} else {
    Write-Warning "vlc-cache-gen.exe not found; VLC will scan plugins on first start"
}

$size = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("[prepare-vlc] Done: {0:N1} MB in {1}" -f $size, $dest)
