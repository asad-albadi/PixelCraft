use std::{env, path::PathBuf};

fn main() {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if !matches!(os.as_str(), "windows" | "macos") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    if os == "macos" {
        let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "arm64",
            Ok("x86_64") => "x86_64",
            _ => panic!("PixelCraft supports Apple Silicon and Intel macOS targets"),
        };
        let runtime = root.join("runtime/macos").join(arch);
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
}
