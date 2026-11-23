#!/usr/bin/env python3
"""Build and verify signed Android APK/AAB without printing signing credentials."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import ssl
import subprocess

ROOT = Path(__file__).resolve().parent.parent
MOBILE = ROOT / 'flutter/pixelcraft_mobile'
SIGNING_VARIABLES = (
    'PIXELCRAFT_ANDROID_KEYSTORE', 'PIXELCRAFT_ANDROID_STORE_PASSWORD',
    'PIXELCRAFT_ANDROID_KEY_ALIAS', 'PIXELCRAFT_ANDROID_KEY_PASSWORD',
)
APPLICATION_ID = 'om.astralhive.pixelcraft'


def require_private(path, label):
    if not path.is_file():
        raise ValueError(f'{label} file is missing')
    if os.name == 'posix' and path.stat().st_mode & 0o077:
        raise ValueError(f'{label} must be private (chmod 600)')


def signing_environment(config_path, environ):
    env = dict(environ)
    if config_path is not None:
        path = Path(config_path).resolve()
        require_private(path, 'Signing credentials')
        try:
            config = json.loads(path.read_text())
            key = (path.parent / config['keystore']).resolve()
            values = (str(key), config['store_password'], config['key_alias'], config['key_password'])
        except (OSError, ValueError, KeyError, TypeError):
            raise ValueError('Invalid signing credentials file') from None
        env.update(zip(SIGNING_VARIABLES, values))
    if not all(isinstance(env.get(name), str) and env[name].strip() for name in SIGNING_VARIABLES):
        raise ValueError('Release signing is required; configure all PIXELCRAFT_ANDROID signing variables')
    key = Path(env[SIGNING_VARIABLES[0]]).resolve()
    require_private(key, 'Signing keystore')
    env[SIGNING_VARIABLES[0]] = str(key)
    return env


def artifact_prefix(version):
    if not re.fullmatch(r'v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?', version):
        raise ValueError('Expected a release version such as v0.12.0')
    return f'PixelCraft-{version}-Android'


def release_identity(pubspec_version, requested):
    match = re.fullmatch(r'(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)\+(\d+)', pubspec_version)
    if not match or not 1 <= int(match[2]) <= 2100000000:
        raise ValueError('Mobile pubspec needs a release version and positive Android versionCode')
    version, build_name, build_number = 'v' + match[1], match[1], int(match[2])
    if requested is not None and requested != version:
        raise ValueError('Release tag does not match the mobile pubspec version')
    return version, build_name, build_number


def check_version_progression(build_name, build_number, previous):
    if previous is None:
        return
    _, previous_name, previous_code = release_identity(previous, None)
    if build_number < previous_code or (build_name != previous_name and build_number <= previous_code):
        raise ValueError('Increment the Android versionCode above the previous release before publishing a new version')


def previous_release_version():
    # The release workflow fetches complete history. Older desktop-only tags
    # legitimately have no mobile pubspec and provide no Android version baseline.
    tag = subprocess.run(['git', 'describe', '--tags', '--match', 'v[0-9]*', '--abbrev=0', 'HEAD^'], cwd=ROOT, capture_output=True, text=True)
    if tag.returncode:
        return None
    previous = subprocess.run(['git', 'show', f'{tag.stdout.strip()}:flutter/pixelcraft_mobile/pubspec.yaml'], cwd=ROOT, capture_output=True, text=True)
    if previous.returncode:
        return None
    match = re.search(r'^version:\s*(\S+)', previous.stdout, re.M)
    if not match:
        raise ValueError('Previous mobile release is missing version metadata')
    return match[1]


def verify_bundle_signature(bundle, env):
    java = Path(env['JAVA_HOME']) / 'bin' if env.get('JAVA_HOME') else Path('')
    signed = subprocess.run([str(java / 'jarsigner'), '-J-Duser.language=en', '-strict', '-verify', str(bundle)], env=env, capture_output=True, text=True)
    # Bit 4 covers the normal untrusted self-signed Android certificate; bit 16
    # (and every other severe-warning bit) must fail, including unsigned payload.
    if signed.returncode not in (0, 4) or 'jar verified' not in signed.stdout:
        raise ValueError('App Bundle contains unsigned/invalid content or failed signature verification')


def run(command, env, **kwargs):
    return subprocess.run(command, env=env, check=True, **kwargs)


def verify(apk, bundle, env, build_name, build_number):
    expected = (ROOT / 'packaging/android/release-certificate.sha256').read_text().strip()
    sdk = Path(env.get('ANDROID_HOME', Path.home() / 'Android/Sdk'))
    tools = sdk / 'build-tools/36.0.0'
    run(['python3', str(ROOT / 'checks/android-ci-apk.py'), str(apk)], env)
    run([str(tools / 'zipalign'), '-c', '-P', '16', '4', str(apk)], env)
    certs = run([str(tools / 'apksigner'), 'verify', '--verbose', '--print-certs', str(apk)], env, capture_output=True, text=True).stdout
    fingerprints = re.findall(r'Signer #\d+ certificate SHA-256 digest: ([0-9a-fA-F]+)', certs)
    if fingerprints != [expected]:
        raise ValueError('APK signing certificate does not match the permanent release identity')
    badging = run([str(tools / 'aapt'), 'dump', 'badging', str(apk)], env, capture_output=True, text=True).stdout
    if f"package: name='{APPLICATION_ID}'" not in badging or 'application-debuggable' in badging:
        raise ValueError('APK has an unexpected application ID or is debuggable')
    if f"versionName='{build_name}'" not in badging or f"versionCode='{build_number}'" not in badging:
        raise ValueError('APK version metadata does not match the intended release')
    permissions = run([str(tools / 'aapt'), 'dump', 'permissions', str(apk)], env, capture_output=True, text=True).stdout
    if 'android.permission.INTERNET' in permissions:
        raise ValueError('Production APK unexpectedly requests Internet permission')
    java = Path(env['JAVA_HOME']) / 'bin' if env.get('JAVA_HOME') else Path('')
    verify_bundle_signature(bundle, env)
    cert = run([str(java / 'keytool'), '-printcert', '-rfc', '-jarfile', str(bundle)], env, capture_output=True, text=True).stdout
    certificates = re.findall(r'-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----', cert, re.S)
    if len(certificates) != 1 or hashlib.sha256(ssl.PEM_cert_to_DER_cert(certificates[0])).hexdigest() != expected:
        raise ValueError('App Bundle signing certificate does not match the permanent release identity')
    print('Verified permanent release signatures, production app ID, non-debuggable APK and no Internet permission.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--signing-config', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/android')
    parser.add_argument('--version')
    args = parser.parse_args()
    env = signing_environment(args.signing_config, os.environ)
    pubspec_version = re.search(r'^version:\s*(\S+)', (MOBILE / 'pubspec.yaml').read_text(), re.M)[1]
    version, build_name, build_number = release_identity(pubspec_version, args.version)
    check_version_progression(build_name, build_number, previous_release_version())
    prefix = artifact_prefix(version)
    lock_before = (MOBILE / 'pubspec.lock').read_bytes()
    for kind in ('apk', 'appbundle'):
        # Flutter must regenerate release-only plugin registration. --no-pub
        # can retain integration_test from a previous debug/test invocation.
        run(['flutter', 'build', kind, '--release', '--target-platform', 'android-arm64,android-x64', '--build-name', build_name, '--build-number', str(build_number)], env, cwd=MOBILE)
        if (MOBILE / 'pubspec.lock').read_bytes() != lock_before:
            raise ValueError('Flutter changed the locked dependencies during the release build')
    apk = MOBILE / 'build/app/outputs/flutter-apk/app-release.apk'
    bundle = MOBILE / 'build/app/outputs/bundle/release/app-release.aab'
    verify(apk, bundle, env, build_name, build_number)
    args.output.mkdir(parents=True, exist_ok=True)
    for source, extension in ((apk, 'apk'), (bundle, 'aab')):
        destination = args.output / f'{prefix}.{extension}'
        shutil.copyfile(source, destination)
        digest = hashlib.sha256(destination.read_bytes()).hexdigest()
        destination.with_suffix(destination.suffix + '.sha256').write_text(f'{digest}  {destination.name}\n')
    shutil.copyfile(ROOT / 'packaging/android/release-certificate.sha256', args.output / f'{prefix}-certificate.sha256')
    print('Published verified build files to:', args.output)


if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        raise SystemExit(str(error)) from None
