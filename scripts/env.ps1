<#
.SYNOPSIS
    Sets up a shell that can build Klar on Windows.

.DESCRIPTION
    Four things have to be true before `tauri build` works here, and a fresh
    terminal has none of them. Forgetting one does not produce a message about
    the thing that is missing:

    - MSVC environment. Ninja, unlike MSBuild, does not find the compiler by
      itself. Without it CMake reports that cl.exe cannot compile a test
      program.
    - CMAKE_GENERATOR=Ninja. MSBuild builds ggml's shader compiler as a nested
      project and puts a scratch tree under it that runs past MAX_PATH. Without
      this the failure is "MSB4018 ... exceeds the OS max path limit", three
      thousand lines into a log.
    - A short CARGO_TARGET_DIR. Same limit, and less headroom in release\ than
      in debug\.
    - The updater signing key, for release builds only. Without it the
      installer is produced and then not signed, and the run ends on a line
      about a private key rather than about the build.

    Dot-source it, ". .\scripts\env.ps1", so the variables land in the current
    shell rather than in a child process that exits.

    ASCII only, and no cmdlet newer than PowerShell 5.1: Windows PowerShell
    reads .ps1 files as ANSI rather than UTF-8, so one em dash in a string
    arrives as mojibake and takes the quoting with it.

.PARAMETER Sign
    Also load the updater signing key, for a build that will be published.
    Prompts for the password rather than taking it as an argument, so it stays
    out of the shell history.

.EXAMPLE
    . .\scripts\env.ps1
    npm run tauri build -- --features vulkan

.EXAMPLE
    . .\scripts\env.ps1 -Sign
    npm run tauri build -- --features vulkan --config src-tauri/tauri.release.conf.json
#>

[CmdletBinding()]
param(
    [switch]$Sign,
    [string]$TargetDir = "C:\kv",
    [string]$Key = "$HOME\.klar\updater.key"
)

$ErrorActionPreference = "Stop"

# Already inside a Developer Shell? cl.exe on PATH is the honest test. Entering
# one twice appends to PATH and eventually breaks it.
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $vs = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
    if (-not (Test-Path $vs)) {
        $vs = "C:\Program Files\Microsoft Visual Studio\2022\Community"
    }
    if (-not (Test-Path $vs)) {
        throw "no Visual Studio 2022 build tools found. See the README's Windows prerequisites."
    }

    Import-Module "$vs\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    Enter-VsDevShell -VsInstallPath $vs -DevCmdArguments "-arch=x64" | Out-Null
    Write-Host "msvc      $vs"
}
else {
    Write-Host "msvc      already on PATH"
}

$env:CARGO_TARGET_DIR = $TargetDir
$env:CMAKE_GENERATOR = "Ninja"

Write-Host "target    $env:CARGO_TARGET_DIR"
Write-Host "generator $env:CMAKE_GENERATOR"

if (-not (Get-Command ninja.exe -ErrorAction SilentlyContinue)) {
    Write-Host "ninja     NOT FOUND. winget install Ninja-build.Ninja" -ForegroundColor Red
}

if (-not $env:VULKAN_SDK) {
    Write-Host "vulkan    VULKAN_SDK is not set. See the README" -ForegroundColor Yellow
}
else {
    Write-Host "vulkan    $env:VULKAN_SDK"
}

if ($Sign) {
    if (-not (Test-Path $Key)) {
        throw "no signing key at $Key. Generate one: npm run tauri signer generate -- -w $Key"
    }

    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $Key -Raw

    # Read rather than take as a parameter: an argument ends up in the shell
    # history, and this secret cannot be rotated without breaking updates for
    # every copy already installed.
    $secure = Read-Host "Updater key password (empty if you set none)" -AsSecureString
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD =
        [System.Runtime.InteropServices.Marshal]::PtrToStringAuto(
            [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))

    Write-Host "signing   key loaded from $Key"
    Write-Host ""
    Write-Host "npm run tauri build -- --features vulkan --config src-tauri/tauri.release.conf.json"
}
else {
    Write-Host ""
    Write-Host "npm run tauri build -- --features vulkan"
}
