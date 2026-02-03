@echo off
:: BTRFS Mount Windows - Dokan Driver Installer
:: This script will request administrator privileges if needed

:: Check for admin rights
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

:: Run the PowerShell installer
powershell -ExecutionPolicy Bypass -File "%~dp0install-dokan.ps1"
pause
