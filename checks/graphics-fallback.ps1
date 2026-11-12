param([string]$App = 'target/release/pixelcraft.exe', [string]$Output = "$env:RUNNER_TEMP/pixelcraft-fallback")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Output | Out-Null
$exe = (Resolve-Path $App).Path
$env:PIXELCRAFT_HOME = "$Output/data"
$env:WGPU_BACKEND = 'metal' # Unsupported on Windows: exercise automatic OpenGL restart.
Remove-Item Env:PIXELCRAFT_OPENGL -ErrorAction SilentlyContinue
$process = Start-Process $exe -ArgumentList @('--screenshot', "`"$Output/fallback.png`"") -PassThru -RedirectStandardError "$Output/fallback.log"
if (!$process.WaitForExit(60000)) { $process.Kill(); throw 'Graphics fallback timed out' }
if ($process.ExitCode -ne 0) { throw 'Graphics fallback failed' }
if (!(Test-Path "$Output/fallback.png")) { throw 'Graphics fallback did not render' }
if (!(Select-String -Quiet -Path "$Output/fallback.log" -Pattern 'restarting with OpenGL')) { throw 'OpenGL fallback was not exercised' }
