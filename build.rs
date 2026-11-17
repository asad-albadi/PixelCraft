use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/pixelcraft.ico");
    println!("cargo:rerun-if-changed=packaging/pixelcraft.rc");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if !matches!(os.as_str(), "windows" | "macos") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if os == "macos" {
        let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64",
            Ok("x86_64") => "x86_64",
            _ => panic!("PixelCraft supports Apple Silicon and Intel macOS targets"),
        };
        let runtime = root.join("runtime/macos").join(arch);
        let lua = runtime.join("liblua5.4.dylib");
        println!("cargo:rerun-if-changed={}", lua.display());
        assert!(
            lua.is_file(),
            "Run packaging/build-macos-runtime.sh {arch} before building PixelCraft"
        );
        println!("cargo:rustc-env=PIXELCRAFT_MACOS_LUA={}", lua.display());
        println!("cargo:rustc-link-search=native={}", runtime.display());
        for library in ["webpdemux", "webp", "sharpyuv"] {
            let archive = runtime.join(format!("lib{library}.a"));
            println!("cargo:rerun-if-changed={}", archive.display());
            assert!(
                archive.is_file(),
                "Run packaging/build-macos-runtime.sh {arch} before building PixelCraft"
            );
            println!("cargo:rustc-link-lib=static={library}");
        }
        return;
    }

    let webp = root.join("runtime/webp");
    println!("cargo:rustc-link-search=native={}", webp.display());
    for library in ["webpdemux", "webp", "sharpyuv"] {
        let archive = webp.join(format!("{library}.lib"));
        println!("cargo:rerun-if-changed={}", archive.display());
        assert!(
            archive.is_file(),
            "Run packaging/build-windows-webp.ps1 before building PixelCraft"
        );
        println!("cargo:rustc-link-lib=static={library}");
    }
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
