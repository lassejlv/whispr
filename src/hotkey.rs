//! Global push-to-talk watcher.
//!
//! CGEventTap on flagsChanged tracks the configured modifier down/up. Requires the user
//! to grant Input Monitoring / Accessibility permission to the app.

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::TCFType;
    use core_foundation::runloop::{
        CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRun, kCFRunLoopCommonModes,
    };
    use core_graphics::event::{
        CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventType,
    };
    use crossbeam_channel::Sender;
    use std::cell::Cell;
    use std::thread;

    use crate::config::{Hotkey, SharedSettings};

    #[derive(Debug, Clone, Copy)]
    pub enum HotkeyEvent {
        Pressed,
        Released,
    }

    pub fn spawn(tx: Sender<HotkeyEvent>, settings: SharedSettings) {
        thread::Builder::new()
            .name("yap-hotkey".into())
            .spawn(move || run_loop(tx, settings))
            .expect("spawn hotkey thread");
    }

    fn run_loop(tx: Sender<HotkeyEvent>, settings: SharedSettings) {
        let pressed = Cell::new(false);

        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::FlagsChanged],
            move |_, _, event| {
                let hotkey = settings.read().hotkey;
                let now_down = hotkey.is_down(event.get_flags());
                if now_down && !pressed.get() {
                    pressed.set(true);
                    let _ = tx.send(HotkeyEvent::Pressed);
                } else if !now_down && pressed.get() {
                    pressed.set(false);
                    let _ = tx.send(HotkeyEvent::Released);
                }
                None
            },
        );

        let tap = match tap {
            Ok(t) => t,
            Err(_) => {
                tracing::error!(
                    "failed to create CGEventTap — grant Input Monitoring permission in System Settings"
                );
                return;
            }
        };

        let loop_source = match tap.mach_port.create_runloop_source(0) {
            Ok(s) => s,
            Err(_) => {
                tracing::error!("create_runloop_source failed");
                return;
            }
        };

        unsafe {
            CFRunLoopAddSource(
                CFRunLoopGetCurrent(),
                loop_source.as_concrete_TypeRef(),
                kCFRunLoopCommonModes,
            );
        }
        tap.enable();
        unsafe {
            CFRunLoopRun();
        }
        drop(loop_source);
        drop(tap);
    }

    impl Hotkey {
        fn is_down(self, flags: CGEventFlags) -> bool {
            match self {
                Hotkey::Fn => flags.contains(CGEventFlags::CGEventFlagSecondaryFn),
                Hotkey::Option => flags.contains(CGEventFlags::CGEventFlagAlternate),
                Hotkey::Control => flags.contains(CGEventFlags::CGEventFlagControl),
                Hotkey::Command => flags.contains(CGEventFlags::CGEventFlagCommand),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::config::SharedSettings;
    use crossbeam_channel::Sender;

    #[derive(Debug, Clone, Copy)]
    pub enum HotkeyEvent {
        Pressed,
        Released,
    }

    pub fn spawn(_tx: Sender<HotkeyEvent>, _settings: SharedSettings) {
        tracing::warn!("hotkey watcher only implemented on macOS");
    }
}

pub use imp::{HotkeyEvent, spawn};
