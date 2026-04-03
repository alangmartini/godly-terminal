//! Framework-agnostic keyboard types.
//!
//! These mirror the `iced::keyboard` / `winit::keyboard` API surface used
//! throughout app-adapter so the crate doesn't depend on either framework.

use std::borrow::Cow;

/// Logical key value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable character (may be multi-codepoint, stored as a small string).
    Character(Cow<'static, str>),
    /// A named non-printable key.
    Named(Named),
    /// Key that could not be identified.
    Unidentified,
}

/// Named (non-printable) keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Named {
    Enter,
    Backspace,
    Tab,
    Escape,
    Space,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// Modifier key state (bitflags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    const SHIFT_BIT: u8 = 1;
    const CTRL_BIT: u8 = 2;
    const ALT_BIT: u8 = 4;
    const LOGO_BIT: u8 = 8;

    pub const SHIFT: Self = Self(Self::SHIFT_BIT);
    pub const CTRL: Self = Self(Self::CTRL_BIT);
    pub const ALT: Self = Self(Self::ALT_BIT);
    pub const LOGO: Self = Self(Self::LOGO_BIT);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn shift(self) -> bool {
        self.0 & Self::SHIFT_BIT != 0
    }

    pub const fn control(self) -> bool {
        self.0 & Self::CTRL_BIT != 0
    }

    pub const fn alt(self) -> bool {
        self.0 & Self::ALT_BIT != 0
    }

    pub const fn logo(self) -> bool {
        self.0 & Self::LOGO_BIT != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Physical key position (scan code), layout-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Physical {
    Code(Code),
}

/// Key codes for physical key positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    Backslash,
}
