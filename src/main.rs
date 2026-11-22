#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pixelcraft_desktop::run().map_err(Into::into)
}
