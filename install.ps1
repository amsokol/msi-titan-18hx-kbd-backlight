# Build titan-kbd-light.exe (if cargo is available) and register a Task
# Scheduler task (dist/windows/titan-kbd-light.xml) that runs it as SYSTEM at
# boot, at any logon (boot does not fire after a Fast Startup "shutdown") and
# after resume.
# Run from an elevated PowerShell:
#   powershell -ExecutionPolicy Bypass -File .\install.ps1 [-Color ff0000]
# NOT TESTED on real hardware yet.
#Requires -RunAsAdministrator
param(
    [ValidatePattern('^([0-9a-fA-F]{6}|off)$')]
    [string]$Color = "ff0000"
)
$ErrorActionPreference = "Stop"

$TaskName = "titan-kbd-light"
$exe = Join-Path $PSScriptRoot "target\release\titan-kbd-light.exe"

if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Push-Location $PSScriptRoot
    try { cargo build --release } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
if (-not (Test-Path $exe)) {
    throw "$exe not found: install Rust (https://rustup.rs) or put a built exe there"
}

$dest = Join-Path $env:ProgramFiles "titan-kbd-light"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
# The task may still hold the old exe for a moment after a previous run.
Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
Copy-Item $exe $dest -Force
$installed = Join-Path $dest "titan-kbd-light.exe"

$xml = Get-Content -Raw (Join-Path $PSScriptRoot "dist\windows\titan-kbd-light.xml")
$xml = $xml.Replace("__EXE__", $installed).Replace("__COLOR__", $Color.ToLower())
Register-ScheduledTask -TaskName $TaskName -Xml $xml -Force | Out-Null
Start-ScheduledTask -TaskName $TaskName

Write-Host "Installed $installed, color $Color. Change it: .\install.ps1 -Color RRGGBB"
