param([string]$App = 'target/release/pixelcraft.exe', [string]$Output = "$env:RUNNER_TEMP/pixelcraft-fallback")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Output | Out-Null
$exe = (Resolve-Path $App).Path
$oldHome = $env:PIXELCRAFT_HOME
$oldBackend = $env:WGPU_BACKEND
$oldOpenGL = $env:PIXELCRAFT_OPENGL
$oldGallium = $env:GALLIUM_DRIVER
function Invoke-Startup([string]$Program, [string]$Name) {
    $shot = Join-Path $Output "$Name.png"
    $log = Join-Path $Output "$Name.log"
    Remove-Item $shot -ErrorAction SilentlyContinue
    $process = Start-Process $Program -ArgumentList @('--screenshot', "`"$shot`"") -PassThru -RedirectStandardError $log
    $null = $process.Handle
    if (!$process.WaitForExit(60000)) {
        $process.Kill($true)
        Get-Content $log -ErrorAction SilentlyContinue | Write-Host
        throw "$Name startup timed out"
    }
    $process.WaitForExit()
    Get-Content $log -ErrorAction SilentlyContinue | Write-Host
    if ($process.ExitCode -ne 0) { throw "$Name startup failed with exit code $($process.ExitCode)" }
    if (!(Test-Path $shot)) { throw "$Name startup did not render" }
}
try {
    $env:PIXELCRAFT_HOME = "$Output/data"
    Remove-Item Env:WGPU_BACKEND -ErrorAction SilentlyContinue
    Remove-Item Env:PIXELCRAFT_OPENGL -ErrorAction SilentlyContinue
    Invoke-Startup $exe 'normal'

    # Hosted Windows runners lack modern hardware OpenGL. Supply a pinned,
    # checksummed software driver only beside an isolated test copy, never dist/.
    $archive = Join-Path $Output 'mesa.7z'
    Invoke-WebRequest 'https://github.com/pal1000/mesa-dist-win/releases/download/25.3.3/mesa3d-25.3.3-release-msvc.7z' -OutFile $archive
    if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne '66b79057ba273c08daa03551a231995d48b8623bb1efa9f3a7c3e09792d8f1ba') { throw 'Mesa checksum mismatch' }
    $mesa = Join-Path $Output 'mesa'
    & 7z x $archive "-o$mesa" -y | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Mesa extraction failed' }
    $test = Join-Path $Output 'software-gl'
    New-Item -ItemType Directory -Force $test | Out-Null
    Copy-Item $exe (Join-Path $test 'pixelcraft.exe')
    Copy-Item "$mesa/x64/*.dll" $test
    $env:GALLIUM_DRIVER = 'llvmpipe'
    $env:WGPU_BACKEND = 'metal' # Unsupported on Windows: force the automatic restart.
    Invoke-Startup (Join-Path $test 'pixelcraft.exe') 'fallback'
    if (!(Select-String -Quiet -Path "$Output/fallback.log" -Pattern 'restarting with OpenGL')) { throw 'OpenGL fallback was not exercised' }
} finally {
    $env:PIXELCRAFT_HOME = $oldHome
    $env:WGPU_BACKEND = $oldBackend
    $env:PIXELCRAFT_OPENGL = $oldOpenGL
    $env:GALLIUM_DRIVER = $oldGallium
}
