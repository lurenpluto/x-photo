$ErrorActionPreference = "Stop"

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$servicePidFile = Join-Path $rootDir ".service.pid"
$webPidFile = Join-Path $rootDir ".web.pid"

function Stop-ByPidFile {
  param(
    [string]$Name,
    [string]$PidFile
  )

  if (-not (Test-Path $PidFile)) {
    Write-Host "[x-photo] $Name pid file not found: $PidFile"
    return
  }

  $raw = (Get-Content -Path $PidFile -Raw).Trim()
  if ([string]::IsNullOrWhiteSpace($raw)) {
    Write-Host "[x-photo] $Name pid file is empty: $PidFile"
    Remove-Item $PidFile -Force
    return
  }

  $pidValue = 0
  if (-not [int]::TryParse($raw, [ref]$pidValue)) {
    Write-Host "[x-photo] $Name pid file is invalid: $PidFile"
    Remove-Item $PidFile -Force
    return
  }

  $proc = Get-Process -Id $pidValue -ErrorAction SilentlyContinue
  if ($proc) {
    Stop-Process -Id $pidValue -Force
    Write-Host "[x-photo] stopped $Name (pid=$pidValue)"
  } else {
    Write-Host "[x-photo] $Name already stopped (pid=$pidValue)"
  }

  Remove-Item $PidFile -Force
}

Stop-ByPidFile -Name "service" -PidFile $servicePidFile
Stop-ByPidFile -Name "web" -PidFile $webPidFile

Write-Host "[x-photo] stop completed"
