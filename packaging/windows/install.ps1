# ENGRAM Installer for Windows
# Run as Administrator: powershell -ExecutionPolicy Bypass -File install.ps1
param(
    [string]$InstallDir = "$env:ProgramFiles\ENGRAM",
    [string]$BinaryPath = ".\engram.exe"
)

$ErrorActionPreference = "Stop"

Write-Host "Installing ENGRAM — Engineering Intelligence, etched in Notion" -ForegroundColor Yellow
Write-Host ""

# Create install directory
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
$ConfigDir = "$InstallDir\config"
$DashboardDir = "$InstallDir\dashboard"
if (!(Test-Path $ConfigDir)) { New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null }
if (!(Test-Path $DashboardDir)) { New-Item -ItemType Directory -Path $DashboardDir -Force | Out-Null }

# Copy binary
Copy-Item $BinaryPath "$InstallDir\engram.exe" -Force
Write-Host "  Binary: $InstallDir\engram.exe" -ForegroundColor Green

# Copy config template if not exists
if (!(Test-Path "$ConfigDir\engram.toml")) {
    Copy-Item ".\engram.toml.example" "$ConfigDir\engram.toml" -Force
    Write-Host "  Config: $ConfigDir\engram.toml" -ForegroundColor Green
} else {
    Write-Host "  Config: $ConfigDir\engram.toml (existing, kept)" -ForegroundColor Cyan
}

# Copy dashboard
if (Test-Path ".\dashboard") {
    Copy-Item ".\dashboard\*" $DashboardDir -Recurse -Force
    Write-Host "  Dashboard: $DashboardDir" -ForegroundColor Green
}

# Add to PATH
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
if ($currentPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$InstallDir", "Machine")
    Write-Host "  Added to system PATH" -ForegroundColor Green
}

# Register as Windows Service using sc.exe
Write-Host ""
Write-Host "Registering Windows service..." -ForegroundColor Yellow
$serviceName = "ENGRAM"
$serviceExists = Get-Service -Name $serviceName -ErrorAction SilentlyContinue

if ($serviceExists) {
    Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
    sc.exe delete $serviceName | Out-Null
    Start-Sleep -Seconds 2
}

# Create wrapper script for service
$wrapperContent = @"
@echo off
cd /d "$ConfigDir"
set ENGRAM_CONFIG=$ConfigDir\engram.toml
"$InstallDir\engram.exe"
"@
Set-Content -Path "$InstallDir\engram-svc.cmd" -Value $wrapperContent

sc.exe create $serviceName binpath= "$InstallDir\engram-svc.cmd" start= auto displayname= "ENGRAM Engineering Intelligence" | Out-Null
sc.exe description $serviceName "AI-powered engineering intelligence platform" | Out-Null
sc.exe start $serviceName | Out-Null

Write-Host "  Service '$serviceName' registered and started" -ForegroundColor Green
Write-Host ""
Write-Host "ENGRAM installed! Open http://localhost:3000 to configure." -ForegroundColor Yellow
Write-Host ""
