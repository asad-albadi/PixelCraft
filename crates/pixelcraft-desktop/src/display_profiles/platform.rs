use super::Surface;

pub(super) fn monitor_profile(surface: Surface) -> Result<Vec<u8>, String> {
    match surface {
        #[cfg(windows)]
        Surface::Windows(window) => windows_profile(window),
        #[cfg(target_os = "linux")]
        Surface::X11(window) => x11_profile(window),
        Surface::Unsupported => {
            Err("Automatic monitor profiles are unavailable for this display backend.".into())
        }
    }
}

#[cfg(windows)]
fn windows_profile(window: isize) -> Result<Vec<u8>, String> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            CreateDCW, DeleteDC, GetMonitorInfoW, HDC, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW,
            MonitorFromWindow,
        },
        UI::ColorSystem::GetICMProfileW,
    };

    // The device context belongs exclusively to this probe and is released
    // on every return path.
    struct DeviceContext(HDC);
    impl Drop for DeviceContext {
        fn drop(&mut self) {
            // SAFETY: this context was created by CreateDCW and is owned here.
            unsafe {
                DeleteDC(self.0);
            }
        }
    }

    // SAFETY: the HWND is borrowed from eframe. The Win32 calls validate handles;
    // MONITORINFOEXW and the filename buffer have the sizes provided to the API.
    let path = unsafe {
        let monitor = MonitorFromWindow(window as _, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info.monitorInfo) == 0 {
            return Err("Windows could not identify the editor monitor.".into());
        }
        let handle = CreateDCW(
            windows_sys::core::w!("DISPLAY"),
            info.szDevice.as_ptr(),
            ptr::null(),
            ptr::null(),
        );
        if handle.is_null() {
            return Err("Windows could not open the monitor device context.".into());
        }
        let dc = DeviceContext(handle);
        let mut buffer = vec![0u16; 512];
        let mut length = buffer.len() as u32;
        if GetICMProfileW(dc.0, &mut length, buffer.as_mut_ptr()) == 0 {
            if !(513..=32_768).contains(&length) {
                return Err(format!(
                    "Windows monitor ICC lookup failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            buffer.resize(length as usize, 0);
            if GetICMProfileW(dc.0, &mut length, buffer.as_mut_ptr()) == 0 {
                return Err(format!(
                    "Windows monitor ICC lookup failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let end = buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len());
        PathBuf::from(OsString::from_wide(&buffer[..end]))
    };
    crate::api::color::read_icc_profile(&path)
}

#[cfg(all(test, windows))]
#[test]
#[ignore = "Requires a configured Windows monitor ICC profile"]
fn windows_monitor_profile_reads_the_desktop_icc() {
    // SAFETY: GetDesktopWindow returns a borrowed desktop handle.
    let desktop = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() };
    let bytes = monitor_profile(Surface::Windows(desktop as isize)).unwrap();
    crate::api::color::validate_display_profile(&bytes).unwrap();
}

#[cfg(target_os = "linux")]
fn x11_profile(window: u32) -> Result<Vec<u8>, String> {
    use x11rb::protocol::{
        randr::ConnectionExt as _,
        xproto::{AtomEnum, ConnectionExt as _},
    };
    // The ICC chunk importer bounds profiles to 16 MiB. The X11 length is
    // expressed in 32-bit units and bytes_after exposes a truncated reply.
    const ICC_WORDS: u32 = 4 * 1024 * 1024;
    let (connection, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let geometry = connection
        .get_geometry(window)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let atom = connection
        .intern_atom(true, b"_ICC_PROFILE")
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .atom;
    if atom == 0 {
        return Err("X11 did not publish a monitor ICC profile.".into());
    }
    let center = connection
        .translate_coordinates(
            window,
            geometry.root,
            (geometry.width / 2) as i16,
            (geometry.height / 2) as i16,
        )
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let monitors = connection
        .randr_get_monitors(geometry.root, true)
        .ok()
        .and_then(|cookie| cookie.reply().ok());
    if let Some(monitor) = monitors.as_ref().and_then(|reply| {
        reply.monitors.iter().find(|m| {
            let x = i32::from(center.dst_x) - i32::from(m.x);
            let y = i32::from(center.dst_y) - i32::from(m.y);
            (0..i32::from(m.width)).contains(&x) && (0..i32::from(m.height)).contains(&y)
        })
    }) {
        for &output in &monitor.outputs {
            if let Some(profile) = connection
                .randr_get_output_property(output, atom, AtomEnum::ANY, 0, ICC_WORDS, false, false)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                && profile.format == 8
                && profile.bytes_after == 0
                && !profile.data.is_empty()
            {
                return crate::api::color::icc_profile(&profile.data);
            }
        }
        if !monitor.primary
            && monitors
                .as_ref()
                .is_some_and(|reply| reply.monitors.len() > 1)
        {
            return Err("X11 did not publish an ICC profile for this output.".into());
        }
    }
    let profile = connection
        .get_property(false, geometry.root, atom, AtomEnum::ANY, 0, ICC_WORDS)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    if profile.format != 8 || profile.bytes_after != 0 || profile.value.is_empty() {
        return Err("X11 did not publish a usable monitor ICC profile.".into());
    }
    crate::api::color::icc_profile(&profile.value)
}
