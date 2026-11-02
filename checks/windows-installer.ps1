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
    foreach ($notice in @('LICENSE', 'LUA-LICENSE.txt')) {
        if (!(Test-Path (Join-Path $installDir $notice))) { throw "Missing license: $notice" }
    }
    # This exercises saves, exports, embedded Lua and extension lifecycle from the installed exe.
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
    Write-Host "Installer verification files: $work"
}
