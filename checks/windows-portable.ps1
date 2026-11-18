param([Parameter(Mandatory=$true)][string]$App)
$ErrorActionPreference = 'Stop'
$appPath = (Resolve-Path -LiteralPath $App).Path
$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$work = Join-Path $tempRoot ('pixelcraft-portable-' + [guid]::NewGuid())
$isolated = Join-Path $work 'Standalone App'
New-Item -ItemType Directory -Path $isolated | Out-Null
$copy = Join-Path $isolated 'PixelCraft.exe'
Copy-Item -LiteralPath $appPath -Destination $copy
$previousPath = $env:PATH
$previousHome = $env:PIXELCRAFT_HOME
try {
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $env:PIXELCRAFT_HOME = Join-Path $work 'User Data'
    $results = Join-Path $work 'Results'
    $stdout = Join-Path $work 'standalone.log'
    $stderr = Join-Path $work 'standalone-error.log'
    $process = Start-Process -FilePath $copy -ArgumentList @('--standalone-check', ('"{0}"' -f $results)) `
        -WorkingDirectory $isolated -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if (!$process.WaitForExit(120000)) {
        $process.Kill()
        throw 'Portable application timed out'
    }
    if ($process.ExitCode -ne 0) {
        Get-Content -LiteralPath $stderr
        throw "Portable application exited with $($process.ExitCode)"
    }
    foreach ($output in @('standalone.pixel', 'frame.png', 'sheet.png', 'animation.gif')) {
        $file = Get-Item -LiteralPath (Join-Path $results $output)
        if ($file.Length -eq 0) { throw "Empty portable smoke-test output: $output" }
    }
    if ((Get-ChildItem -LiteralPath $isolated -Force).Count -ne 1) {
        throw 'Portable application left files beside its executable'
    }
    Write-Host "Single-executable smoke test passed: $work"
} finally {
    $env:PATH = $previousPath
    $env:PIXELCRAFT_HOME = $previousHome
    Write-Host "Portable verification files: $work"
}
