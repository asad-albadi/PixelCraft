use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/pixelcraft.ico");
    println!("cargo:rerun-if-changed=packaging/pixelcraft.rc");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let resource = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pixelcraft.res");
    let status = Command::new("rc.exe")
        .current_dir(root.join("packaging"))
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg("pixelcraft.rc")
        .status()
        .expect("Windows icon compilation requires rc.exe from an MSVC developer shell");
    assert!(status.success(), "Windows icon resource compilation failed");
    println!("cargo:rustc-link-arg-bins={}", resource.display());
}
