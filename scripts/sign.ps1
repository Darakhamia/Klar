<#
.SYNOPSIS
    Switches Windows code signing on for a Klar build.

.DESCRIPTION
    Two signatures are involved in shipping Klar and they have nothing to do
    with each other. Confusing them wastes an afternoon:

    - The **updater** signature is minisign, it is already working, and it is
      what stops somebody serving a hostile installer to every copy of Klar in
      the field. Its key lives in $HOME\.klar\updater.key. This script does not
      touch it.
    - The **code** signature is Authenticode, it is what this script is for, and
      it is what stops Windows telling the person installing Klar that nobody
      knows who wrote it.

    See docs/signing.md for which certificate to buy and why. This script does
    the two mechanical parts: proving the signing path works before any money
    is spent, and pointing a real build at a real certificate.

    ASCII only, and nothing newer than PowerShell 5.1 -- Windows PowerShell
    reads .ps1 as ANSI, so one em dash arrives as mojibake and takes the
    quoting with it.

.PARAMETER Rehearse
    Create a self-signed certificate and configure the build to use it. This
    proves signtool.exe is found, the config overlay merges, and the installer
    comes out signed. It proves nothing about trust: a self-signed build is
    still refused by SmartScreen, and arguably looks worse than an unsigned one
    because Windows can now see a signature and reject it. Never publish one.

.PARAMETER Thumbprint
    Use a certificate already in Cert:\CurrentUser\My -- an OV or EV
    certificate on a token, or one imported from a cloud HSM.

.PARAMETER Azure
    Configure Azure Trusted Signing instead of a local certificate, in the form
    endpoint,account,profile. Needs trusted-signing-cli on PATH.

.PARAMETER Verify
    Check the signature on a file that was already built.

.PARAMETER Off
    Delete the overlay, so the next build is unsigned again.

.EXAMPLE
    . .\scripts\env.ps1
    .\scripts\sign.ps1 -Rehearse
    npm run tauri build -- --features vulkan -c src-tauri/tauri.signing.conf.json
    .\scripts\sign.ps1 -Verify C:\kv\release\bundle\nsis\Klar_0.3.1_x64-setup.exe

.EXAMPLE
    .\scripts\sign.ps1 -Azure "https://weu.codesigning.azure.net,klar,klar-release"
#>

[CmdletBinding(DefaultParameterSetName = "None")]
param(
    [Parameter(ParameterSetName = "Rehearse")][switch]$Rehearse,
    [Parameter(ParameterSetName = "Thumbprint")][string]$Thumbprint,
    [Parameter(ParameterSetName = "Azure")][string]$Azure,
    [Parameter(ParameterSetName = "Verify")][string]$Verify,
    [Parameter(ParameterSetName = "Off")][switch]$Off
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$overlay = Join-Path $root "src-tauri\tauri.signing.conf.json"
$subject = "CN=Klar rehearsal - NOT a real publisher"

function Write-Overlay($windows) {
    # Only the keys that differ. The build is run with this file after
    # tauri.conf.json, and Tauri merges them in the order given.
    $config = @{
        '$schema' = "https://schema.tauri.app/config/2"
        bundle    = @{ windows = $windows }
    }
    $config | ConvertTo-Json -Depth 6 | Set-Content -Path $overlay -Encoding ASCII
    Write-Host "overlay   $overlay"
    Write-Host ""
    Write-Host "npm run tauri build -- --features vulkan -c src-tauri/tauri.release.conf.json -c src-tauri/tauri.signing.conf.json"
}

function Find-SignTool {
    $found = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    # Outside a Developer Shell it is only under the SDK, in a directory named
    # after a version nobody remembers.
    $sdk = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    if (Test-Path $sdk) {
        $candidate = Get-ChildItem "$sdk\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
            Sort-Object FullName | Select-Object -Last 1
        if ($candidate) { return $candidate.FullName }
    }
    return $null
}

if ($Off) {
    if (Test-Path $overlay) {
        Remove-Item $overlay
        Write-Host "removed   $overlay"
    }
    else {
        Write-Host "nothing to remove; builds are already unsigned"
    }
    return
}

if ($Verify) {
    if (-not (Test-Path $Verify)) { throw "no such file: $Verify" }
    $signtool = Find-SignTool
    if (-not $signtool) { throw "signtool.exe not found. Run . .\scripts\env.ps1 first, or install the Windows SDK." }

    Write-Host "verifying $Verify"
    Write-Host ""
    # /pa is the Authenticode policy -- the one Windows itself applies. Without
    # it signtool uses the driver policy and reports failures that do not
    # matter here.
    & $signtool verify /pa /v $Verify
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "Not trusted. Expected for a rehearsal certificate: it chains to itself" -ForegroundColor Yellow
        Write-Host "and nothing on the machine vouches for it. What matters is the lines above" -ForegroundColor Yellow
        Write-Host "showing a signature and a timestamp at all -- that is the part a real" -ForegroundColor Yellow
        Write-Host "certificate will inherit." -ForegroundColor Yellow
    }
    return
}

if ($Rehearse) {
    $existing = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
        Select-Object -First 1

    if ($existing) {
        Write-Host "reusing   rehearsal certificate, expires $($existing.NotAfter.ToString('yyyy-MM-dd'))"
        $cert = $existing
    }
    else {
        # Three months. Long enough to finish the work, short enough that a
        # forgotten one stops working rather than quietly staying valid.
        $cert = New-SelfSignedCertificate `
            -Type CodeSigningCert `
            -Subject $subject `
            -CertStoreLocation Cert:\CurrentUser\My `
            -KeyUsage DigitalSignature `
            -KeyExportPolicy NonExportable `
            -NotAfter (Get-Date).AddMonths(3)
        Write-Host "created   rehearsal certificate, expires $($cert.NotAfter.ToString('yyyy-MM-dd'))"
    }

    Write-Host ""
    Write-Host "This signature is worth nothing to Windows. Do not publish a build made" -ForegroundColor Yellow
    Write-Host "with it: SmartScreen refuses a signature it cannot chain, and that reads" -ForegroundColor Yellow
    Write-Host "worse to a user than no signature at all." -ForegroundColor Yellow
    Write-Host ""

    Write-Overlay @{
        certificateThumbprint = $cert.Thumbprint
        digestAlgorithm       = "sha256"
        timestampUrl          = "http://timestamp.digicert.com"
    }
    return
}

if ($Thumbprint) {
    $clean = ($Thumbprint -replace '[^0-9A-Fa-f]', '').ToUpper()
    $cert = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Thumbprint -eq $clean } | Select-Object -First 1
    if (-not $cert) {
        throw "no certificate with thumbprint $clean in Cert:\CurrentUser\My. If it lives on a USB token, plug it in and let the vendor's middleware register it."
    }
    if ($cert.NotAfter -lt (Get-Date)) {
        throw "that certificate expired on $($cert.NotAfter.ToString('yyyy-MM-dd'))."
    }

    Write-Host "using     $($cert.Subject)"
    Write-Host "expires   $($cert.NotAfter.ToString('yyyy-MM-dd'))"
    Write-Host ""
    Write-Overlay @{
        certificateThumbprint = $clean
        digestAlgorithm       = "sha256"
        timestampUrl          = "http://timestamp.digicert.com"
    }
    return
}

if ($Azure) {
    $parts = $Azure.Split(",")
    if ($parts.Count -ne 3) {
        throw "expected endpoint,account,profile -- for example https://weu.codesigning.azure.net,klar,klar-release"
    }
    if (-not (Get-Command trusted-signing-cli -ErrorAction SilentlyContinue)) {
        Write-Host "trusted-signing-cli is not on PATH: cargo install trusted-signing-cli" -ForegroundColor Yellow
    }

    # %1 is Tauri's placeholder for the file being signed. There is no
    # thumbprint here on purpose: Trusted Signing issues a fresh certificate
    # per signature, valid for three days, so there is nothing stable to name
    # and nothing on disk to lose.
    $command = "trusted-signing-cli -e $($parts[0].Trim()) -a $($parts[1].Trim()) -c $($parts[2].Trim()) %1"
    Write-Host "sign      $command"
    Write-Host ""
    Write-Host "Azure credentials come from the environment, not from this file:" -ForegroundColor Yellow
    Write-Host "AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET." -ForegroundColor Yellow
    Write-Host ""
    Write-Overlay @{ signCommand = $command }
    return
}

Write-Host "Nothing asked for. One of:"
Write-Host "  .\scripts\sign.ps1 -Rehearse                 self-signed, to prove the path works"
Write-Host "  .\scripts\sign.ps1 -Thumbprint <hex>         a real certificate in the store"
Write-Host "  .\scripts\sign.ps1 -Azure <endpoint,acct,profile>"
Write-Host "  .\scripts\sign.ps1 -Verify <path to exe>"
Write-Host "  .\scripts\sign.ps1 -Off"
Write-Host ""
Write-Host "Read docs\signing.md first. It is the part that costs money."
