# OptiRAR Release Build Script
param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "OptiRAR Production Release Build" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# LLVM-MinGW toolchain: $env:LLVM_MINGW_BIN, or an llvm-mingw-* folder next to CARGO_HOME.
$toolchainPath = $env:LLVM_MINGW_BIN
if (-not $toolchainPath -and $env:CARGO_HOME) {
    $found = Get-ChildItem (Split-Path -Parent $env:CARGO_HOME) -Directory -Filter "llvm-mingw-*" -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($found) { $toolchainPath = Join-Path $found.FullName "bin" }
}
if ($toolchainPath -and (Test-Path $toolchainPath)) {
    $env:PATH = "$toolchainPath;" + $env:PATH
    Write-Host "Loaded MinGW toolchain: $toolchainPath" -ForegroundColor Green
}

# Keep local folder names out of the binaries (Rust embeds source paths for panic messages).
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE ".rustup" }
# CARGO_ENCODED_RUSTFLAGS (0x1F-separated) because the paths may contain spaces.
$env:CARGO_ENCODED_RUSTFLAGS = @(
    "--remap-path-prefix=$repoRoot=optirar",
    "--remap-path-prefix=$cargoHome=cargo",
    "--remap-path-prefix=$rustupHome=rustup"
) -join [char]0x1f

# Run Cargo release build
Write-Host "`nCompiling OptiRAR CLI and Desktop GUI with LTO thin and symbol stripping..." -ForegroundColor Yellow
cargo build --release --workspace

if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo release build failed."
    exit 1
}

# Create distribution directory
if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$cliSrc = "target\release\optirar.exe"
$guiSrc = "target\release\optirar-desktop.exe"

Copy-Item $cliSrc -Destination "$OutDir\optirar.exe" -Force
Copy-Item $guiSrc -Destination "$OutDir\optirar-desktop.exe" -Force

# Copy shell scripts & assets
Copy-Item "scripts\register_shell.ps1" -Destination "$OutDir\register_shell.ps1" -Force -ErrorAction SilentlyContinue
Copy-Item "scripts\unregister_shell.ps1" -Destination "$OutDir\unregister_shell.ps1" -Force -ErrorAction SilentlyContinue
foreach ($f in @("install.ps1", "uninstall.ps1", "Setup.cmd")) {
    Copy-Item "scripts\$f" -Destination "$OutDir\$f" -Force
}

if (-not (Test-Path "$OutDir\locales")) {
    New-Item -ItemType Directory -Path "$OutDir\locales" | Out-Null
}
Copy-Item "assets\locales\*" -Destination "$OutDir\locales\" -Recurse -Force -ErrorAction SilentlyContinue

# Copy documentation
Copy-Item "README.md" -Destination "$OutDir\README.md" -Force -ErrorAction SilentlyContinue
Copy-Item "LICENSE" -Destination "$OutDir\LICENSE" -Force -ErrorAction SilentlyContinue

Write-Host "`nRelease build complete!" -ForegroundColor Green
Write-Host "Binaries located in: $(Resolve-Path $OutDir)" -ForegroundColor Green

Get-ChildItem -Path $OutDir | Format-Table Name, Length, LastWriteTime
