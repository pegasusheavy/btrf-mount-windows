#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Installs the Dokan driver for BTRFS Mount Windows.

.DESCRIPTION
    Downloads and installs the Dokan library which provides FUSE-like
    functionality on Windows, enabling userspace filesystem drivers.

.PARAMETER Version
    The version of Dokan to install. Defaults to 2.1.0.0.

.PARAMETER Silent
    Run the installer silently without user interaction.

.EXAMPLE
    .\install-dokan.ps1
    
.EXAMPLE
    .\install-dokan.ps1 -Version "2.1.0.0" -Silent

.NOTES
    Author: Pegasus Heavy Industries LLC
    Requires: Administrator privileges
#>

param(
    [string]$Version = "2.1.0.0",
    [switch]$Silent
)

$ErrorActionPreference = "Stop"

# Dokan download URL
$DokanBaseUrl = "https://github.com/dokan-dev/dokany/releases/download"
$DokanInstallerName = "Dokan_x64.msi"
$DokanUrl = "$DokanBaseUrl/v$Version/$DokanInstallerName"

# Temporary download location
$TempDir = Join-Path $env:TEMP "dokan-install"
$InstallerPath = Join-Path $TempDir $DokanInstallerName

function Write-Header {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  BTRFS Mount Windows - Dokan Installer" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
}

function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Test-DokanInstalled {
    # Check if Dokan driver is installed
    $dokanService = Get-Service -Name "dokan*" -ErrorAction SilentlyContinue
    if ($dokanService) {
        return $true
    }
    
    # Also check for Dokan DLL
    $dokanDll = Join-Path $env:SystemRoot "System32\dokan2.dll"
    return Test-Path $dokanDll
}

function Get-InstalledDokanVersion {
    try {
        $dokanKey = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Dokan*" -ErrorAction SilentlyContinue
        if ($dokanKey) {
            return $dokanKey.DisplayVersion
        }
        
        # Try WOW64 path
        $dokanKey = Get-ItemProperty -Path "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Dokan*" -ErrorAction SilentlyContinue
        if ($dokanKey) {
            return $dokanKey.DisplayVersion
        }
    }
    catch {
        return $null
    }
    return $null
}

function Download-Dokan {
    Write-Host "Downloading Dokan v$Version..." -ForegroundColor Yellow
    
    # Create temp directory
    if (-not (Test-Path $TempDir)) {
        New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    }
    
    # Download installer
    try {
        $webClient = New-Object System.Net.WebClient
        $webClient.DownloadFile($DokanUrl, $InstallerPath)
        Write-Host "  Downloaded to: $InstallerPath" -ForegroundColor Green
    }
    catch {
        Write-Host "  Failed to download Dokan installer!" -ForegroundColor Red
        Write-Host "  URL: $DokanUrl" -ForegroundColor Red
        Write-Host "  Error: $_" -ForegroundColor Red
        throw
    }
}

function Install-Dokan {
    Write-Host "Installing Dokan..." -ForegroundColor Yellow
    
    $msiArgs = @("/i", $InstallerPath)
    
    if ($Silent) {
        $msiArgs += "/quiet", "/norestart"
    }
    else {
        $msiArgs += "/passive", "/norestart"
    }
    
    # Add logging
    $logFile = Join-Path $TempDir "dokan-install.log"
    $msiArgs += "/log", $logFile
    
    Write-Host "  Running installer..." -ForegroundColor Gray
    
    $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $msiArgs -Wait -PassThru
    
    if ($process.ExitCode -eq 0) {
        Write-Host "  Installation completed successfully!" -ForegroundColor Green
        return $true
    }
    elseif ($process.ExitCode -eq 3010) {
        Write-Host "  Installation completed. A reboot is required." -ForegroundColor Yellow
        return $true
    }
    else {
        Write-Host "  Installation failed with exit code: $($process.ExitCode)" -ForegroundColor Red
        Write-Host "  Check log file: $logFile" -ForegroundColor Red
        return $false
    }
}

function Cleanup {
    Write-Host "Cleaning up temporary files..." -ForegroundColor Gray
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Main script
try {
    Write-Header
    
    # Check for admin rights
    if (-not (Test-Administrator)) {
        Write-Host "ERROR: This script requires administrator privileges." -ForegroundColor Red
        Write-Host "Please run PowerShell as Administrator and try again." -ForegroundColor Red
        exit 1
    }
    
    # Check if already installed
    if (Test-DokanInstalled) {
        $installedVersion = Get-InstalledDokanVersion
        if ($installedVersion) {
            Write-Host "Dokan is already installed (version $installedVersion)" -ForegroundColor Green
        }
        else {
            Write-Host "Dokan is already installed" -ForegroundColor Green
        }
        
        $response = Read-Host "Do you want to reinstall/upgrade? (y/N)"
        if ($response -ne "y" -and $response -ne "Y") {
            Write-Host "Installation cancelled." -ForegroundColor Yellow
            exit 0
        }
    }
    
    # Download
    Download-Dokan
    
    # Install
    $success = Install-Dokan
    
    # Cleanup
    Cleanup
    
    if ($success) {
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Green
        Write-Host "  Dokan installation complete!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
        Write-Host ""
        Write-Host "You can now use BTRFS Mount Windows to mount BTRFS volumes." -ForegroundColor Cyan
        Write-Host ""
        
        # Check if reboot needed
        $pendingReboot = Test-Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending"
        if ($pendingReboot) {
            Write-Host "NOTE: A system reboot may be required for the driver to work properly." -ForegroundColor Yellow
        }
    }
    else {
        Write-Host ""
        Write-Host "Installation failed. Please try again or install manually from:" -ForegroundColor Red
        Write-Host "https://github.com/dokan-dev/dokany/releases" -ForegroundColor Cyan
        exit 1
    }
}
catch {
    Write-Host ""
    Write-Host "An error occurred: $_" -ForegroundColor Red
    Cleanup
    exit 1
}
