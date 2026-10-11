# Remove everything install.ps1 put into the system.
# Run from an elevated PowerShell:
#   powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
#Requires -RunAsAdministrator
$ErrorActionPreference = "Stop"

Unregister-ScheduledTask -TaskName "titan-kbd-light" -Confirm:$false -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $env:ProgramFiles "titan-kbd-light") -ErrorAction SilentlyContinue

Write-Host "Uninstalled. The backlight stays as it is until the next reboot."
