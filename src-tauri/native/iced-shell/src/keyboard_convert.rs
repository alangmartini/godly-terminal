//! Iced → app-adapter keyboard type conversions.
//!
//! `iced-shell` receives keyboard events in `iced::keyboard` types from the
//! framework event loop.  `godly-app-adapter` defines its own framework-agnostic
//! keyboard types.  This module bridges the two so the adapter functions can be
//! called from the iced event handlers.

use godly_app_adapter::keyboard as ak;
use iced::keyboard as ik;
use std::borrow::Cow;

/// Convert an iced logical key to the adapter key type.
pub fn convert_key(key: &ik::Key) -> ak::Key {
    match key {
        ik::Key::Character(s) => ak::Key::Character(Cow::Owned(s.to_string())),
        ik::Key::Named(n) => match convert_named(*n) {
            Some(named) => ak::Key::Named(named),
            None => ak::Key::Unidentified,
        },
        ik::Key::Unidentified => ak::Key::Unidentified,
    }
}

/// Convert an iced named key, returning `None` for keys the adapter doesn't model
/// (modifier-only keys, media keys, etc.).
fn convert_named(n: ik::key::Named) -> Option<ak::Named> {
    Some(match n {
        ik::key::Named::Enter => ak::Named::Enter,
        ik::key::Named::Backspace => ak::Named::Backspace,
        ik::key::Named::Tab => ak::Named::Tab,
        ik::key::Named::Escape => ak::Named::Escape,
        ik::key::Named::Space => ak::Named::Space,
        ik::key::Named::Delete => ak::Named::Delete,
        ik::key::Named::Insert => ak::Named::Insert,
        ik::key::Named::Home => ak::Named::Home,
        ik::key::Named::End => ak::Named::End,
        ik::key::Named::PageUp => ak::Named::PageUp,
        ik::key::Named::PageDown => ak::Named::PageDown,
        ik::key::Named::ArrowUp => ak::Named::ArrowUp,
        ik::key::Named::ArrowDown => ak::Named::ArrowDown,
        ik::key::Named::ArrowLeft => ak::Named::ArrowLeft,
        ik::key::Named::ArrowRight => ak::Named::ArrowRight,
        ik::key::Named::F1 => ak::Named::F1,
        ik::key::Named::F2 => ak::Named::F2,
        ik::key::Named::F3 => ak::Named::F3,
        ik::key::Named::F4 => ak::Named::F4,
        ik::key::Named::F5 => ak::Named::F5,
        ik::key::Named::F6 => ak::Named::F6,
        ik::key::Named::F7 => ak::Named::F7,
        ik::key::Named::F8 => ak::Named::F8,
        ik::key::Named::F9 => ak::Named::F9,
        ik::key::Named::F10 => ak::Named::F10,
        ik::key::Named::F11 => ak::Named::F11,
        ik::key::Named::F12 => ak::Named::F12,
        _ => return None,
    })
}

/// Convert iced modifier flags to adapter modifiers.
pub fn convert_modifiers(m: ik::Modifiers) -> ak::Modifiers {
    let mut out = ak::Modifiers::empty();
    if m.shift() {
        out = out.union(ak::Modifiers::SHIFT);
    }
    if m.control() {
        out = out.union(ak::Modifiers::CTRL);
    }
    if m.alt() {
        out = out.union(ak::Modifiers::ALT);
    }
    if m.logo() {
        out = out.union(ak::Modifiers::LOGO);
    }
    out
}

/// Convert an iced physical key to the adapter type.
/// Returns `None` for keys the adapter doesn't model.
pub fn convert_physical(p: &ik::key::Physical) -> Option<ak::Physical> {
    match p {
        ik::key::Physical::Code(ik::key::Code::Backslash) => {
            Some(ak::Physical::Code(ak::Code::Backslash))
        }
        _ => None,
    }
}

/// Returns `true` if the iced key is a modifier-only key (Ctrl, Shift, Alt, Super).
pub fn is_modifier_only_key(key: &ik::Key) -> bool {
    matches!(
        key,
        ik::Key::Named(
            ik::key::Named::Control
                | ik::key::Named::Shift
                | ik::key::Named::Alt
                | ik::key::Named::Super
        )
    )
}
