use pixelcraft::updates::Release;

#[test]
fn update_notice_requires_a_newer_version_not_just_a_different_tag() {
    for (tag, installed, newer) in [
        ("v0.3.0", "0.2.0", true),
        ("v0.2.0", "0.2.0", false),
        ("0.2.0+build.2", "0.2.0", false),
        ("v0.1.9", "0.2.0", false),
        ("v0.10.0", "0.9.0", true),
        ("v0.3.0", "0.3.0-rc.1", true),
    ] {
        let release = Release::parse(&format!(
            r#"{{"tag_name":"{tag}","draft":false,"prerelease":false}}"#
        ))
        .unwrap();
        assert_eq!(
            release.is_newer_than(installed),
            newer,
            "{tag} vs {installed}"
        );
    }
}

#[test]
fn invalid_and_unpublished_releases_do_not_trigger_updates() {
    for body in [
        r#"{"message":"Not Found"}"#,
        r#"{"tag_name":"nightly"}"#,
        r#"{"tag_name":"v0.4.0","draft":true}"#,
        r#"{"tag_name":"v0.4.0-rc.1","prerelease":true}"#,
        "not json",
    ] {
        assert!(Release::parse(body).is_err(), "{body}");
    }
}

#[test]
fn repository_links_use_only_valid_github_repositories() {
    use pixelcraft::updates::{AboutInfo, repository_slug};
    assert_eq!(
        repository_slug(" https://github.com/example/pixelcraft.git/ "),
        Some("example/pixelcraft".into())
    );
    for invalid in [
        "",
        "example",
        "https://example.com/owner/repo",
        "owner/repo/releases",
        "../repo",
        "owner/repo?x=1",
    ] {
        assert!(repository_slug(invalid).is_none(), "{invalid}");
    }
    let info = AboutInfo {
        developer: "Example developer".into(),
        repository: "example/pixelcraft".into(),
    };
    assert_eq!(
        info.github_url().as_deref(),
        Some("https://github.com/example/pixelcraft")
    );
    assert_eq!(
        info.releases_url().as_deref(),
        Some("https://github.com/example/pixelcraft/releases/latest")
    );
}

#[test]
fn saved_blank_about_fields_keep_the_build_identity() {
    use pixelcraft::updates::AboutInfo;
    let info: AboutInfo = serde_json::from_str(r#"{"developer":"","repository":""}"#).unwrap();
    assert!(!info.developer.trim().is_empty());
}

#[test]
fn saved_about_identity_cannot_override_build_credits_or_release_destination() {
    use pixelcraft::updates::AboutInfo;
    let info: AboutInfo = serde_json::from_str(
        r#"{"developer":"Previous custom author","repository":"https://github.com/previous/custom-repo.git"}"#,
    ).unwrap();
    assert_eq!(
        info.developer,
        option_env!("PIXELCRAFT_DEVELOPER").unwrap_or(env!("CARGO_PKG_AUTHORS"))
    );
    assert_eq!(
        info.repository,
        option_env!("PIXELCRAFT_REPOSITORY").unwrap_or("")
    );
    let build = AboutInfo::default();
    assert_eq!(info.github_url(), build.github_url());
    assert_eq!(info.releases_url(), build.releases_url());
}
