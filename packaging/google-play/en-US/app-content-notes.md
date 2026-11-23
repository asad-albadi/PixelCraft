# Play Console app-content preparation

Prepared from the current Android source; these are review notes, not submitted declarations.

## Confirmed for the current Android implementation

- Free and open source (MIT); pricing confirmed by the owner.
- No in-app advertising, billing, PixelCraft account or authentication.
- No analytics/crash collection SDK in the Flutter dependency list.
- Drawing, document import, recovery and export use local Rust/Android workflows.
- Main Android manifest has no Internet permission. Debug/profile manifests add Internet permission for development; verify the final release merged manifest.
- Android's picker may expose user-selected cloud providers. Sharing is explicit and grants a receiving app read access to an export.
- The application uses Android backup defaults. Do not claim that Android system backups are disabled.

## Data safety draft

The current implementation appears to support **no data collected by the app** and **no data shared by the developer** for the in-app editing workflow. Confirm this against the signed release's merged manifest, dependency inventory and actual behavior before submitting the form. Do not infer privacy solely from a missing permission. User-directed third-party sharing/provider actions and external support services must be considered under Google's definitions.

No account is created, so an in-app account-deletion workflow is not applicable to the current implementation. Artwork can be deleted locally; exported copies and third-party backups are controlled separately.

## Owner decisions / console work remaining

- Confirm the developer identity exactly as displayed by the existing Play account.
- Choose target audience age groups; this package does not select ages or claim Families compliance.
- Complete the IARC content-rating questionnaire honestly. The included garden artwork is nonviolent; the app is a general drawing tool, not a hosted user-content service. The rating is assigned by the questionnaire, not this draft.
- Publish the reviewed policy by merging `PRIVACY.md` onto main, then enter its public GitHub URL in Play Console. The app menu opens that URL through Android's browser; no copy is bundled and no Internet permission is added to the app.
- Use `om.astralhive.pixelcraft` under AstralHive. Production signing is configured; enroll in Play App Signing using the existing key if GitHub/Play updates must share the same signing identity.
- Build/sign the release AAB; verify versionCode, target API, native-library alignment and release behavior. Current screenshots depict the real preview app's UI, not a signed production build.
- Recheck declarations whenever dependencies, permissions, network services or monetization change.

## Official guidance

- https://support.google.com/googleplay/android-developer/answer/10787469
- https://support.google.com/googleplay/android-developer/answer/10144311
- https://support.google.com/googleplay/android-developer/answer/9866151
