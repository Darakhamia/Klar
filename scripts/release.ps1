<#
.SYNOPSIS
    Builds the update manifest from what the build actually produced.

.DESCRIPTION
    A wrong `signature` field in latest.json breaks updates for everybody at
    once, and it does it late: the manifest parses, the version shows, the
    button appears, the installer downloads in full, and only then does the
    signature check refuse it. One mis-copied character out of a few hundred
    does that. So the field is never typed: it is read out of the .sig file the
    build wrote.

    This script only writes the manifest and checks it against reality. Upload
    is manual scp, on purpose: the box that receives it is a static nginx and
    nothing here should be able to reach it.

    ASCII only, and no cmdlet newer than PowerShell 5.1. Windows PowerShell
    reads .ps1 files as ANSI rather than UTF-8, so a single em dash in a string
    arrives as three bytes of mojibake and takes the quoting with it.

.PARAMETER Bundle
    Where `tauri build` left the installer. Defaults to the bundle directory
    under CARGO_TARGET_DIR, or ./src-tauri/target when that is unset.

.PARAMETER Notes
    What this release changed, shown to the user before they install it. Taken
    from the matching section of CHANGELOG.md when omitted.

.PARAMETER Verify
    Check that the URL the manifest points at is actually live. Run this AFTER
    uploading the installer, in the order the script prints.

.EXAMPLE
    .\scripts\release.ps1
    .\scripts\release.ps1 -Verify
#>

[CmdletBinding()]
param(
    [string]$Bundle,
    [string]$Notes,
    [switch]$Verify,

    # The server's own address, not the domain. getklar.net is proxied through
    # Cloudflare, which resolves to Cloudflare and forwards HTTP and HTTPS only:
    # scp to the hostname reaches a machine that does not answer on port 22.
    [string]$Host_ = "65.108.92.69",

    # Symlinks on the server, standing in for the Coolify bind mounts under
    # /data/coolify/applications/<id>/.
    [string]$DownloadsPath = "/root/klar-dl/",
    [string]$UpdatesPath = "/root/klar-up/"
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
    throw "plugins.updater.pubkey is empty in tauri.conf.json. This build cannot ship updates."
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
    $name = $installer.Name
    throw "$name has no .sig beside it, so it cannot be offered as an update. The build needs both the signing key and the release config: . .\scripts\env.ps1 -Sign, then npm run tauri build -- --features vulkan --config src-tauri/tauri.release.conf.json"
}

# Existing is not enough: it has to belong to THIS installer. When signing fails
# -- a mistyped key password is the way -- the bundle is still written and the
# .sig from the previous build of the same version is still lying beside it.
# Everything downstream then looks correct: the manifest has a real signature of
# a real installer, just not of this one. Clients download ten megabytes and
# refuse it, and the first sign of trouble is on their machine rather than here.
#
# Signing is the last step of the bundle, so a good .sig is always the newer
# file. Nothing more subtle is possible without an Ed25519 verifier, which
# Windows PowerShell 5.1 does not have.
$sigWritten = (Get-Item $sig).LastWriteTimeUtc
if ($sigWritten -lt $installer.LastWriteTimeUtc) {
    $age = [math]::Round(($installer.LastWriteTimeUtc - $sigWritten).TotalMinutes)
    throw "the .sig beside $($installer.Name) is $age minutes older than the installer, so it signs a build that no longer exists. Signing failed and this one went out unsigned -- scroll up in the build output for the line about the private key. Fix the password and build again; do not upload anything from this run."
}

# Read, never retype. -Raw then trim: the file is one long line and a trailing
# newline is not part of the signature.
$signature = (Get-Content $sig -Raw).Trim()
if ($signature.Length -lt 64) {
    $length = $signature.Length
    throw "the signature in $sig looks truncated: $length characters."
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
        # at all: an unsigned bundle gets a new signature on every build, macOS
        # ties the Accessibility permission to that signature, and Klar without
        # Accessibility can neither see the hotkey nor insert text.
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = $url
        }
    }
}

$out = Join-Path $root "dist-release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$manifestPath = Join-Path $out "latest.json"

# WriteAllText rather than Set-Content: -Encoding utf8NoBOM does not exist in
# Windows PowerShell 5.1, and its plain "utf8" writes a BOM that some servers
# and parsers hand straight through into the first key.
$json = $manifest | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($manifestPath, $json, (New-Object System.Text.UTF8Encoding($false)))

$megabytes = [math]::Round($installer.Length / 1MB)
Write-Host ""
Write-Host "installer $($installer.Name)  $megabytes MB"
Write-Host "manifest  $manifestPath"
Write-Host ""

if ($Verify) {
    Write-Host "checking $url"
    try {
        $head = Invoke-WebRequest -Uri $url -Method Head -MaximumRedirection 3 -UseBasicParsing
        Write-Host "  $($head.StatusCode)  $($head.Headers['Content-Length']) bytes" -ForegroundColor Green
    }
    catch {
        Write-Host "  the installer is not there yet: $($_.Exception.Message)" -ForegroundColor Yellow
        Write-Host "  upload it before the manifest, or clients get a manifest naming a missing file."
        exit 1
    }

    $live = "$base/updates/latest.json"
    Write-Host "checking $live"
    try {
        $served = Invoke-WebRequest -Uri $live -Headers @{ "Cache-Control" = "no-cache" } -UseBasicParsing
        $type = $served.Headers['Content-Type']
        $cache = $served.Headers['Cache-Control']

        # A site with a single-page fallback answers a missing file with 200 and
        # the landing page, not 404. The updater would then try to parse HTML as
        # JSON and report something about the server being unreachable. Caught
        # here, where it can be named.
        if ($type -notmatch "json") {
            Write-Host "  200, but Content-Type is '$type', not JSON." -ForegroundColor Red
            Write-Host "  The manifest is not there and the site answered with a page instead." -ForegroundColor Red
            Write-Host "  /updates/ must 404 on a missing file rather than fall through to index.html." -ForegroundColor Red
            exit 1
        }

        $servedVersion = ($served.Content | ConvertFrom-Json).version
        Write-Host "  serving $servedVersion  (Cache-Control: $cache)" -ForegroundColor Green

        if ($servedVersion -ne $version) {
            Write-Host "  still the old manifest. Upload it, then purge Cloudflare for this URL." -ForegroundColor Yellow
        }
        if ($cache -match "immutable|max-age=(\d{5,})") {
            Write-Host "  this manifest is cached for a long time. Nobody will see the next release." -ForegroundColor Red
            Write-Host "  /updates/ needs its own nginx location. See the README." -ForegroundColor Red
        }
    }
    catch {
        Write-Host "  not published yet: $($_.Exception.Message)" -ForegroundColor Yellow
    }
    exit 0
}

# Order matters. A manifest naming a file which is not there yet is a broken
# update for everybody who checks in that window.
Write-Host "Upload in this order:" -ForegroundColor Cyan
Write-Host "  scp `"$($installer.FullName)`" root@${Host_}:$DownloadsPath"
Write-Host "  scp `"$manifestPath`" root@${Host_}:$UpdatesPath"
Write-Host "  ssh root@${Host_} `"chmod 644 $DownloadsPath* $UpdatesPath*`""
Write-Host ""
Write-Host "Then confirm it landed:"
Write-Host "  .\scripts\release.ps1 -Verify"
