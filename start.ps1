$ErrorActionPreference = "Stop"

function Get-EnvOrDefault {
  param(
    [string]$Name,
    [string]$Default
  )
  $value = [Environment]::GetEnvironmentVariable($Name)
  if ([string]::IsNullOrWhiteSpace($value)) {
    return $Default
  }
  return $value
}

function Show-LogTail {
  param(
    [string]$Path,
    [string]$Label,
    [int]$Lines = 25
  )

  if (-not (Test-Path $Path)) {
    return
  }

  Write-Host "[x-photo] ---- $Label (last $Lines lines) ----"
  Get-Content -Path $Path -Tail $Lines | ForEach-Object { Write-Host $_ }
}

function Assert-ProcessAlive {
  param(
    [System.Diagnostics.Process]$Process,
    [string]$Name,
    [string]$StdoutLog,
    [string]$StderrLog
  )

  if (-not $Process) {
    throw "[x-photo] $Name process failed to start"
  }

  $Process.Refresh()
  if ($Process.HasExited) {
    Write-Host "[x-photo] $Name failed shortly after startup (exit=$($Process.ExitCode))."
    Write-Host "[x-photo] Common reason: port already in use."
    Show-LogTail -Path $StderrLog -Label "$Name stderr"
    Show-LogTail -Path $StdoutLog -Label "$Name stdout"
    throw "[x-photo] $Name startup failed"
  }
}

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$serviceManifest = Join-Path $rootDir "service/Cargo.toml"
$webDir = Join-Path $rootDir "web"

$serviceHost = Get-EnvOrDefault -Name "SERVICE_HOST" -Default "127.0.0.1"
$servicePort = Get-EnvOrDefault -Name "SERVICE_PORT" -Default "8080"
$webHost = Get-EnvOrDefault -Name "WEB_HOST" -Default "127.0.0.1"
$webPort = Get-EnvOrDefault -Name "WEB_PORT" -Default "5174"

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
  Write-Error "[x-photo] cargo not found. Please install Rust toolchain first."
}

$pythonExe = $null
$pythonArgsPrefix = @()
if (Get-Command py -ErrorAction SilentlyContinue) {
  $pythonExe = "py"
  $pythonArgsPrefix = @("-3")
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
  $pythonExe = "python"
} else {
  Write-Error "[x-photo] python/py not found. Please install Python first."
}

Write-Host "[x-photo] Building service..."
& cargo build --manifest-path $serviceManifest

$serviceLog = Join-Path $rootDir ".service.log"
$serviceErrLog = Join-Path $rootDir ".service.err.log"
$webLog = Join-Path $rootDir ".web.log"
$webErrLog = Join-Path $rootDir ".web.err.log"
$servicePidFile = Join-Path $rootDir ".service.pid"
$webPidFile = Join-Path $rootDir ".web.pid"

$serviceProc = $null
$webProc = $null

try {
  Write-Host "[x-photo] Starting service on http://$serviceHost`:$servicePort ..."
  $serviceProc = Start-Process -FilePath "cargo" -ArgumentList @("run", "--manifest-path", $serviceManifest, "--bin", "service") -WorkingDirectory $rootDir -RedirectStandardOutput $serviceLog -RedirectStandardError $serviceErrLog -PassThru
  Set-Content -Path $servicePidFile -Value $serviceProc.Id -NoNewline

  Write-Host "[x-photo] Starting web on http://$webHost`:$webPort ..."
  $webArgs = @() + $pythonArgsPrefix + @("-m", "http.server", $webPort, "--bind", $webHost, "--directory", $webDir)
  $webProc = Start-Process -FilePath $pythonExe -ArgumentList $webArgs -WorkingDirectory $rootDir -RedirectStandardOutput $webLog -RedirectStandardError $webErrLog -PassThru
  Set-Content -Path $webPidFile -Value $webProc.Id -NoNewline

  Start-Sleep -Milliseconds 900
  Assert-ProcessAlive -Process $serviceProc -Name "service" -StdoutLog $serviceLog -StderrLog $serviceErrLog
  Assert-ProcessAlive -Process $webProc -Name "web" -StdoutLog $webLog -StderrLog $webErrLog

  Write-Host "[x-photo] Ready"
  Write-Host "  - Web: http://$webHost`:$webPort"
  Write-Host "  - API: http://$serviceHost`:$servicePort/rpc/v1"
  Write-Host "[x-photo] Logs: $serviceLog, $serviceErrLog, $webLog, $webErrLog"
  Write-Host "[x-photo] Pid files: $servicePidFile, $webPidFile"
  Write-Host "[x-photo] Press Ctrl+C to stop both services."

  Wait-Process -Id @($serviceProc.Id, $webProc.Id)
}
finally {
  if ($serviceProc -and -not $serviceProc.HasExited) {
    Stop-Process -Id $serviceProc.Id -Force
  }
  if ($webProc -and -not $webProc.HasExited) {
    Stop-Process -Id $webProc.Id -Force
  }
  if (Test-Path $servicePidFile) {
    Remove-Item $servicePidFile -Force
  }
  if (Test-Path $webPidFile) {
    Remove-Item $webPidFile -Force
  }
}
