@echo off
setlocal

set "ROOT=%~dp0"

where pwsh >nul 2>nul
if %ERRORLEVEL%==0 (
  pwsh -NoProfile -ExecutionPolicy Bypass -File "%ROOT%stop.ps1"
  exit /b %ERRORLEVEL%
)

where powershell >nul 2>nul
if %ERRORLEVEL%==0 (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%stop.ps1"
  exit /b %ERRORLEVEL%
)

echo [x-photo] PowerShell not found. Please install PowerShell 5+ or PowerShell 7+.
exit /b 1
