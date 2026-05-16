//! macOS menubar tray: NSStatusBar item with Settings / Quit menu.

#[cfg(target_os = "macos")]
mod imp {
    use crossbeam_channel::Sender;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
    use objc2_app_kit::{
        NSAppearance, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem, NSStatusBar,
    };
    use objc2_foundation::{MainThreadMarker, NSString};
    use std::sync::OnceLock;

    use crate::state::UiCmd;

    static TRAY_TX: OnceLock<Sender<UiCmd>> = OnceLock::new();

    declare_class!(
        struct TrayTarget;

        unsafe impl ClassType for TrayTarget {
            type Super = NSObject;
            type Mutability = mutability::InteriorMutable;
            const NAME: &'static str = "YapTrayTarget";
        }

        impl DeclaredClass for TrayTarget {}

        unsafe impl TrayTarget {
            #[method(openSettings:)]
            fn open_settings(&self, _sender: Option<&AnyObject>) {
                if let Some(tx) = TRAY_TX.get() {
                    let _ = tx.send(UiCmd::OpenSettings);
                }
            }

            #[method(quit:)]
            fn quit(&self, _sender: Option<&AnyObject>) {
                if let Some(tx) = TRAY_TX.get() {
                    let _ = tx.send(UiCmd::Quit);
                }
            }

            #[method(hideYap:)]
            fn hide_yap(&self, _sender: Option<&AnyObject>) {
                if let Some(tx) = TRAY_TX.get() {
                    let _ = tx.send(UiCmd::Hide);
                }
            }
        }
    );

    /// Install the menubar item. Must be called on the main thread.
    /// Returned status item is leaked so it lives for app lifetime.
    pub fn install(tx: Sender<UiCmd>) {
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::error!("tray install must run on main thread");
            return;
        };
        let _ = TRAY_TX.set(tx);

        force_dark_appearance(mtm);

        let target: Retained<TrayTarget> = unsafe { msg_send_id![TrayTarget::alloc(), init] };

        install_app_menu(mtm, &target);
        let bar = unsafe { NSStatusBar::systemStatusBar() };
        // -1.0 == NSVariableStatusItemLength
        let item = unsafe { bar.statusItemWithLength(-1.0) };

        if let Some(button) = unsafe { item.button(mtm) } {
            unsafe { button.setTitle(&NSString::from_str("●")) };
        }

        let menu = NSMenu::new(mtm);

        let settings_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str("Settings…"),
                Some(sel!(openSettings:)),
                &NSString::from_str(""),
            )
        };
        unsafe { settings_item.setTarget(Some(&*target)) };
        menu.addItem(&settings_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str("Quit Yap"),
                Some(sel!(quit:)),
                &NSString::from_str("q"),
            )
        };
        unsafe { quit_item.setTarget(Some(&*target)) };
        menu.addItem(&quit_item);

        unsafe { item.setMenu(Some(&menu)) };

        // Intentionally leak so the status item + target survive process lifetime.
        std::mem::forget(target);
        std::mem::forget(item);
    }

    fn force_dark_appearance(mtm: MainThreadMarker) {
        let app = NSApplication::sharedApplication(mtm);
        let name = NSString::from_str("NSAppearanceNameDarkAqua");
        if let Some(appearance) = NSAppearance::appearanceNamed(&name) {
            app.setAppearance(Some(&appearance));
        }
    }

    fn install_app_menu(mtm: MainThreadMarker, target: &TrayTarget) {
        let main_menu = NSMenu::new(mtm);
        let app_menu_item = NSMenuItem::new(mtm);
        main_menu.addItem(&app_menu_item);

        let app_menu = NSMenu::new(mtm);
        let settings_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str("Settings…"),
                Some(sel!(openSettings:)),
                &NSString::from_str(","),
            )
        };
        unsafe { settings_item.setTarget(Some(target)) };
        settings_item
            .setKeyEquivalentModifierMask(NSEventModifierFlags::NSEventModifierFlagCommand);
        app_menu.addItem(&settings_item);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let hide_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str("Hide Yap"),
                Some(sel!(hideYap:)),
                &NSString::from_str("m"),
            )
        };
        unsafe { hide_item.setTarget(Some(target)) };
        hide_item.setKeyEquivalentModifierMask(NSEventModifierFlags::NSEventModifierFlagCommand);
        app_menu.addItem(&hide_item);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &NSString::from_str("Quit Yap"),
                Some(sel!(quit:)),
                &NSString::from_str("q"),
            )
        };
        unsafe { quit_item.setTarget(Some(target)) };
        quit_item.setKeyEquivalentModifierMask(NSEventModifierFlags::NSEventModifierFlagCommand);
        app_menu.addItem(&quit_item);

        app_menu_item.setSubmenu(Some(&app_menu));
        NSApplication::sharedApplication(mtm).setMainMenu(Some(&main_menu));
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::state::UiCmd;
    use crossbeam_channel::Sender;

    pub fn install(_tx: Sender<UiCmd>) {}
}

pub use imp::install;
