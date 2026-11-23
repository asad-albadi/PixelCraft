# Android production identity and signing

Publisher: **AstralHive**. Production application ID: **`om.astralhive.pixelcraft`**.
Debug builds use **`om.astralhive.pixelcraft.debug`** and a debug certificate; they
can coexist with the production app. Older `.preview` installations remain
separate. Export projects before moving between these application IDs.

## Signing material and backups

The permanent RSA-4096 app signing key was generated outside this repository.
No keystore or password belongs in Git, issue/PR attachments, APK assets or build
artifacts. `release-certificate.sha256` contains only the **public certificate
fingerprint** used to verify that builds have the intended signing identity.

The owner must securely back up both the `.jks` file and its private credentials
JSON, preferably in encrypted storage, and retain the public certificate and
fingerprint. GitHub Secrets is not a downloadable key backup. Keep using the
same signing key for subsequent releases; do not regenerate it for each build.

Local builds read a private JSON file outside the repo:

```sh
python3 checks/build-android-release.py \
  --signing-config /absolute/private/path/signing-credentials.json \
  --output /absolute/path/to/public-build-output
```

The JSON contains `keystore` (relative to that JSON, or absolute), `key_alias`,
`store_password` and `key_password`. It and the keystore must be readable only
by the owner on POSIX systems (`chmod 600`). Do not paste their values into a
shell command or documentation. The helper passes credentials through the
build process environment and does not print them.

Without signing credentials, explicit release tasks fail. Release builds do
not use the debug certificate. Debug PR builds need no production secrets.

## GitHub Actions

Four encrypted repository secrets supply production signing:

- `PIXELCRAFT_ANDROID_KEYSTORE_BASE64`
- `PIXELCRAFT_ANDROID_STORE_PASSWORD`
- `PIXELCRAFT_ANDROID_KEY_ALIAS`
- `PIXELCRAFT_ANDROID_KEY_PASSWORD`

`android-release.yml` only signs trusted main/tag builds. It recreates the key
with private permissions in runner temporary storage, builds an optimized
universal APK and Play App Bundle, verifies signatures against the public
fingerprint, then removes the temporary key. Only APK/AAB files, checksums and
the public certificate fingerprint enter the artifact.

Tagged desktop releases call this workflow and publish its downloads alongside
Linux, Windows and macOS packages. The separate Android PR test workflow still
produces debug APKs without signing secrets.

Expected downloads for v0.12.0:

- `PixelCraft-v0.12.0-Android.apk`
- `PixelCraft-v0.12.0-Android.aab`
- Their `.sha256` files and public certificate fingerprint.

Bump the mobile `version` and increment its `+versionCode` for every new release.
The tag must match the mobile version, and the code must exceed the previous
Android release. The helper passes both values explicitly to Flutter.

The helper also checks the permanent application ID, non-debuggable APK,
absence of Internet permission, both 64-bit ABIs and 16 KiB native/ZIP alignment.

## Google Play enrollment

To allow the same installed app to update across GitHub APKs and Google Play,
**enroll in Play App Signing using this existing app signing key**. Choosing an
unrelated Google-generated app signing key creates a different APK signing
identity. Follow Play Console's PEPK instructions when it asks for an encrypted
key export; do not upload the original keystore anywhere public. A separate
Play upload key can be registered later; it is not the app signing key used
on installed APKs.

[Android app signing](https://developer.android.com/studio/publish/app-signing)
· [Flutter Android release guide](https://docs.flutter.dev/deployment/android)
