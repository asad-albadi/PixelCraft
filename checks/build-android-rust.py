#!/usr/bin/env python3
"""Build only the shared native editor for Android; no desktop dependencies."""
import argparse, os, pathlib, shutil, subprocess
ROOT = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--abi', choices=['all', 'x86_64', 'arm64-v8a'], default='all')
args = parser.parse_args()
sdk = pathlib.Path(os.environ.get('ANDROID_HOME', os.environ.get('ANDROID_SDK_ROOT', pathlib.Path.home() / 'Android/Sdk')))
ndk = pathlib.Path(os.environ.get('ANDROID_NDK_HOME', sdk / 'ndk/28.2.13676358'))
toolchain = ndk / 'toolchains/llvm/prebuilt/linux-x86_64/bin'
if not toolchain.is_dir():
    raise SystemExit('Set ANDROID_NDK_HOME to Android NDK 28.2.13676358 (Linux host).')
target_dir = ROOT / 'target/android-rust'
for abi, target, clang in [('x86_64', 'x86_64-linux-android', 'x86_64-linux-android24-clang'), ('arm64-v8a', 'aarch64-linux-android', 'aarch64-linux-android24-clang')]:
    if args.abi not in ('all', abi): continue
    env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir))
    env['CARGO_TARGET_' + target.upper().replace('-', '_') + '_LINKER'] = str(toolchain / clang)
    env['CARGO_TARGET_' + target.upper().replace('-', '_') + '_RUSTFLAGS'] = '-C link-arg=-Wl,-z,max-page-size=16384'
    subprocess.run(['cargo', '+1.95.0', 'build', '--locked', '--release', '-p', 'pixelcraft-ffi', '--target', target], cwd=ROOT, env=env, check=True)
    out = ROOT / 'flutter/pixelcraft_mobile/android/app/src/main/jniLibs' / abi
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target_dir / target / 'release/libpixelcraft_ffi.so', out / 'libpixelcraft_ffi.so')
    print('Staged native editor for', abi, flush=True)
