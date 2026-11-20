//! A filter over the bytes a program writes, ahead of the terminal parser.
//!
//! The parser keeps every byte of an open string sequence and grows one cell
//! for every combining mark it is given, so a program that never ends a string
//! or never stops sending marks costs the host memory or time without bound.
//! The guard bounds both, in a streaming way: its state survives across
//! `feed` calls, and a sequence split over several calls is judged as a whole.
//! It also keeps three kinds of character out of every cell: DEL, which the
//! renderer cannot draw, the bidirectional controls, which reorder the text
//! around them, and the tag characters, which are invisible. Bytes that are not
//! valid UTF-8 never pass on as they are: each broken sequence becomes one
//! U+FFFD, so a neighbour the guard drops cannot join the bytes around it into
//! a character the guard never saw.
//!
//! A combining mark joins the cell left of the cursor, so the guard lets one
//! through only while it is sure which cell that is: after a base character it
//! has seen print, and until anything that could move the cursor or change what
//! prints. Colour changes (SGR) are the one sequence that keeps the run.

use unicode_width::UnicodeWidthChar;

/// The most bytes of one string sequence (OSC, DCS, APC, PM, SOS) that is
/// passed on. A 100,000 byte clipboard write is 133,336 bytes of base64, and
/// the rest is room for the sequence's own prefix.
const MAX_STRING_BYTES: usize = 144 * 1024;

/// The most characters that may join the cell before them: combining marks,
/// variation selectors, joiners, and the characters a joiner pulls in.
const MAX_JOINERS: u16 = 16;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;
const ST: u8 = 0x9c;
const ZWJ: char = '\u{200d}';
const REPLACEMENT: &[u8] = "\u{fffd}".as_bytes();
const MAX_DECSET_PARAMS: u8 = 16;

pub fn is_spoofing_format(c: char) -> bool {
    matches!(
        c,
        '\u{061c}'
            | '\u{200e}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{e0000}'..='\u{e007f}'
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    /// After an `ESC` that is not yet passed on: it goes with the byte that
    /// says what it starts.
    Esc,
    EscIntermediate,
    Csi,
    /// Inside a string sequence. `Held` keeps its bytes until it ends, so a
    /// sequence that grows past the cap is dropped whole and the parser never
    /// sees a prefix of it. `Dropping` discards bytes until the end.
    Held {
        osc: bool,
        esc: bool,
    },
    Dropping {
        osc: bool,
        esc: bool,
    },
}

#[derive(Debug)]
struct Csi {
    started: bool,
    marker: Option<u8>,
    intermediate: bool,
    bang: bool,
    plain: bool,
    params: u8,
    param: u32,
    seven: bool,
    screen: bool,
}

impl Csi {
    fn new() -> Self {
        Self {
            started: false,
            marker: None,
            intermediate: false,
            bang: false,
            plain: true,
            params: 0,
            param: 0,
            seven: false,
            screen: false,
        }
    }

    fn push(&mut self, byte: u8) {
        let first = !self.started;
        self.started = true;
        match byte {
            0x3c..=0x3f if first => self.marker = Some(byte),
            b'0'..=b'9' => {
                self.param = self
                    .param
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
            }
            b';' => self.end_param(),
            b':' => {
                self.end_param();
                self.plain = false;
            }
            0x20..=0x2f => {
                self.bang = byte == b'!' && !self.intermediate;
                self.intermediate = true;
                self.plain = false;
            }
            _ => self.plain = false,
        }
    }

    fn end_param(&mut self) {
        self.seven |= self.param == 7;
        self.screen |= matches!(self.param, 47 | 1047 | 1049);
        self.param = 0;
        self.params = self.params.saturating_add(1);
    }

    fn is_sgr(&self, fin: u8) -> bool {
        fin == b'm' && self.marker.is_none() && !self.intermediate
    }

    fn is_private_mode(&self, fin: u8) -> bool {
        matches!(fin, b'h' | b'l') && self.marker == Some(b'?') && !self.intermediate
    }

    fn is_soft_reset(&self, fin: u8) -> bool {
        fin == b'p' && self.marker.is_none() && self.bang
    }

    fn certain(&self) -> bool {
        self.plain && self.params <= MAX_DECSET_PARAMS
    }
}

#[derive(Debug)]
pub(super) struct FeedGuard {
    state: State,
    held: Vec<u8>,
    /// The bytes of a character that a feed ended in the middle of.
    partial: Vec<u8>,
    csi: Csi,
    attached: bool,
    no_wrap: bool,
    joiners: u16,
    after_joiner: bool,
    out: Vec<u8>,
    switches: Vec<usize>,
}

pub(super) struct Filtered<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) switches: &'a [usize],
}

impl FeedGuard {
    pub(super) fn new() -> Self {
        Self {
            state: State::Ground,
            held: Vec::new(),
            partial: Vec::new(),
            csi: Csi::new(),
            attached: false,
            no_wrap: false,
            joiners: 0,
            after_joiner: false,
            out: Vec::new(),
            switches: Vec::new(),
        }
    }

    /// The bytes to give the parser for `bytes`.
    pub(super) fn filter(&mut self, bytes: &[u8]) -> Filtered<'_> {
        self.out.clear();
        self.switches.clear();
        for &byte in bytes {
            self.step(byte);
        }
        Filtered {
            bytes: &self.out,
            switches: &self.switches,
        }
    }

    fn step(&mut self, byte: u8) {
        loop {
            match self.state {
                State::Ground => return self.ground(byte),
                State::Esc => {
                    if self.escape(byte) {
                        return;
                    }
                }
                State::EscIntermediate => return self.escape_intermediate(byte),
                State::Csi => return self.csi(byte),
                State::Held { osc, esc } => {
                    if self.held_string(byte, osc, esc) {
                        return;
                    }
                }
                State::Dropping { osc, esc } => {
                    if self.dropping(byte, osc, esc) {
                        return;
                    }
                }
            }
        }
    }

    /// Whether `byte` is used up. `false` means the state changed and the byte
    /// is to be read again in the new one.
    fn escape(&mut self, byte: u8) -> bool {
        match byte {
            b']' | b'P' | b'_' | b'^' | b'X' => {
                self.detach();
                self.held.clear();
                self.held.extend_from_slice(&[ESC, byte]);
                self.state = State::Held {
                    osc: byte == b']',
                    esc: false,
                };
            }
            b'[' => {
                self.out.extend_from_slice(&[ESC, byte]);
                self.csi = Csi::new();
                self.state = State::Csi;
            }
            ESC => self.out.push(ESC),
            0x7f..=0xff => {}
            0x00..=0x17 | 0x19 | 0x1c..=0x1f => {
                self.detach();
                self.out.push(byte);
            }
            0x20..=0x2f => {
                self.detach();
                self.out.extend_from_slice(&[ESC, byte]);
                self.state = State::EscIntermediate;
            }
            _ => {
                self.detach();
                self.out.extend_from_slice(&[ESC, byte]);
                if byte == b'c' {
                    self.no_wrap = false;
                    self.switches.push(self.out.len());
                }
                self.state = State::Ground;
            }
        }
        true
    }

    fn escape_intermediate(&mut self, byte: u8) {
        match byte {
            ESC => self.state = State::Esc,
            0x20..=0x2f => self.out.push(byte),
            0x30..=0x7e | CAN | SUB => {
                self.out.push(byte);
                self.state = State::Ground;
            }
            0x00..=0x1f => {
                self.detach();
                self.out.push(byte);
            }
            _ => {}
        }
    }

    fn csi(&mut self, byte: u8) {
        self.out.push(byte);
        match byte {
            0x40..=0x7e => {
                self.csi.end_param();
                self.csi_final(byte);
                self.state = State::Ground;
            }
            CAN | SUB => self.state = State::Ground,
            ESC => self.state = State::Esc,
            0x00..=0x1f => {
                self.detach();
                self.csi.plain = false;
            }
            _ => self.csi.push(byte),
        }
    }

    fn csi_final(&mut self, fin: u8) {
        if self.csi.is_sgr(fin) {
            return;
        }
        self.detach();
        if self.csi.is_soft_reset(fin) {
            self.no_wrap = false;
        }
        if self.csi.is_private_mode(fin) && self.csi.screen {
            self.switches.push(self.out.len());
        }
        if self.csi.is_private_mode(fin) && self.csi.seven {
            if fin == b'l' {
                self.no_wrap = true;
            } else if self.csi.certain() {
                self.no_wrap = false;
            }
        }
    }

    fn held_string(&mut self, byte: u8, osc: bool, esc: bool) -> bool {
        if esc {
            if byte == b'\\' {
                self.held.push(byte);
                self.release();
                return true;
            }
            self.held.pop();
            self.release();
            self.state = State::Esc;
            return false;
        }
        match byte {
            BEL if osc => {
                self.held.push(byte);
                self.release();
            }
            CAN | SUB => {
                self.held.push(byte);
                self.release();
            }
            ST if !osc => {}
            ESC => {
                self.held.push(byte);
                self.state = State::Held { osc, esc: true };
            }
            _ => {
                if self.held.len() >= MAX_STRING_BYTES {
                    self.held.clear();
                    self.held.shrink_to(0);
                    self.state = State::Dropping { osc, esc: false };
                } else {
                    self.held.push(byte);
                }
            }
        }
        true
    }

    fn dropping(&mut self, byte: u8, osc: bool, esc: bool) -> bool {
        if esc {
            if byte == b'\\' {
                self.state = State::Ground;
                return true;
            }
            self.state = State::Esc;
            return false;
        }
        match byte {
            BEL if osc => self.state = State::Ground,
            CAN | SUB => self.state = State::Ground,
            ESC => self.state = State::Dropping { osc, esc: true },
            _ => {}
        }
        true
    }

    fn release(&mut self) {
        self.out.append(&mut self.held);
        self.state = State::Ground;
    }

    fn ground(&mut self, byte: u8) {
        if !self.partial.is_empty() || byte >= 0x80 {
            return self.multibyte(byte);
        }
        match byte {
            ESC => self.state = State::Esc,
            0x7f => {}
            0x20..=0x7e => {
                self.base();
                self.out.push(byte);
            }
            _ => {
                self.detach();
                self.out.push(byte);
            }
        }
    }

    fn base(&mut self) {
        self.attached = true;
        self.joiners = 0;
        self.after_joiner = false;
    }

    fn detach(&mut self) {
        self.attached = false;
        self.after_joiner = false;
    }

    fn multibyte(&mut self, byte: u8) {
        let want = match self.partial.first().copied().unwrap_or(byte) {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        let continues = (0x80..=0xbf).contains(&byte) && !self.partial.is_empty();
        if !self.partial.is_empty() && !continues {
            self.partial.clear();
            self.replace();
            return self.ground(byte);
        }
        self.partial.push(byte);
        if self.partial.len() < want {
            return;
        }
        let bytes = std::mem::take(&mut self.partial);
        match std::str::from_utf8(&bytes)
            .ok()
            .and_then(|s| s.chars().next())
        {
            Some(c) => self.character(c, &bytes),
            None => self.replace(),
        }
    }

    fn replace(&mut self) {
        self.detach();
        self.out.extend_from_slice(REPLACEMENT);
    }

    fn character(&mut self, c: char, bytes: &[u8]) {
        if is_spoofing_format(c) {
            return;
        }
        if matches!(c, '\u{80}'..='\u{9f}') {
            self.detach();
            self.out.extend_from_slice(bytes);
            return;
        }
        let width = c.width();
        if self.after_joiner || width == Some(0) {
            if !self.attached || self.joiners >= MAX_JOINERS {
                return;
            }
            self.joiners += 1;
            self.after_joiner = c == ZWJ;
        } else if !(self.no_wrap && width == Some(2)) {
            self.base();
        }
        self.out.extend_from_slice(bytes);
    }
}
