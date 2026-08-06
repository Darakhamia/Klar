<#
.SYNOPSIS
    Builds the update manifest from what the build actually produced.

.DESCRIPTION
    A wrong `signature` field in latest.json breaks updates for everybody at
    once, and it does it late: the manifest parses, the version shows, the
    button appears, the installer downloads in full, and only then does the
    signature check refuse it. One mis-copied character out of a few hundred
    does that. So the field is never typed — it is read out of the .sig file the
    build wrote.

    This script only writes the manifest and checks it against reality. Upload
    is manual scp, on purpose: the box that receives it is a static nginx and
    nothing here should be able to reach it.

.PARAMETER Bundle
    Where `tauri build` left the installer. Defaults to the bundle directory
    under CARGO_TARGET_DIR, or ./src-tauri/target when that is unset.

.PARAMETER Notes
    What this release changed, shown to the user before they install it. Taken
    from the matching section of CHANGELOG.md when omitted.

.PARAMETER Verify
    Check that the URL the manifest points at is actually live. Run this *after*
    uploading the installer — see the order the script prints.

.EXAMPLE
    .\scripts\release.ps1
    .\scripts\release.ps1 -Verify
#>

[CmdletBinding()]
param(
    [string]$Bundle,
    [string]$Notes,
    [switch]$Verify
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$conf = Join-Path $root "src-tauri\tauri.conf.json"

# The version comes from the file that names the installer, so the manifest
# cannot claim a version the build did not produce.
$config = Get-Content $conf -Raw | ConvertFrom-Json
$version = $config.version
$base = $config.plugins.updater.endpoints[0] -replace '/updates/latest\.json$', ''

if (-not $config.plugins.updater.pubkey) {
    throw "plugins.updater.pubkey is empty in tauri.conf.json — this build cannot ship updates."
}

Write-Host "version   $version"
Write-Host "site      $base"

if (-not $Bundle) {
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root "src-tauri\target" }
    $Bundle = Join-Path $target "release\bundle\nsis"
}

if (-not (Test-Path $Bundle)) {
    throw "no bundle directory at $Bundle. Build first, or pass -Bundle."
}

# Match on the version rather than taking whatever is newest: a stale installer
# from the previous release sitting in the same directory is exactly the mistake
# this is here to catch.
$installer = Get-ChildItem $Bundle -Filter "*_${version}_*-setup.exe" | Select-Object -First 1
if (-not $installer) {
    throw "no installer for $version in $Bundle. Did the version bump land before the build?"
}

$sig = "$($installer.FullName).sig"
if (-not (Test-Path $sig)) {
    throw @"
$($installer.Name) has no .sig beside it, so it cannot be offered as an update.

The build needs both the signing key and the release config:
  `$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content `$HOME\.klar\updater.key -Raw
  `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "..."
  npm run tauri build -- --features vulkan --config src-tauri/tauri.release.conf.json
"@
}

# Read, never retype. `-Raw` then trim: the file is one long line and a trailing
# newline is not part of the signature.
$signature = (Get-Content $sig -Raw).Trim()
if ($signature.Length -lt 64) {
    throw "the signature in $sig looks truncated ($($signature.Length) characters)."
}

if (-not $Notes) {
    # The section for this version out of the changelog, so release notes are
    # written once rather than twice.
    $changelog = Join-Path $root "CHANGELOG.md"
    if (Test-Path $changelog) {
        $text = Get-Content $changelog -Raw
        if ($text -match "(?ms)^## $([regex]::Escape($version))\s*\r?\n(.+?)(?=^## |\z)") {
            $Notes = ($Matches[1] -replace '\s+', ' ').Trim()
            if ($Notes.Length -gt 400) { $Notes = $Notes.Substring(0, 397) + "..." }
        }
    }
}
if (-not $Notes) { $Notes = "See getklar.net for what changed." }

$url = "$base/downloads/$($installer.Name)"

$manifest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        # Only Windows. macOS updates need a Developer ID before they can work
        # at all: an unsigned bundle loses its Accessibility permission on every
        # update, and Klar without Accessibility cannot see the hotkey or insert
        # text. See the README.
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = $url
        }
    }
}

$out = Join-Path $root "dist-release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$manifestPath = Join-Path $out "latest.json"
$manifest | ConvertTo-Json -Depth 5 | Set-Content $manifestPath -Encoding utf8NoBOM

Write-Host ""
Write-Host "installer $($installer.Name)  $([math]::Round($installer.Length / 1MB)) MB"
Write-Host "manifest  $manifestPath"
Write-Host ""

if ($Verify) {
    Write-Host "checking $url"
    try {
        $head = Invoke-WebRequest -Uri $url -Method Head -MaximumRedirection 3
        Write-Host "  $($head.StatusCode)  $($head.Headers['Content-Length']) bytes" -ForegroundColor Green
    }
    catch {
        Write-Host "  the installer is not there yet: $($_.Exception.Message)" -ForegroundColor Yellow
        Write-Host "  upload it before the manifest, or clients get a manifest promising a missing file."
        exit 1
    }

    $live = "$base/updates/latest.json"
    Write-Host "checking $live"
    try {
        $served = Invoke-WebRequest -Uri $live -Headers @{ "Cache-Control" = "no-cache" }
        $servedVersion = ($served.Content | ConvertFrom-Json).version
        $cache = $served.Headers['Cache-Control']
        Write-Host "  serving $servedVersion  (Cache-Control: $cache)" -ForegroundColor Green
        if ($servedVersion -ne $version) {
            Write-Host "  still the old manifest — upload it, then purge Cloudflare for this URL." -ForegroundColor Yellow
        }
        if ($cache -match "immutable|max-age=(\d{5,})") {
            Write-Host "  this manifest is cached for a long time. Nobody will see the next release." -ForegroundColor Red
            Write-Host "  /updates/ needs its own nginx location — see README." -ForegroundColor Red
        }
    }
    catch {
        Write-Host "  not published yet: $($_.Exception.Message)" -ForegroundColor Yellow
    }
    exit 0
}

# Order matters. A manifest that names a file which is not there yet is a
# broken update for everybody who checks in that window.
Write-Host "Upload in this order:" -ForegroundColor Cyan
Write-Host "  scp `"$($installer.FullName)`" root@getklar.net:/path/to/downloads/"
Write-Host "  scp `"$manifestPath`" root@getklar.net:/path/to/updates/"
Write-Host ""
Write-Host "Then purge the Cloudflare cache for $base/updates/latest.json,"
Write-Host "and confirm it landed:"
Write-Host "  .\scripts\release.ps1 -Verify"
