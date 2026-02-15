// Originally from vte by Joe Wilm / alacritty (https://github.com/alacritty/vte)
// Copyright (c) 2016 Joe Wilm
// Licensed under MIT — see LICENSE-MIT in this directory
// Modified for godly-vt
//
//! Parser for implementing virtual terminal emulators
//!
//! [`Parser`] is implemented according to [Paul Williams' ANSI parser state
//! machine]. The state machine doesn't assign meaning to the parsed data and is
//! thus not itself sufficient for writing a terminal emulator. Instead, it is
//! expected that an implementation of [`Perform`] is provided which does
//! something useful with the parsed data. The [`Parser`] handles the book
//! keeping, and the [`Perform`] gets to simply handle actions.
//!
//! # Examples
//!
//! For an example of using the [`Parser`] please see the examples folder. The
//! example included there simply logs all the actions [`Perform`] does. One
//! quick way to see it in action is to pipe `printf` into it
//!
//! ```sh
//! printf '\x1b[31mExample' | cargo run --example parselog
//! ```
//!
//! # Differences from original state machine description
//!
//! * UTF-8 Support for Input
//! * OSC Strings can be terminated by 0x07
//! * Only supports 7-bit codes
//!
//! [`Parser`]: struct.Parser.html
//! [`Perform`]: trait.Perform.html
//! [Paul Williams' ANSI parser state machine]: https://vt100.net/emu/dec_ansi_parser
use core::mem::MaybeUninit;
use core::str;

pub mod params;

pub use params::{Params, ParamsIter};

const MAX_INTERMEDIATES: usize = 2;
const MAX_OSC_PARAMS: usize = 16;
const MAX_OSC_RAW: usize = 1024;

/// Parser for raw _VTE_ protocol which delegates actions to a [`Perform`]
///
/// [`Perform`]: trait.Perform.html
///
/// Generic over the value for the size of the raw Operating System Command
/// buffer. Only used when the `std` feature is not enabled.
#[derive(Default)]
pub struct Parser<const OSC_RAW_BUF_SIZE: usize = MAX_OSC_RAW> {
    state: State,
    intermediates: [u8; MAX_INTERMEDIATES],
    intermediate_idx: usize,
    params: Params,
    param: u16,
    osc_raw: Vec<u8>,
    osc_params: [(usize, usize); MAX_OSC_PARAMS],
    osc_num_params: usize,
    ignoring: bool,
    partial_utf8: [u8; 4],
    partial_utf8_len: usize,
}

impl Parser {
    /// Create a new Parser
    pub fn new() -> Parser {
        Default::default()
    }
}

impl<const OSC_RAW_BUF_SIZE: usize> Parser<OSC_RAW_BUF_SIZE> {
    #[inline]
    fn params(&self) -> &Params {
        &self.params
    }

    #[inline]
    fn intermediates(&self) -> &[u8] {
        &self.intermediates[..self.intermediate_idx]
    }

    /// Advance the parser state.
    ///
    /// Requires a [`Perform`] implementation to handle the triggered actions.
    ///
    /// [`Perform`]: trait.Perform.html
    #[inline]
    pub fn advance<P: Perform>(&mut self, performer: &mut P, bytes: &[u8]) {
        let mut i = 0;

        // Handle partial codepoints from previous calls to `advance`.
        if self.partial_utf8_len != 0 {
            i += self.advance_partial_utf8(performer, bytes);
        }

        while i != bytes.len() {
            match self.state {
                State::Ground => i += self.advance_ground(performer, &bytes[i..]),
                _ => {
                    // Inlining it results in worse codegen.
                    let byte = bytes[i];
                    self.change_state(performer, byte);
                    i += 1;
                },
            }
        }
    }

    /// Partially advance the parser state.
    ///
    /// This is equivalent to [`Self::advance`], but stops when
    /// [`Perform::terminated`] is true after reading a byte.
    ///
    /// Returns the number of bytes read before termination.
    ///
    /// See [`Self::advance`] for more details.
    #[inline]
    #[must_use = "Returned value should be used to processs the remaining bytes"]
    pub fn advance_until_terminated<P: Perform>(
        &mut self,
        performer: &mut P,
        bytes: &[u8],
    ) -> usize {
        let mut i = 0;

        // Handle partial codepoints from previous calls to `advance`.
        if self.partial_utf8_len != 0 {
            i += self.advance_partial_utf8(performer, bytes);
        }

        while i != bytes.len() && !performer.terminated() {
            match self.state {
                State::Ground => i += self.advance_ground(performer, &bytes[i..]),
                _ => {
                    // Inlining it results in worse codegen.
                    let byte = bytes[i];
                    self.change_state(performer, byte);
                    i += 1;
                },
            }
        }

        i
    }

    #[inline(always)]
    fn change_state<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match self.state {
            State::CsiEntry => self.advance_csi_entry(performer, byte),
            State::CsiIgnore => self.advance_csi_ignore(performer, byte),
            State::CsiIntermediate => self.advance_csi_intermediate(performer, byte),
            State::CsiParam => self.advance_csi_param(performer, byte),
            State::DcsEntry => self.advance_dcs_entry(performer, byte),
            State::DcsIgnore => self.anywhere(performer, byte),
            State::DcsIntermediate => self.advance_dcs_intermediate(performer, byte),
            State::DcsParam => self.advance_dcs_param(performer, byte),
            State::DcsPassthrough => self.advance_dcs_passthrough(performer, byte),
            State::Escape => self.advance_esc(performer, byte),
            State::EscapeIntermediate => self.advance_esc_intermediate(performer, byte),
            State::OscString => self.advance_osc_string(performer, byte),
            State::SosPmApcString => self.anywhere(performer, byte),
            State::Ground => unreachable!(),
        }
    }

    #[inline(always)]
    fn advance_csi_entry<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x2F => {
                self.action_collect(byte);
                self.state = State::CsiIntermediate
            },
            0x30..=0x39 => {
                self.action_paramnext(byte);
                self.state = State::CsiParam
            },
            0x3A => {
                self.action_subparam();
                self.state = State::CsiParam
            },
            0x3B => {
                self.action_param();
                self.state = State::CsiParam
            },
            0x3C..=0x3F => {
                self.action_collect(byte);
                self.state = State::CsiParam
            },
            0x40..=0x7E => self.action_csi_dispatch(performer, byte),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_csi_ignore<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x3F => (),
            0x40..=0x7E => self.state = State::Ground,
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_csi_intermediate<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x2F => self.action_collect(byte),
            0x30..=0x3F => self.state = State::CsiIgnore,
            0x40..=0x7E => self.action_csi_dispatch(performer, byte),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_csi_param<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x2F => {
                self.action_collect(byte);
                self.state = State::CsiIntermediate
            },
            0x30..=0x39 => self.action_paramnext(byte),
            0x3A => self.action_subparam(),
            0x3B => self.action_param(),
            0x3C..=0x3F => self.state = State::CsiIgnore,
            0x40..=0x7E => self.action_csi_dispatch(performer, byte),
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_dcs_entry<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => (),
            0x20..=0x2F => {
                self.action_collect(byte);
                self.state = State::DcsIntermediate
            },
            0x30..=0x39 => {
                self.action_paramnext(byte);
                self.state = State::DcsParam
            },
            0x3A => {
                self.action_subparam();
                self.state = State::DcsParam
            },
            0x3B => {
                self.action_param();
                self.state = State::DcsParam
            },
            0x3C..=0x3F => {
                self.action_collect(byte);
                self.state = State::DcsParam
            },
            0x40..=0x7E => self.action_hook(performer, byte),
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_dcs_intermediate<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => (),
            0x20..=0x2F => self.action_collect(byte),
            0x30..=0x3F => self.state = State::DcsIgnore,
            0x40..=0x7E => self.action_hook(performer, byte),
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_dcs_param<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => (),
            0x20..=0x2F => {
                self.action_collect(byte);
                self.state = State::DcsIntermediate
            },
            0x30..=0x39 => self.action_paramnext(byte),
            0x3A => self.action_subparam(),
            0x3B => self.action_param(),
            0x3C..=0x3F => self.state = State::DcsIgnore,
            0x40..=0x7E => self.action_hook(performer, byte),
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_dcs_passthrough<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x7E => performer.put(byte),
            0x18 | 0x1A => {
                performer.unhook();
                performer.execute(byte);
                self.state = State::Ground
            },
            0x1B => {
                performer.unhook();
                self.reset_params();
                self.state = State::Escape
            },
            0x7F => (),
            0x9C => {
                performer.unhook();
                self.state = State::Ground
            },
            _ => (),
        }
    }

    #[inline(always)]
    fn advance_esc<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x2F => {
                self.action_collect(byte);
                self.state = State::EscapeIntermediate
            },
            0x30..=0x4F => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            0x50 => {
                self.reset_params();
                self.state = State::DcsEntry
            },
            0x51..=0x57 => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            0x58 => self.state = State::SosPmApcString,
            0x59..=0x5A => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            0x5B => {
                self.reset_params();
                self.state = State::CsiEntry
            },
            0x5C => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            0x5D => {
                self.osc_raw.clear();
                self.osc_num_params = 0;
                self.state = State::OscString
            },
            0x5E..=0x5F => self.state = State::SosPmApcString,
            0x60..=0x7E => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            // Anywhere.
            0x18 | 0x1A => {
                performer.execute(byte);
                self.state = State::Ground
            },
            0x1B => (),
            _ => (),
        }
    }

    #[inline(always)]
    fn advance_esc_intermediate<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => performer.execute(byte),
            0x20..=0x2F => self.action_collect(byte),
            0x30..=0x7E => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
                self.state = State::Ground
            },
            0x7F => (),
            _ => self.anywhere(performer, byte),
        }
    }

    #[inline(always)]
    fn advance_osc_string<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1C..=0x1F => (),
            0x07 => {
                self.osc_end(performer, byte);
                self.state = State::Ground
            },
            0x18 | 0x1A => {
                self.osc_end(performer, byte);
                performer.execute(byte);
                self.state = State::Ground
            },
            0x1B => {
                self.osc_end(performer, byte);
                self.reset_params();
                self.state = State::Escape
            },
            0x3B => self.action_osc_put_param(),
            _ => self.action_osc_put(byte),
        }
    }

    #[inline(always)]
    fn anywhere<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        match byte {
            0x18 | 0x1A => {
                performer.execute(byte);
                self.state = State::Ground
            },
            0x1B => {
                self.reset_params();
                self.state = State::Escape
            },
            _ => (),
        }
    }

    #[inline]
    fn action_csi_dispatch<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
        }
        performer.csi_dispatch(self.params(), self.intermediates(), self.ignoring, byte as char);

        self.state = State::Ground
    }

    #[inline]
    fn action_hook<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
        }
        performer.hook(self.params(), self.intermediates(), self.ignoring, byte as char);
        self.state = State::DcsPassthrough;
    }

    #[inline]
    fn action_collect(&mut self, byte: u8) {
        if self.intermediate_idx == MAX_INTERMEDIATES {
            self.ignoring = true;
        } else {
            self.intermediates[self.intermediate_idx] = byte;
            self.intermediate_idx += 1;
        }
    }

    /// Advance to the next subparameter.
    #[inline]
    fn action_subparam(&mut self) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.extend(self.param);
            self.param = 0;
        }
    }

    /// Advance to the next parameter.
    #[inline]
    fn action_param(&mut self) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            self.params.push(self.param);
            self.param = 0;
        }
    }

    /// Advance inside the parameter without terminating it.
    #[inline]
    fn action_paramnext(&mut self, byte: u8) {
        if self.params.is_full() {
            self.ignoring = true;
        } else {
            // Continue collecting bytes into param.
            self.param = self.param.saturating_mul(10);
            self.param = self.param.saturating_add((byte - b'0') as u16);
        }
    }

    /// Add OSC param separator.
    #[inline]
    fn action_osc_put_param(&mut self) {
        let idx = self.osc_raw.len();

        let param_idx = self.osc_num_params;
        match param_idx {
            // First param is special - 0 to current byte index.
            0 => self.osc_params[param_idx] = (0, idx),

            // Only process up to MAX_OSC_PARAMS.
            MAX_OSC_PARAMS => return,

            // All other params depend on previous indexing.
            _ => {
                let prev = self.osc_params[param_idx - 1];
                let begin = prev.1;
                self.osc_params[param_idx] = (begin, idx);
            },
        }

        self.osc_num_params += 1;
    }

    #[inline(always)]
    fn action_osc_put(&mut self, byte: u8) {
        self.osc_raw.push(byte);
    }

    fn osc_end<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        self.action_osc_put_param();
        self.osc_dispatch(performer, byte);
        self.osc_raw.clear();
        self.osc_num_params = 0;
    }

    /// Reset escape sequence parameters and intermediates.
    #[inline]
    fn reset_params(&mut self) {
        self.intermediate_idx = 0;
        self.ignoring = false;
        self.param = 0;

        self.params.clear();
    }

    /// Separate method for osc_dispatch that borrows self as read-only
    ///
    /// The aliasing is needed here for multiple slices into self.osc_raw
    #[inline]
    fn osc_dispatch<P: Perform>(&self, performer: &mut P, byte: u8) {
        let mut slices: [MaybeUninit<&[u8]>; MAX_OSC_PARAMS] =
            unsafe { MaybeUninit::uninit().assume_init() };

        for (i, slice) in slices.iter_mut().enumerate().take(self.osc_num_params) {
            let indices = self.osc_params[i];
            *slice = MaybeUninit::new(&self.osc_raw[indices.0..indices.1]);
        }

        unsafe {
            let num_params = self.osc_num_params;
            let params = &slices[..num_params] as *const [MaybeUninit<&[u8]>] as *const [&[u8]];
            performer.osc_dispatch(&*params, byte == 0x07);
        }
    }

    /// Advance the parser state from ground.
    ///
    /// The ground state is handled separately since it can only be left using
    /// the escape character (`\x1b`). This allows more efficient parsing by
    /// using SIMD scanning.
    ///
    /// Optimization strategy:
    /// 1. Use SIMD `scan_for_control()` to find the first C0 control or DEL.
    ///    Everything before it is printable bytes (0x20..=0x7E or 0x80..=0xFF).
    /// 2. For the printable portion: use `is_all_ascii()` to skip UTF-8 decode
    ///    when possible, or `simdutf8` for fast validation otherwise.
    /// 3. Dispatch the entire printable run via `print_str()` — no per-char
    ///    control check needed since the scanner already excluded them.
    /// 4. Handle the control character (ESC transitions state, others execute).
    #[inline]
    fn advance_ground<P: Perform>(&mut self, performer: &mut P, bytes: &[u8]) -> usize {
        let num_bytes = bytes.len();

        // Find the first control character (< 0x20 or 0x7F) using SIMD.
        let control_pos = crate::simd::scan_for_control(bytes).unwrap_or(num_bytes);

        // If the first byte is a control character, handle it immediately.
        if control_pos == 0 {
            let byte = bytes[0];
            if byte == 0x1B {
                // Try CSI fast-path: ESC [ <params> <final>
                if let Some(consumed) = self.try_csi_fast_path(performer, bytes) {
                    return consumed;
                }
                self.state = State::Escape;
                self.reset_params();
            } else {
                performer.execute(byte);
            }
            return 1;
        }

        // Everything in bytes[..control_pos] is printable (>= 0x20, != 0x7F).
        // Validate as UTF-8 and dispatch.
        let printable = &bytes[..control_pos];
        let processed = self.dispatch_printable(performer, printable, control_pos, num_bytes);
        if processed < control_pos {
            // UTF-8 error cut things short; return what was consumed.
            return processed;
        }

        // Now handle the control character (if any).
        let mut total = control_pos;
        if total < num_bytes {
            let byte = bytes[total];
            if byte == 0x1B {
                // Try CSI fast-path for ESC after printable text.
                if let Some(consumed) = self.try_csi_fast_path(performer, &bytes[total..]) {
                    return total + consumed;
                }
                self.state = State::Escape;
                self.reset_params();
            } else {
                performer.execute(byte);
            }
            total += 1;
        }

        total
    }

    /// Validate a printable byte slice as UTF-8 and dispatch via `print_str()`.
    ///
    /// The input is guaranteed to contain no C0 controls or DEL, only bytes
    /// >= 0x20 and != 0x7F. However, bytes 0x80-0x9F may appear as part of
    /// multi-byte UTF-8 sequences, and the corresponding Unicode code points
    /// (U+0080..U+009F) are C1 controls that `ground_dispatch` handles.
    #[inline]
    fn dispatch_printable<P: Perform>(
        &mut self,
        performer: &mut P,
        printable: &[u8],
        control_pos: usize,
        num_bytes: usize,
    ) -> usize {
        // ASCII fast path: if all bytes are < 0x80, skip UTF-8 validation entirely.
        if crate::simd::is_all_ascii(printable) {
            // Pure ASCII — safe to use from_utf8_unchecked, and no C1 controls
            // are possible (all bytes are 0x20..=0x7E).
            let text = unsafe { str::from_utf8_unchecked(printable) };
            performer.print_str(text);
            return control_pos;
        }

        // Non-ASCII path: use simdutf8 for fast validation.
        match simdutf8::basic::from_utf8(printable) {
            Ok(parsed) => {
                // The printable range has no C0/DEL controls, but may contain
                // C1 code points (U+0080..U+009F) encoded as 2-byte UTF-8.
                // Use ground_dispatch to handle those.
                Self::ground_dispatch(performer, parsed);
                control_pos
            },
            Err(_) => {
                // simdutf8's basic error doesn't give us valid_up_to, so fall
                // back to std for error details on the uncommon invalid-UTF-8 path.
                match str::from_utf8(printable) {
                    Ok(_) => unreachable!("simdutf8 and std disagree"),
                    Err(err) => {
                        let valid_bytes = err.valid_up_to();
                        if valid_bytes > 0 {
                            let parsed = unsafe { str::from_utf8_unchecked(&printable[..valid_bytes]) };
                            Self::ground_dispatch(performer, parsed);
                        }

                        match err.error_len() {
                            Some(len) => {
                                // Execute C1 escapes or emit replacement character.
                                if len == 1 && printable[valid_bytes] <= 0x9F {
                                    performer.execute(printable[valid_bytes]);
                                } else {
                                    performer.print('�');
                                }
                                valid_bytes + len
                            },
                            None => {
                                if control_pos < num_bytes {
                                    // Partial UTF-8 cut off by a control char.
                                    performer.print('�');
                                    control_pos
                                } else {
                                    // Partial UTF-8 at buffer end — store for next call.
                                    let extra_bytes = printable.len() - valid_bytes;
                                    let partial_len = self.partial_utf8_len + extra_bytes;
                                    self.partial_utf8[self.partial_utf8_len..partial_len]
                                        .copy_from_slice(&printable[valid_bytes..]);
                                    self.partial_utf8_len = partial_len;
                                    num_bytes
                                }
                            },
                        }
                    },
                }
            },
        }
    }

    /// Advance the parser while processing a partial utf8 codepoint.
    #[inline]
    fn advance_partial_utf8<P: Perform>(&mut self, performer: &mut P, bytes: &[u8]) -> usize {
        // Try to copy up to 3 more characters, to ensure the codepoint is complete.
        let old_bytes = self.partial_utf8_len;
        let to_copy = bytes.len().min(self.partial_utf8.len() - old_bytes);
        self.partial_utf8[old_bytes..old_bytes + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.partial_utf8_len += to_copy;

        // Parse the unicode character.
        match str::from_utf8(&self.partial_utf8[..self.partial_utf8_len]) {
            // If the entire buffer is valid, use the first character and continue parsing.
            Ok(parsed) => {
                let c = unsafe { parsed.chars().next().unwrap_unchecked() };
                performer.print(c);

                self.partial_utf8_len = 0;
                c.len_utf8() - old_bytes
            },
            Err(err) => {
                let valid_bytes = err.valid_up_to();
                // If we have any valid bytes, that means we partially copied another
                // utf8 character into `partial_utf8`. Since we only care about the
                // first character, we just ignore the rest.
                if valid_bytes > 0 {
                    let c = unsafe {
                        let parsed = str::from_utf8_unchecked(&self.partial_utf8[..valid_bytes]);
                        parsed.chars().next().unwrap_unchecked()
                    };

                    performer.print(c);

                    self.partial_utf8_len = 0;
                    return valid_bytes - old_bytes;
                }

                match err.error_len() {
                    // If the partial character was also invalid, emit the replacement
                    // character.
                    Some(invalid_len) => {
                        performer.print('�');

                        self.partial_utf8_len = 0;
                        invalid_len - old_bytes
                    },
                    // If the character still isn't complete, wait for more data.
                    None => to_copy,
                }
            },
        }
    }

    /// Try to parse a CSI sequence inline without entering the state machine.
    ///
    /// Called when `bytes[0] == 0x1B`. Looks ahead for `[` and tries to parse
    /// the entire CSI sequence in one shot. Returns `Some(consumed)` if the
    /// fast path succeeded, `None` if the caller should fall back to the
    /// normal state machine.
    ///
    /// Handles common sequences without intermediates:
    /// - SGR (m): `ESC [ <params> m`
    /// - CUP (H/f): `ESC [ <params> H` or `f`
    /// - ED (J): `ESC [ <params> J`
    /// - EL (K): `ESC [ <params> K`
    /// - Cursor movement (A/B/C/D): `ESC [ <params> A/B/C/D`
    /// - Other simple CSI: any `ESC [ <digits;> <0x40..0x7E>`
    ///
    /// Falls back to normal parsing for:
    /// - Incomplete sequences (buffer boundary)
    /// - Private marker sequences (`ESC [ ?`)
    /// - Sequences with intermediates (`ESC [ <params> <0x20..0x2F> <final>`)
    /// - Subparameters (`:`)
    #[inline]
    fn try_csi_fast_path<P: Perform>(
        &mut self,
        performer: &mut P,
        bytes: &[u8],
    ) -> Option<usize> {
        // Need at least ESC [ <final> = 3 bytes
        if bytes.len() < 3 {
            return None;
        }

        // bytes[0] is ESC (0x1B), check for [
        if bytes[1] != b'[' {
            return None;
        }

        // Scan for the final byte (0x40..=0x7E) starting at position 2.
        // Bail out if we see intermediates (0x20..=0x2F), private markers
        // (0x3C..=0x3F at position 2), subparameters (:), or control chars.
        let start = 2;

        // Check for private marker at first param position
        if bytes[start] >= 0x3C && bytes[start] <= 0x3F {
            // Private CSI like ESC[? — fall back to state machine
            return None;
        }

        // Parse parameters inline. We support up to MAX_PARAMS parameters
        // (same as the state machine) with digits and semicolons only.
        let mut params = Params::default();
        let mut current_param: u16 = 0;
        let mut has_param_digit = false;
        let mut i = start;

        while i < bytes.len() {
            let byte = bytes[i];
            match byte {
                // Digit: accumulate parameter
                b'0'..=b'9' => {
                    current_param = current_param.saturating_mul(10);
                    current_param = current_param.saturating_add((byte - b'0') as u16);
                    has_param_digit = true;
                }
                // Semicolon: parameter separator
                b';' => {
                    if params.is_full() {
                        return None; // Too many params, fall back
                    }
                    params.push(current_param);
                    current_param = 0;
                    has_param_digit = false;
                }
                // Colon: subparameter — fall back to state machine
                b':' => return None,
                // Intermediate bytes — fall back
                0x20..=0x2F => return None,
                // Final byte: dispatch!
                0x40..=0x7E => {
                    // Push the last parameter
                    if has_param_digit || i > start {
                        if params.is_full() {
                            // Too many params — still dispatch but with ignore flag
                            performer.csi_dispatch(&params, &[], true, byte as char);
                        } else {
                            params.push(current_param);
                            performer.csi_dispatch(&params, &[], false, byte as char);
                        }
                    } else {
                        // No parameters at all (e.g., ESC [ m)
                        params.push(0);
                        performer.csi_dispatch(&params, &[], false, byte as char);
                    }
                    self.state = State::Ground;
                    return Some(i + 1);
                }
                // Control character or anything unexpected — fall back
                _ => return None,
            }
            i += 1;
        }

        // Reached end of buffer without finding final byte — incomplete sequence.
        // Fall back to state machine which handles partial sequences.
        None
    }

    /// Handle ground dispatch of print/execute for all characters in a string.
    ///
    /// Batches runs of printable characters into `print_str()` calls, only
    /// falling back to per-character `execute()` for control codes.
    #[inline]
    fn ground_dispatch<P: Perform>(performer: &mut P, text: &str) {
        let mut printable_start: Option<usize> = None;

        for (i, c) in text.char_indices() {
            match c {
                '\x00'..='\x1f' | '\u{80}'..='\u{9f}' => {
                    // Flush any accumulated printable run before the control char.
                    if let Some(start) = printable_start.take() {
                        performer.print_str(&text[start..i]);
                    }
                    performer.execute(c as u8);
                }
                _ => {
                    if printable_start.is_none() {
                        printable_start = Some(i);
                    }
                }
            }
        }

        // Flush remaining printable run.
        if let Some(start) = printable_start {
            performer.print_str(&text[start..]);
        }
    }
}

#[derive(PartialEq, Eq, Debug, Default, Copy, Clone)]
enum State {
    CsiEntry,
    CsiIgnore,
    CsiIntermediate,
    CsiParam,
    DcsEntry,
    DcsIgnore,
    DcsIntermediate,
    DcsParam,
    DcsPassthrough,
    Escape,
    EscapeIntermediate,
    OscString,
    SosPmApcString,
    #[default]
    Ground,
}

/// Performs actions requested by the Parser
///
/// Actions in this case mean, for example, handling a CSI escape sequence
/// describing cursor movement, or simply printing characters to the screen.
///
/// The methods on this type correspond to actions described in
/// <http://vt100.net/emu/dec_ansi_parser>. I've done my best to describe them in
/// a useful way in my own words for completeness, but the site should be
/// referenced if something isn't clear. If the site disappears at some point in
/// the future, consider checking archive.org.
pub trait Perform {
    /// Draw a character to the screen and update states.
    fn print(&mut self, _c: char) {}

    /// Draw a string of printable characters to the screen (batch optimization).
    ///
    /// The provided string is guaranteed to contain only printable characters
    /// (no C0/C1 control codes). Implementations can override this for
    /// efficient batch processing. The default falls back to per-character
    /// `print()` calls.
    fn print_str(&mut self, text: &str) {
        for c in text.chars() {
            self.print(c);
        }
    }

    /// Execute a C0 or C1 control function.
    fn execute(&mut self, _byte: u8) {}

    /// Invoked when a final character arrives in first part of device control
    /// string.
    ///
    /// The control function should be determined from the private marker, final
    /// character, and execute with a parameter list. A handler should be
    /// selected for remaining characters in the string; the handler
    /// function should subsequently be called by `put` for every character in
    /// the control string.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    /// Pass bytes as part of a device control string to the handle chosen in
    /// `hook`. C0 controls will also be passed to the handler.
    fn put(&mut self, _byte: u8) {}

    /// Pass a slice of bytes as part of a device control string (batch optimization).
    ///
    /// The default falls back to per-byte `put()` calls.
    fn put_slice(&mut self, data: &[u8]) {
        for &b in data {
            self.put(b);
        }
    }

    /// Called when a device control string is terminated.
    ///
    /// The previously selected handler should be notified that the DCS has
    /// terminated.
    fn unhook(&mut self) {}

    /// Dispatch an operating system command.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    /// A final character has arrived for a CSI sequence
    ///
    /// The `ignore` flag indicates that either more than two intermediates
    /// arrived or the number of parameters exceeded the maximum supported
    /// length, and subsequent characters were ignored.
    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }

    /// The final character of an escape sequence has arrived.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}

    /// Whether the parser should terminate prematurely.
    ///
    /// This can be used in conjunction with
    /// [`Parser::advance_until_terminated`] to terminate the parser after
    /// receiving certain escape sequences like synchronized updates.
    ///
    /// This is checked after every parsed byte, so no expensive computation
    /// should take place in this function.
    #[inline(always)]
    fn terminated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::*;

    const OSC_BYTES: &[u8] = &[
        0x1B, 0x5D, // Begin OSC
        b'2', b';', b'j', b'w', b'i', b'l', b'm', b'@', b'j', b'w', b'i', b'l', b'm', b'-', b'd',
        b'e', b's', b'k', b':', b' ', b'~', b'/', b'c', b'o', b'd', b'e', b'/', b'a', b'l', b'a',
        b'c', b'r', b'i', b't', b't', b'y', 0x07, // End OSC
    ];

    #[derive(Default)]
    struct Dispatcher {
        dispatched: Vec<Sequence>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Sequence {
        Osc(Vec<Vec<u8>>, bool),
        Csi(Vec<Vec<u16>>, Vec<u8>, bool, char),
        Esc(Vec<u8>, bool, u8),
        DcsHook(Vec<Vec<u16>>, Vec<u8>, bool, char),
        DcsPut(u8),
        Print(char),
        Execute(u8),
        DcsUnhook,
    }

    impl Perform for Dispatcher {
        fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
            let params = params.iter().map(|p| p.to_vec()).collect();
            self.dispatched.push(Sequence::Osc(params, bell_terminated));
        }

        fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
            let params = params.iter().map(|subparam| subparam.to_vec()).collect();
            let intermediates = intermediates.to_vec();
            self.dispatched.push(Sequence::Csi(params, intermediates, ignore, c));
        }

        fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
            let intermediates = intermediates.to_vec();
            self.dispatched.push(Sequence::Esc(intermediates, ignore, byte));
        }

        fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
            let params = params.iter().map(|subparam| subparam.to_vec()).collect();
            let intermediates = intermediates.to_vec();
            self.dispatched.push(Sequence::DcsHook(params, intermediates, ignore, c));
        }

        fn put(&mut self, byte: u8) {
            self.dispatched.push(Sequence::DcsPut(byte));
        }

        fn unhook(&mut self) {
            self.dispatched.push(Sequence::DcsUnhook);
        }

        fn print(&mut self, c: char) {
            self.dispatched.push(Sequence::Print(c));
        }

        fn execute(&mut self, byte: u8) {
            self.dispatched.push(Sequence::Execute(byte));
        }
    }

    #[test]
    fn parse_osc() {
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, OSC_BYTES);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(params, _) => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], &OSC_BYTES[2..3]);
                assert_eq!(params[1], &OSC_BYTES[4..(OSC_BYTES.len() - 1)]);
            },
            _ => panic!("expected osc sequence"),
        }
    }

    #[test]
    fn parse_empty_osc() {
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &[0x1B, 0x5D, 0x07]);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(..) => (),
            _ => panic!("expected osc sequence"),
        }
    }

    #[test]
    fn parse_osc_max_params() {
        let params = ";".repeat(params::MAX_PARAMS + 1);
        let input = format!("\x1b]{}\x1b", &params[..]).into_bytes();
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &input);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(params, _) => {
                assert_eq!(params.len(), MAX_OSC_PARAMS);
                assert!(params.iter().all(Vec::is_empty));
            },
            _ => panic!("expected osc sequence"),
        }
    }

    #[test]
    fn osc_bell_terminated() {
        const INPUT: &[u8] = b"\x1b]11;ff/00/ff\x07";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(_, true) => (),
            _ => panic!("expected osc with bell terminator"),
        }
    }

    #[test]
    fn osc_c0_st_terminated() {
        const INPUT: &[u8] = b"\x1b]11;ff/00/ff\x1b\\";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 2);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(_, false) => (),
            _ => panic!("expected osc with ST terminator"),
        }
    }

    #[test]
    fn parse_osc_with_utf8_arguments() {
        const INPUT: &[u8] = &[
            0x0D, 0x1B, 0x5D, 0x32, 0x3B, 0x65, 0x63, 0x68, 0x6F, 0x20, 0x27, 0xC2, 0xAF, 0x5C,
            0x5F, 0x28, 0xE3, 0x83, 0x84, 0x29, 0x5F, 0x2F, 0xC2, 0xAF, 0x27, 0x20, 0x26, 0x26,
            0x20, 0x73, 0x6C, 0x65, 0x65, 0x70, 0x20, 0x31, 0x07,
        ];
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched[0], Sequence::Execute(b'\r'));
        let osc_data = INPUT[5..(INPUT.len() - 1)].into();
        assert_eq!(dispatcher.dispatched[1], Sequence::Osc(vec![vec![b'2'], osc_data], true));
        assert_eq!(dispatcher.dispatched.len(), 2);
    }

    #[test]
    fn osc_containing_string_terminator() {
        const INPUT: &[u8] = b"\x1b]2;\xe6\x9c\xab\x1b\\";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 2);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(params, _) => {
                assert_eq!(params[1], &INPUT[4..(INPUT.len() - 2)]);
            },
            _ => panic!("expected osc sequence"),
        }
    }

    #[test]
    fn exceed_max_buffer_size() {
        const NUM_BYTES: usize = MAX_OSC_RAW + 100;
        const INPUT_START: &[u8] = b"\x1b]52;s";
        const INPUT_END: &[u8] = b"\x07";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        // Create valid OSC escape
        parser.advance(&mut dispatcher, INPUT_START);

        // Exceed max buffer size
        parser.advance(&mut dispatcher, &[b'a'; NUM_BYTES]);

        // Terminate escape for dispatch
        parser.advance(&mut dispatcher, INPUT_END);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Osc(params, _) => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], b"52");

                assert_eq!(params[1].len(), NUM_BYTES + INPUT_END.len());
            },
            _ => panic!("expected osc sequence"),
        }
    }

    #[test]
    fn parse_csi_max_params() {
        // This will build a list of repeating '1;'s
        // The length is MAX_PARAMS - 1 because the last semicolon is interpreted
        // as an implicit zero, making the total number of parameters MAX_PARAMS
        let params = "1;".repeat(params::MAX_PARAMS - 1);
        let input = format!("\x1b[{}p", &params[..]).into_bytes();

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &input);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, _, ignore, _) => {
                assert_eq!(params.len(), params::MAX_PARAMS);
                assert!(!ignore);
            },
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn parse_csi_params_ignore_long_params() {
        // This will build a list of repeating '1;'s
        // The length is MAX_PARAMS because the last semicolon is interpreted
        // as an implicit zero, making the total number of parameters MAX_PARAMS + 1
        let params = "1;".repeat(params::MAX_PARAMS);
        let input = format!("\x1b[{}p", &params[..]).into_bytes();

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &input);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, _, ignore, _) => {
                assert_eq!(params.len(), params::MAX_PARAMS);
                assert!(ignore);
            },
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn parse_csi_params_trailing_semicolon() {
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, b"\x1b[4;m");

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, ..) => assert_eq!(params, &[[4], [0]]),
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn parse_csi_params_leading_semicolon() {
        // Create dispatcher and check state
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, b"\x1b[;4m");

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, ..) => assert_eq!(params, &[[0], [4]]),
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn parse_long_csi_param() {
        // The important part is the parameter, which is (i64::MAX + 1)
        const INPUT: &[u8] = b"\x1b[9223372036854775808m";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, ..) => assert_eq!(params, &[[u16::MAX]]),
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn csi_reset() {
        const INPUT: &[u8] = b"\x1b[3;1\x1b[?1049h";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, intermediates, ignore, _) => {
                assert_eq!(intermediates, b"?");
                assert_eq!(params, &[[1049]]);
                assert!(!ignore);
            },
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn csi_subparameters() {
        const INPUT: &[u8] = b"\x1b[38:2:255:0:255;1m";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, intermediates, ignore, _) => {
                assert_eq!(params, &[vec![38, 2, 255, 0, 255], vec![1]]);
                assert_eq!(intermediates, &[]);
                assert!(!ignore);
            },
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn parse_dcs_max_params() {
        let params = "1;".repeat(params::MAX_PARAMS + 1);
        let input = format!("\x1bP{}p", &params[..]).into_bytes();
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &input);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::DcsHook(params, _, ignore, _) => {
                assert_eq!(params.len(), params::MAX_PARAMS);
                assert!(params.iter().all(|param| param == &[1]));
                assert!(ignore);
            },
            _ => panic!("expected dcs sequence"),
        }
    }

    #[test]
    fn dcs_reset() {
        const INPUT: &[u8] = b"\x1b[3;1\x1bP1$tx\x9c";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 3);
        match &dispatcher.dispatched[0] {
            Sequence::DcsHook(params, intermediates, ignore, _) => {
                assert_eq!(intermediates, b"$");
                assert_eq!(params, &[[1]]);
                assert!(!ignore);
            },
            _ => panic!("expected dcs sequence"),
        }
        assert_eq!(dispatcher.dispatched[1], Sequence::DcsPut(b'x'));
        assert_eq!(dispatcher.dispatched[2], Sequence::DcsUnhook);
    }

    #[test]
    fn parse_dcs() {
        const INPUT: &[u8] = b"\x1bP0;1|17/ab\x9c";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 7);
        match &dispatcher.dispatched[0] {
            Sequence::DcsHook(params, _, _, c) => {
                assert_eq!(params, &[[0], [1]]);
                assert_eq!(c, &'|');
            },
            _ => panic!("expected dcs sequence"),
        }
        for (i, byte) in b"17/ab".iter().enumerate() {
            assert_eq!(dispatcher.dispatched[1 + i], Sequence::DcsPut(*byte));
        }
        assert_eq!(dispatcher.dispatched[6], Sequence::DcsUnhook);
    }

    #[test]
    fn intermediate_reset_on_dcs_exit() {
        const INPUT: &[u8] = b"\x1bP=1sZZZ\x1b+\x5c";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 6);
        match &dispatcher.dispatched[5] {
            Sequence::Esc(intermediates, ..) => assert_eq!(intermediates, b"+"),
            _ => panic!("expected esc sequence"),
        }
    }

    #[test]
    fn esc_reset() {
        const INPUT: &[u8] = b"\x1b[3;1\x1b(A";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Esc(intermediates, ignore, byte) => {
                assert_eq!(intermediates, b"(");
                assert_eq!(*byte, b'A');
                assert!(!ignore);
            },
            _ => panic!("expected esc sequence"),
        }
    }

    #[test]
    fn esc_reset_intermediates() {
        const INPUT: &[u8] = b"\x1b[?2004l\x1b#8";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 2);
        assert_eq!(dispatcher.dispatched[0], Sequence::Csi(vec![vec![2004]], vec![63], false, 'l'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Esc(vec![35], false, 56));
    }

    #[test]
    fn params_buffer_filled_with_subparam() {
        const INPUT: &[u8] = b"\x1b[::::::::::::::::::::::::::::::::x\x1b";
        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 1);
        match &dispatcher.dispatched[0] {
            Sequence::Csi(params, intermediates, ignore, c) => {
                assert_eq!(intermediates, &[]);
                assert_eq!(params, &[[0; 32]]);
                assert_eq!(c, &'x');
                assert!(ignore);
            },
            _ => panic!("expected csi sequence"),
        }
    }

    #[test]
    fn unicode() {
        const INPUT: &[u8] = b"\xF0\x9F\x8E\x89_\xF0\x9F\xA6\x80\xF0\x9F\xA6\x80_\xF0\x9F\x8E\x89";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 6);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('🎉'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Print('_'));
        assert_eq!(dispatcher.dispatched[2], Sequence::Print('🦀'));
        assert_eq!(dispatcher.dispatched[3], Sequence::Print('🦀'));
        assert_eq!(dispatcher.dispatched[4], Sequence::Print('_'));
        assert_eq!(dispatcher.dispatched[5], Sequence::Print('🎉'));
    }

    #[test]
    fn invalid_utf8() {
        const INPUT: &[u8] = b"a\xEF\xBCb";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 3);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('a'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Print('�'));
        assert_eq!(dispatcher.dispatched[2], Sequence::Print('b'));
    }

    #[test]
    fn partial_utf8() {
        const INPUT: &[u8] = b"\xF0\x9F\x9A\x80";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &INPUT[..1]);
        parser.advance(&mut dispatcher, &INPUT[1..2]);
        parser.advance(&mut dispatcher, &INPUT[2..3]);
        parser.advance(&mut dispatcher, &INPUT[3..]);

        assert_eq!(dispatcher.dispatched.len(), 1);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('🚀'));
    }

    #[test]
    fn partial_utf8_separating_utf8() {
        // This is different from the `partial_utf8` test since it has a multi-byte UTF8
        // character after the partial UTF8 state, causing a partial byte to be present
        // in the `partial_utf8` buffer after the 2-byte codepoint.

        // "ĸ🎉"
        const INPUT: &[u8] = b"\xC4\xB8\xF0\x9F\x8E\x89";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &INPUT[..1]);
        parser.advance(&mut dispatcher, &INPUT[1..]);

        assert_eq!(dispatcher.dispatched.len(), 2);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('ĸ'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Print('🎉'));
    }

    #[test]
    fn partial_invalid_utf8() {
        const INPUT: &[u8] = b"a\xEF\xBCb";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &INPUT[..1]);
        parser.advance(&mut dispatcher, &INPUT[1..2]);
        parser.advance(&mut dispatcher, &INPUT[2..3]);
        parser.advance(&mut dispatcher, &INPUT[3..]);

        assert_eq!(dispatcher.dispatched.len(), 3);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('a'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Print('�'));
        assert_eq!(dispatcher.dispatched[2], Sequence::Print('b'));
    }

    #[test]
    fn partial_invalid_utf8_split() {
        const INPUT: &[u8] = b"\xE4\xBF\x99\xB5";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, &INPUT[..2]);
        parser.advance(&mut dispatcher, &INPUT[2..]);

        assert_eq!(dispatcher.dispatched[0], Sequence::Print('俙'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Print('�'));
    }

    #[test]
    fn partial_utf8_into_esc() {
        const INPUT: &[u8] = b"\xD8\x1b012";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 4);
        assert_eq!(dispatcher.dispatched[0], Sequence::Print('�'));
        assert_eq!(dispatcher.dispatched[1], Sequence::Esc(Vec::new(), false, b'0'));
        assert_eq!(dispatcher.dispatched[2], Sequence::Print('1'));
        assert_eq!(dispatcher.dispatched[3], Sequence::Print('2'));
    }

    #[test]
    fn c1s() {
        const INPUT: &[u8] = b"\x00\x1f\x80\x90\x98\x9b\x9c\x9d\x9e\x9fa";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 11);
        assert_eq!(dispatcher.dispatched[0], Sequence::Execute(0));
        assert_eq!(dispatcher.dispatched[1], Sequence::Execute(31));
        assert_eq!(dispatcher.dispatched[2], Sequence::Execute(128));
        assert_eq!(dispatcher.dispatched[3], Sequence::Execute(144));
        assert_eq!(dispatcher.dispatched[4], Sequence::Execute(152));
        assert_eq!(dispatcher.dispatched[5], Sequence::Execute(155));
        assert_eq!(dispatcher.dispatched[6], Sequence::Execute(156));
        assert_eq!(dispatcher.dispatched[7], Sequence::Execute(157));
        assert_eq!(dispatcher.dispatched[8], Sequence::Execute(158));
        assert_eq!(dispatcher.dispatched[9], Sequence::Execute(159));
        assert_eq!(dispatcher.dispatched[10], Sequence::Print('a'));
    }

    #[test]
    fn execute_anywhere() {
        const INPUT: &[u8] = b"\x18\x1a";

        let mut dispatcher = Dispatcher::default();
        let mut parser = Parser::new();

        parser.advance(&mut dispatcher, INPUT);

        assert_eq!(dispatcher.dispatched.len(), 2);
        assert_eq!(dispatcher.dispatched[0], Sequence::Execute(0x18));
        assert_eq!(dispatcher.dispatched[1], Sequence::Execute(0x1A));
    }
}
