//! Linux's winit backend does not report system appearance. Use the desktop portal
//! on a worker so D-Bus startup and waiting for changes never block the editor.
use eframe::egui::{Context, Theme};
use std::sync::mpsc::Receiver;

pub fn watch(ctx: Context) -> Option<Receiver<Option<Theme>>> {
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("desktop-appearance".into())
            .spawn(move || {
                loop {
                    let publish = |theme| {
                        let alive = tx.send(theme).is_ok();
                        if alive {
                            ctx.request_repaint();
                        }
                        alive
                    };
                    // Reconnect if the portal starts late or is restarted.
                    if monitor(&publish).is_ok() || !publish(None) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            })
            .ok()?;
        Some(rx)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = ctx;
        None
    }
}

#[cfg(target_os = "linux")]
fn portal_theme(value: &zbus::zvariant::OwnedValue) -> Option<Theme> {
    match u32::try_from(value).ok()? {
        1 => Some(Theme::Dark),
        2 => Some(Theme::Light),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn monitor(publish: &impl Fn(Option<Theme>) -> bool) -> zbus::Result<()> {
    use std::collections::HashMap;
    use zbus::{
        blocking::{Connection, Proxy},
        zvariant::OwnedValue,
    };
    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )?;
    // Subscribe before reading so changes during startup cannot be missed.
    let signals = proxy.receive_signal_with_args(
        "SettingChanged",
        &[(0, "org.freedesktop.appearance"), (1, "color-scheme")],
    )?;
    let settings: HashMap<String, HashMap<String, OwnedValue>> =
        proxy.call("ReadAll", &(vec!["org.freedesktop.appearance"],))?;
    let theme = settings
        .get("org.freedesktop.appearance")
        .and_then(|values| values.get("color-scheme"))
        .and_then(portal_theme);
    if !publish(theme) {
        return Ok(());
    }
    for signal in signals {
        let (namespace, key, value): (String, String, OwnedValue) = signal.body().deserialize()?;
        if namespace == "org.freedesktop.appearance"
            && key == "color-scheme"
            && !publish(portal_theme(&value))
        {
            return Ok(());
        }
    }
    Err(zbus::Error::Failure(
        "Appearance portal disconnected".into(),
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn portal_color_scheme_values_follow_desktop_contract() {
        for (value, expected) in [
            (0u32, None),
            (1, Some(Theme::Dark)),
            (2, Some(Theme::Light)),
            (3, None),
        ] {
            assert_eq!(portal_theme(&value.into()), expected);
        }
        assert_eq!(portal_theme(&true.into()), None);
    }
}
