use pixelcraft::extensions::Manager;
use serde_json::json;
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "pixelcraft-retirement-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn script_only_install_is_rejected_without_installing_or_executing_it() {
    let root = root();
    let manager = Manager::at(root.join("data")).unwrap();
    let script = root.join("unsupported.lua");
    fs::write(&script, "error('must never execute')").unwrap();
    let error = manager
        .install(&script)
        .expect_err("Lua is no longer an installable extension");
    assert!(error.contains("no longer supported"), "{error}");
    assert!(manager.list().unwrap().is_empty());
    assert!(script.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mixed_packages_keep_assets_without_resolving_or_executing_script_paths() {
    let root = root();
    let manager = Manager::at(root.join("data")).unwrap();
    let package = root.join("mixed");
    fs::create_dir(&package).unwrap();
    fs::write(package.join("palette.json"), "[[0,0,0,0],[80,90,100,255]]").unwrap();
    fs::write(package.join("package.json"), serde_json::to_vec(&json!({"name":"mixed", "contributes":{"palettes":[{"path":"palette.json"}],"scripts":[{"path":"../../must-not-resolve.lua"}]}})).unwrap()).unwrap();
    let id = manager
        .install(&package)
        .expect("independent native assets survive script retirement");
    let installed = manager.list().unwrap().remove(0);
    assert!(installed.enabled);
    assert_eq!(installed.manifest.contributes.palettes.len(), 1);
    assert_eq!(id, "mixed");
    assert!(installed.manifest.contributes.script_warning().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_script_only_package_cannot_be_enabled_and_keeps_user_files() {
    let root = root();
    let manager = Manager::at(root.join("data")).unwrap();
    let directory = manager.root.join("extensions/legacy");
    fs::create_dir(&directory).unwrap();
    let marker = root.join("executed");
    fs::write(
        directory.join("main.lua"),
        format!(
            "local f=io.open({:?},'w');f:write('executed');f:close()",
            marker.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        directory.join("package.json"),
        serde_json::to_vec(
            &json!({"name":"legacy","contributes":{"scripts":[{"path":"main.lua"}]}}),
        )
        .unwrap(),
    )
    .unwrap();
    let installed = manager.list().unwrap().remove(0);
    assert!(!installed.enabled, "legacy Lua packages must not be active");
    assert!(manager.set_enabled("legacy", true).is_err());
    assert!(installed.manifest.contributes.script_warning().is_some());
    assert!(!marker.exists());
    assert!(directory.join("main.lua").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_asset_packages_install_disable_reenable_and_uninstall_independently() {
    let root = root();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("colors.json"), "[[255,0,0,255]]").unwrap();
    fs::write(
        source.join("package.json"),
        serde_json::to_vec(
            &json!({"name":"assets","contributes":{"palettes":[{"path":"colors.json"}]}}),
        )
        .unwrap(),
    )
    .unwrap();
    let manager = Manager::at(root.join("data")).unwrap();
    let id = manager.install(&source).unwrap();
    let contribution = manager.list().unwrap()[0].manifest.contributes.palettes[0].clone();
    assert!(manager.asset(&id, &contribution).unwrap().is_file());
    assert_eq!(
        manager.load_palette_asset(&id, &contribution).unwrap(),
        vec![[255, 0, 0, 255]]
    );
    manager.set_enabled(&id, false).unwrap();
    assert!(!manager.list().unwrap()[0].enabled);
    assert!(manager.asset(&id, &contribution).is_err());
    manager.set_enabled(&id, true).unwrap();
    assert!(manager.list().unwrap()[0].enabled);
    assert!(manager.asset(&id, &contribution).unwrap().is_file());
    manager.uninstall(&id).unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(source.join("colors.json").is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn supported_asset_paths_cannot_escape_the_package_on_install() {
    let root = root();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(root.join("outside.json"), "[[255,0,0,255]]").unwrap();
    fs::write(
        source.join("package.json"),
        serde_json::to_vec(
            &json!({"name":"escape","contributes":{"palettes":[{"path":"../outside.json"}]}}),
        )
        .unwrap(),
    )
    .unwrap();
    let manager = Manager::at(root.join("data")).unwrap();
    assert!(manager.install(&source).is_err());
    assert!(manager.list().unwrap().is_empty());
    assert_eq!(
        fs::read(root.join("outside.json")).unwrap(),
        b"[[255,0,0,255]]"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extension_palette_decoder_rejects_sidecars_outside_its_package() {
    let root = root();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("package.json"),
        serde_json::to_vec(
            &json!({"name":"contained","contributes":{"palettes":[{"path":"terrain.tsj"}]}}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(source.join("terrain.tsj"),serde_json::to_vec(&json!({"type":"tileset","name":"set","tilewidth":1,"tileheight":1,"tilecount":1,"columns":1,"image":"../outside.png"})).unwrap()).unwrap();
    let manager = Manager::at(root.join("data")).unwrap();
    let id = manager.install(&source).unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba([30, 60, 90, 255]))
        .save(manager.root.join("extensions/outside.png"))
        .unwrap();
    let contribution = manager.list().unwrap()[0].manifest.contributes.palettes[0].clone();
    let error = manager
        .load_palette_asset(&id, &contribution)
        .expect_err("palette dependencies must remain in the installed package");
    assert!(error.contains("package"), "{error}");
    // A legitimate sidecar under the installed package remains usable.
    let package = manager.root.join("extensions").join(&id);
    fs::write(package.join("terrain.tsj"),serde_json::to_vec(&json!({"type":"tileset","name":"set","tilewidth":1,"tileheight":1,"tilecount":1,"columns":1,"image":"inside.png"})).unwrap()).unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba([30, 60, 90, 255]))
        .save(package.join("inside.png"))
        .unwrap();
    assert!(
        !manager
            .load_palette_asset(&id, &contribution)
            .unwrap()
            .is_empty()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_archive_is_rejected_before_zip_decoding() {
    let root = root();
    let archive = root.join("oversized.zip");
    let file = fs::File::create(&archive).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let manager = Manager::at(root.join("data")).unwrap();
    let error = manager.install(&archive).unwrap_err();
    assert!(
        error.contains("archive exceeds"),
        "archive size must be checked before decoding: {error}"
    );
    assert!(manager.list().unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}
