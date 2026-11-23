# PixelCraft — Google Play listing pack

English (United States), prepared November 23, 2026. **Local preparation only; nothing has been submitted to Play Console.**

## Listing text

Copy each file into its matching Play Console field:

| File under `en-US/` | Field / purpose |
| --- | --- |
| `title.txt` | App name — 29/30 characters |
| `short-description.txt` | Short description — 78/80 characters |
| `full-description.txt` | Full description — 2,408/4,000 characters |
| `release-notes.txt` | What's new — 445/500 characters |
| `contact-details.json` | Support email, website, free pricing, category suggestion and proposed app ID |
| `reviewer-instructions.txt` | Optional review instructions; no login required |
| `asset-alt-text.json` | Accessible descriptions keyed by image path |
| `privacy-policy-draft.md` | Review and host before using as the app's privacy policy |
| `app-content-notes.md` | Source-based Data safety notes and remaining owner declarations |

The support contact is **asad.badi@astralhive.om**. Pricing is **free**. The Android implementation has no ads or in-app billing. Suggested category: **Art & Design**; choose applicable tags from the options actually offered in Console.

## Graphics to upload

| Console field | Files | Size / encoding |
| --- | --- | --- |
| App icon | `en-US/graphics/icon-512.png` | 512×512, RGBA PNG, square background without a baked-in outer rounded mask |
| Feature graphic | `en-US/graphics/feature-graphic-1024x500.png` | 1024×500, RGB PNG, opaque as required by Play |
| Phone screenshots | `en-US/graphics/phone-screenshots/` | Four real 1080×1920 captures, RGB PNG |
| 7-inch tablet screenshots | `en-US/graphics/seven-inch-screenshots/` | Four real 1920×1080 captures at a 7-inch-class virtual display configuration, RGB PNG |
| 10-inch tablet screenshots | `en-US/graphics/ten-inch-screenshots/` | Four real 2560×1440 captures at a 10-inch-class virtual display configuration, RGB PNG |

Upload screenshot files in filename order. `contact-sheet.png` is for reviewing the whole set, **not** a store screenshot. Screenshots contain only real mobile UI and built-in sample artwork, with no desktop screens, promotional overlays or device frames. They include light and dark appearance, color controls, animation/frame tools, and Zen drawing mode.

Screenshots were taken in API 35 Android emulators. Tablet size classes use actual Android display-size/density overrides, rather than stretching or cropping captures; they are not claims of separate physical 7-inch/10-inch hardware tests. Phone: 1080×1920, 420 dpi. Tablet: 1920×1080, 315 dpi and 2560×1440, 280 dpi. Android's System UI demo mode sets a generic clock/battery and hides notifications. The capture build reports versionName 0.11.0 / versionCode 1 and the preview application ID; the proposed v0.12.0 version/signing work remains uncommitted. Recapture if the final release UI changes.

The icon uses PixelCraft's existing pixel P emblem. The feature graphic uses that same brand palette and the garden exported through the shared Rust engine. No AI-generated UI or artwork is used.

## Before submission

1. Settle the permanent application ID and release signing, and build the signed production AAB. Store copy does not change the current preview package identity.
2. Review and host the privacy policy at a stable public URL, then enter its URL in Play Console. The app menu now includes an offline Privacy policy view. The draft file is not a published URL.
3. Confirm developer/account details, audience ages, IARC rating and Data safety answers in Console. These forms have not been submitted or preselected here.
4. Review the final release's dependencies and merged permissions before making privacy declarations.
5. Run release-build checks and confirm that these screenshots still match the signed build.

A preview video is optional for this art editor and is not included. No TV, Wear OS, automotive or foldable-specific listing assets are claimed.

## Regeneration and verification

- `python3 source/export-sample.py /absolute/path/to/libpixelcraft_ffi.so` exports the actual built-in garden from a current native FFI build.
- `python3 source/build-graphics.py` renders the icon and feature graphic using `rsvg-convert` and Pillow for PNG encoding. Noto Sans is used for the branding layout.
- `python3 source/validate-listing.py` checks text limits, dimensions, PNG encoding, file sizes, screenshot counts and alt-text coverage.
- `asset-manifest.json` records image dimensions, byte sizes and SHA-256 hashes.

Official references, checked November 23, 2026:

- [Store graphics and screenshot requirements](https://support.google.com/googleplay/android-developer/answer/9866151)
- [Data safety declarations](https://support.google.com/googleplay/android-developer/answer/10787469)
- [Privacy policy / User Data requirements](https://support.google.com/googleplay/android-developer/answer/10144311)
- [Play icon design](https://developer.android.com/distribute/google-play/resources/icon-design-specifications)
