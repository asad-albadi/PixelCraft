# Run in an x64 MSVC developer shell before cargo build/test on Windows.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$build = Join-Path ([IO.Path]::GetTempPath()) ('pixelcraft-lua-build-' + [guid]::NewGuid())
New-Item -ItemType Directory $build | Out-Null
try {
    $archive = Join-Path $build 'lua.tar.gz'
    # Official source and SHA-256: https://www.lua.org/ftp/
    Invoke-WebRequest 'https://www.lua.org/ftp/lua-5.4.9.tar.gz' -OutFile $archive
    if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne '2335b6c582a52654f94612bf10d2f4672805d05329aa6568b1d8cd9e5c6fb8e6') {
        throw 'Lua source checksum mismatch'
    }
    tar -xzf $archive -C $build
    if ($LASTEXITCODE -ne 0) { throw 'Lua source extraction failed' }
    Push-Location (Join-Path $build 'lua-5.4.9/src')
    try {
        $sources = @(Get-ChildItem *.c | Where-Object Name -NotIn @('lua.c', 'luac.c') | ForEach-Object Name)
        # /MT embeds the CRT; LUA_BUILD_AS_DLL exports the Lua C API.
        & cl.exe /nologo /O2 /MT /LD /DLUA_BUILD_AS_DLL @sources /link /OUT:lua54.dll
        if ($LASTEXITCODE -ne 0) { throw 'Windows Lua compilation failed' }
        Copy-Item lua54.dll (Join-Path $root 'runtime/lua54.dll')
    } finally { Pop-Location }
} finally { Remove-Item -Recurse -Force $build }
