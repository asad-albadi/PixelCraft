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
