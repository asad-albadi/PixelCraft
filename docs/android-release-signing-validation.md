# AstralHive Android release signing — 2026-11-23

## Identity and key handling

- Production: `om.astralhive.pixelcraft`; debug: `om.astralhive.pixelcraft.debug`.
- Permanent RSA-4096 key, 10,000-day certificate validity, generated outside Git.
- Key directory is owner-only; keystore and credential JSON are mode 600.
- Four encrypted repository signing secrets were configured; no secret values are documented here.
- Only the public certificate SHA-256 fingerprint is tracked in `packaging/android/release-certificate.sha256`.
- Signed workflow is restricted to main/tag builds; PR test APKs do not receive production signing secrets.

## Executed verification

- Direct Gradle release build without credentials failed with the required-signing message; no debug fallback.
- Signed universal APK and AAB built successfully with versionName `0.12.0`, versionCode `2`.
- APK signature and AAB signature match the permanent public certificate fingerprint.
- APK application ID, non-debuggable flag and absence of Internet permission verified.
- Six native libraries cover ARM64/x86_64 with at least 16 KiB ELF LOAD and ZIP alignment.
- Production APK installed and launched in the phone emulator; the garden/editor rendered and reported ready.
- Debug build succeeds without production credentials and has the separate `.debug` application ID.
- Eight release-helper tests passed, including missing/private credential handling, version/tag matching, increasing versionCode, and a real synthetic signed bundle with appended unsigned payload rejected.
- Flutter analysis clean; all 41 Flutter tests passed.
- Rust workspace: 843 passed, 16 existing opt-in ignores. Workspace boundaries and Rust formatting passed.
- Existing ExportPolicy standalone JVM main passed after the package migration. It is a standalone harness, not a JUnit discovery suite.
- Workflow actionlint passed. Independent read-only code/security review found no remaining actionable findings after fixes.
- Scanned tracked/unignored repository files and both produced archives for the actual signing passwords/keystore content; no signing secrets were found. Private materials are not build artifacts.

## Release build details

Flutter release builds must regenerate production plugin registration. Keeping
`--no-pub` after a debug/integration-test build can leave integration_test in the
Java registrant ([upstream issue](https://github.com/flutter/flutter/issues/169336)).
The helper allows that regeneration and then rejects any dependency-lock changes.

The helper checks both version metadata fields and verifies tag/pubspec agreement.
Complete release history is fetched so a new version must increment Android's
versionCode above the previous mobile release. Self-signed Android certificates
produce the expected strict jarsigner trust warning; unsigned payload entries
and other severe signature errors are rejected. The expected certificate is
checked separately, rather than accepting any self-signed signer.

## Not performed

No merge, release tag, public release upload, Play Console submission, organization
verification or Play App Signing enrollment was performed for this setup. The
first tagged GitHub signing run remains to be exercised after merge. Desktop
platform checks continue to run through the ordinary PR/release workflows.
