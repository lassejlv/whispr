//! Inject text into the focused app: set clipboard → synth Cmd+V → restore.

use anyhow::Result;
use std::thread;
use std::time::Duration;

const KVK_ANSI_V: u16 = 0x09;

pub fn paste_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let mut clipboard = arboard::Clipboard::new()?;
    let prev = clipboard.get_text().ok();
    clipboard.set_text(text.to_string())?;

    // Small delay so the new clipboard value is settled before paste fires.
    thread::sleep(Duration::from_millis(30));
    synth_cmd_v();

    // Wait for receiving app to consume the paste before restoring. Some apps
    // read the clipboard after handling the key event, not during it.
    let prev_owned = prev;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(2_000));
        if let Some(prev) = prev_owned
            && let Ok(mut cb) = arboard::Clipboard::new()
        {
            let _ = cb.set_text(prev);
        }
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn synth_cmd_v() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    const KVK_COMMAND: u16 = 0x37;

    let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };
    let Ok(cmd_down) = CGEvent::new_keyboard_event(src.clone(), KVK_COMMAND, true) else {
        return;
    };
    cmd_down.set_flags(CGEventFlags::CGEventFlagCommand);
    cmd_down.post(CGEventTapLocation::HID);

    let Ok(v_down) = CGEvent::new_keyboard_event(src.clone(), KVK_ANSI_V, true) else {
        return;
    };
    v_down.set_flags(CGEventFlags::CGEventFlagCommand);
    v_down.post(CGEventTapLocation::HID);

    let Ok(v_up) = CGEvent::new_keyboard_event(src.clone(), KVK_ANSI_V, false) else {
        return;
    };
    v_up.set_flags(CGEventFlags::CGEventFlagCommand);
    v_up.post(CGEventTapLocation::HID);

    let Ok(cmd_up) = CGEvent::new_keyboard_event(src, KVK_COMMAND, false) else {
        return;
    };
    cmd_up.set_flags(CGEventFlags::CGEventFlagNull);
    cmd_up.post(CGEventTapLocation::HID);
}

#[cfg(not(target_os = "macos"))]
fn synth_cmd_v() {
    tracing::warn!("paste only implemented on macOS");
}
