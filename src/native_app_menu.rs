//! A main-thread-owned AppKit Settings command. No editor pointers cross FFI.
use eframe::egui;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, ns_string};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

struct TargetState {
    sender: SyncSender<()>,
    context: egui::Context,
}

define_class!(
    // SAFETY: NSObject has no additional subclassing requirements. The target
    // stays on the main thread and is retained for the entire menu lifetime.
    #[unsafe(super = NSObject)]
    #[name = "PixelCraftSettingsTarget"]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetState]
    struct SettingsTarget;

    unsafe impl NSObjectProtocol for SettingsTarget {}

    impl SettingsTarget {
        #[unsafe(method(pixelCraftOpenSettings:))]
        fn open_settings(&self, _sender: Option<&NSObject>) {
            // Coalesce repeated key events; AppKit must never block on egui.
            let _ = self.ivars().sender.try_send(());
            self.ivars().context.request_repaint();
        }
    }
);

pub(super) struct AppMenu {
    menu: Retained<NSMenu>,
    item: Retained<NSMenuItem>,
    separator: Retained<NSMenuItem>,
    _target: Retained<SettingsTarget>,
    receiver: Receiver<()>,
}

impl AppMenu {
    pub(super) fn install(context: &egui::Context) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let menu = NSApplication::sharedApplication(mtm)
            .mainMenu()?
            .itemAtIndex(0)?
            .submenu()?;
        let (sender, receiver) = sync_channel(1);
        let allocated = SettingsTarget::alloc(mtm).set_ivars(TargetState {
            sender,
            context: context.clone(),
        });
        // SAFETY: NSObject init returns this correctly allocated subclass.
        let target: Retained<SettingsTarget> = unsafe { msg_send![super(allocated), init] };
        // SAFETY: This selector is implemented above with the AppKit action ABI.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                ns_string!("Settings…"),
                Some(sel!(pixelCraftOpenSettings:)),
                ns_string!(","),
            )
        };
        // SAFETY: AppMenu retains the target and clears this weak pointer on Drop.
        unsafe { item.setTarget(Some(&target)) };
        item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
        let separator = NSMenuItem::separatorItem(mtm);
        let index = menu.numberOfItems().min(2);
        menu.insertItem_atIndex(&item, index);
        menu.insertItem_atIndex(&separator, index + 1);
        Some(Self {
            menu,
            item,
            separator,
            _target: target,
            receiver,
        })
    }

    pub(super) fn settings_requested(&self) -> bool {
        self.receiver.try_recv().is_ok()
    }

    /// Called by the packaged graphical smoke test, on AppKit's main thread.
    pub(super) fn verify_settings_command(&self) {
        assert_eq!(self.item.title().to_string(), "Settings…");
        assert_eq!(self.item.keyEquivalent().to_string(), ",");
        assert_eq!(
            self.item.keyEquivalentModifierMask(),
            NSEventModifierFlags::Command
        );
        let index = self.menu.indexOfItem(&self.item);
        assert!(index >= 0);
        let command_comma = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown, NSPoint::new(0., 0.), NSEventModifierFlags::Command,
            0., 0, None, ns_string!(","), ns_string!(","), false, 43,
        ).expect("cannot construct native Command-Comma event");
        assert!(
            self.menu.performKeyEquivalent(&command_comma),
            "native Command-Comma did not match Settings"
        );
        self.menu.performActionForItemAtIndex(index);
        assert!(
            self.settings_requested(),
            "native Settings action was not delivered"
        );
        assert!(
            !self.settings_requested(),
            "native events were not coalesced"
        );
        // Leave a real native event queued for the production ui() drain path.
        self.menu.performActionForItemAtIndex(index);
    }
}

impl Drop for AppMenu {
    fn drop(&mut self) {
        // SAFETY: Clear the weak target and selector before releasing the target.
        unsafe {
            self.item.setTarget(None);
            self.item.setAction(None);
        }
        self.menu.removeItem(&self.item);
        self.menu.removeItem(&self.separator);
    }
}
