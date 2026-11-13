param([Parameter(Mandatory=$true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path $Installer).Path
$work = Join-Path $env:RUNNER_TEMP ('pixelcraft-installer-' + [guid]::NewGuid())
$installDir = Join-Path $work 'Installed App'
New-Item -ItemType Directory $work | Out-Null
function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    $process = Start-Process -FilePath $Program -ArgumentList $Arguments -PassThru
    if (!$process.WaitForExit(120000)) {
        $process.Kill()
        throw "Timed out: $Program"
    }
    if ($process.ExitCode -ne 0) { throw "$Program failed with exit code $($process.ExitCode)" }
}
$env:PIXELCRAFT_HOME = Join-Path $work 'User Data'
$previousPath = $env:PATH
$previousDirectory = Get-Location
try {
    Invoke-Checked $installerPath @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$installDir`"", "/LOG=`"$work/install.log`"")
    $app = Join-Path $installDir 'pixelcraft.exe'
    if (!(Test-Path $app)) { throw 'Installer did not create the executable' }
    # A console-subsystem executable opens a terminal beside the editor.
    $image = [IO.File]::ReadAllBytes($app)
    $pe = [BitConverter]::ToInt32($image, 0x3c)
    if ([BitConverter]::ToUInt32($image, $pe) -ne 0x4550) { throw 'Invalid PE signature' }
    $subsystem = [BitConverter]::ToUInt16($image, $pe + 24 + 68)
    if ($subsystem -ne 2) { throw "Expected Windows GUI subsystem (2), got $subsystem; this build would open a console" }
    foreach ($dll in @('libwebpdemux.dll', 'libwebp.dll', 'libsharpyuv.dll')) {
        if (!(Test-Path (Join-Path $installDir $dll))) { throw "Missing WebP runtime: $dll" }
    }
    foreach ($notice in @('LICENSE', 'FONT-LICENSE.txt', 'LUA-LICENSE.txt', 'WEBP-COPYING.txt', 'WEBP-PATENTS.txt', 'WEBP-AUTHORS.txt')) {
        if (!(Test-Path (Join-Path $installDir $notice))) { throw "Missing license: $notice" }
    }
    # Exclude build tools and their DLLs from PATH/current directory. WebP must
    # load from the installation, as it would on a clean Windows machine.
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    Set-Location $work
    # Exercises WebP import, saves, exports, embedded Lua and extension lifecycle.
    Invoke-Checked $app @('--standalone-check', "`"$work/results`"")
    # Repeat installation to exercise the upgrade path with the stable AppId.
    Invoke-Checked $installerPath @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/DIR=`"$installDir`"")
    Invoke-Checked $app @('--standalone-check', "`"$work/upgrade-results`"")
    $userMarker = Join-Path $env:PIXELCRAFT_HOME 'keep-on-uninstall.txt'
    New-Item -ItemType Directory -Force $env:PIXELCRAFT_HOME | Out-Null
    Set-Content $userMarker 'User data must survive uninstall'
    Invoke-Checked (Join-Path $installDir 'unins000.exe') @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART')
    if (Test-Path $app) { throw 'Uninstall left the executable behind' }
    if (!(Test-Path $userMarker)) { throw 'Uninstall removed user data' }
    $hash = (Get-FileHash $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($installerPath))" | Set-Content "$installerPath.sha256" -Encoding ascii
} finally {
    $env:PATH = $previousPath
    Set-Location $previousDirectory
    Write-Host "Installer verification files: $work"
}
