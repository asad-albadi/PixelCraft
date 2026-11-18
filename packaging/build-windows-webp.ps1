# Run in an x64 MSVC developer shell before cargo test/build on Windows.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$build = Join-Path ([IO.Path]::GetTempPath()) ('pixelcraft-webp-build-' + [guid]::NewGuid())
$runtime = Join-Path $root 'runtime/webp'
New-Item -ItemType Directory $build | Out-Null
try {
    $archive = Join-Path $build 'webp.tar.gz'
    # Official release; checksum also recorded by Buildroot:
    # https://lists.buildroot.org/pipermail/buildroot/2026-January/794888.html
    Invoke-WebRequest 'https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0.tar.gz' -OutFile $archive
    if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne 'e4ab7009bf0629fd11982d4c2aa83964cf244cffba7347ecd39019a9e38c4564') {
        throw 'WebP source checksum mismatch'
    }
    tar -xzf $archive -C $build
    if ($LASTEXITCODE -ne 0) { throw 'WebP source extraction failed' }
    $source = Join-Path $build 'libwebp-1.6.0'
    $binary = Join-Path $build 'compiled'
    # Build static libraries with /MT for the single-file executable.
    $options = @('-DBUILD_SHARED_LIBS=OFF', '-DWEBP_LINK_STATIC=ON',
        '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded')
    foreach ($feature in @('ANIM_UTILS', 'CWEBP', 'DWEBP', 'GIF2WEBP', 'IMG2WEBP', 'VWEBP', 'WEBPINFO', 'LIBWEBPMUX', 'WEBPMUX', 'EXTRAS')) {
        $options += "-DWEBP_BUILD_${feature}=OFF"
    }
    & cmake -S $source -B $binary -G 'Visual Studio 17 2022' -A x64 @options
    if ($LASTEXITCODE -ne 0) { throw 'WebP configuration failed' }
    & cmake --build $binary --config Release --target webpdemux --parallel
    if ($LASTEXITCODE -ne 0) { throw 'WebP compilation failed' }
    New-Item -ItemType Directory -Force $runtime | Out-Null
    foreach ($name in @('webpdemux', 'webp', 'sharpyuv')) {
        $lib = Get-ChildItem -LiteralPath $binary -Recurse -File -Filter '*.lib' |
            Where-Object { $_.BaseName -in @($name, "lib$name") -and $_.Directory.Name -eq 'Release' } |
            Select-Object -First 1
        if (!$lib) { throw "Missing static WebP library: $name" }
        Copy-Item -LiteralPath $lib.FullName -Destination (Join-Path $runtime "$name.lib")
    }
    foreach ($notice in @('COPYING', 'PATENTS', 'AUTHORS')) {
        Copy-Item (Join-Path $source $notice) (Join-Path $runtime "WEBP-$notice.txt")
    }
} finally {
    $resolved = (Resolve-Path -LiteralPath $build).Path
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (!$resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected build directory: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
