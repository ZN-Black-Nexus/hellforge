//! Keyboard, mouse and the virtual game buttons.
//!
//! Most games only need [`Button`]s: each one listens to several keys, so a
//! game works with arrows or WASD and whatever action key the player tries.
//! Raw keys ([`Key`]) and the mouse ([`Mouse`]) are there when you need them.

/// A virtual game button.
///
/// | Button | Keys |
/// |---|---|
/// | `Up` `Down` `Left` `Right` | arrow keys, W S A D |
/// | `A` | Z, Space, J |
/// | `B` | X, K |
/// | `X` | C, L |
/// | `Y` | V, I |
/// | `Start` | Enter, P |
/// | `Select` | Tab, M |
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    Start,
    Select,
}

impl Button {
    pub const ALL: [Button; 10] =
        [Button::Up, Button::Down, Button::Left, Button::Right, Button::A, Button::B, Button::X, Button::Y, Button::Start, Button::Select];

    /// The keys that press this button.
    pub const fn keys(self) -> &'static [Key] {
        match self {
            Button::Up => &[Key::UP, Key::W],
            Button::Down => &[Key::DOWN, Key::S],
            Button::Left => &[Key::LEFT, Key::A],
            Button::Right => &[Key::RIGHT, Key::D],
            Button::A => &[Key::Z, Key::SPACE, Key::J],
            Button::B => &[Key::X, Key::K],
            Button::X => &[Key::C, Key::L],
            Button::Y => &[Key::V, Key::I],
            Button::Start => &[Key::ENTER, Key::P],
            Button::Select => &[Key::TAB, Key::M],
        }
    }

    /// Lowercase name, as used in `--input` scripts.
    pub const fn name(self) -> &'static str {
        match self {
            Button::Up => "up",
            Button::Down => "down",
            Button::Left => "left",
            Button::Right => "right",
            Button::A => "a",
            Button::B => "b",
            Button::X => "x",
            Button::Y => "y",
            Button::Start => "start",
            Button::Select => "select",
        }
    }

    pub fn from_name(n: &str) -> Option<Button> {
        Button::ALL.iter().copied().find(|b| b.name() == n)
    }
}

/// A keyboard key. Letters, digits and punctuation are their lowercase ASCII
/// code (`Key::char('q')`, `Key::Q`); other keys have constants.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Key(pub u16);

impl Key {
    pub const BACKSPACE: Key = Key(8);
    pub const TAB: Key = Key(9);
    pub const ENTER: Key = Key(13);
    pub const ESCAPE: Key = Key(27);
    pub const SPACE: Key = Key(32);
    pub const UP: Key = Key(0x100);
    pub const DOWN: Key = Key(0x101);
    pub const LEFT: Key = Key(0x102);
    pub const RIGHT: Key = Key(0x103);
    pub const SHIFT: Key = Key(0x104);
    pub const CTRL: Key = Key(0x105);
    pub const ALT: Key = Key(0x106);
    pub const F1: Key = Key(0x110);
    pub const F2: Key = Key(0x111);
    pub const F3: Key = Key(0x112);
    pub const F4: Key = Key(0x113);
    pub const F5: Key = Key(0x114);
    pub const F6: Key = Key(0x115);
    pub const F7: Key = Key(0x116);
    pub const F8: Key = Key(0x117);
    pub const F9: Key = Key(0x118);
    pub const F10: Key = Key(0x119);
    pub const F11: Key = Key(0x11a);
    pub const F12: Key = Key(0x11b);
    pub const PAGE_UP: Key = Key(0x120);
    pub const PAGE_DOWN: Key = Key(0x121);
    pub const HOME: Key = Key(0x122);
    pub const END: Key = Key(0x123);
    pub const INSERT: Key = Key(0x124);
    pub const DELETE: Key = Key(0x125);
    pub const PAUSE: Key = Key(0x126);

    pub const A: Key = Key(b'a' as u16);
    pub const B: Key = Key(b'b' as u16);
    pub const C: Key = Key(b'c' as u16);
    pub const D: Key = Key(b'd' as u16);
    pub const E: Key = Key(b'e' as u16);
    pub const F: Key = Key(b'f' as u16);
    pub const G: Key = Key(b'g' as u16);
    pub const H: Key = Key(b'h' as u16);
    pub const I: Key = Key(b'i' as u16);
    pub const J: Key = Key(b'j' as u16);
    pub const K: Key = Key(b'k' as u16);
    pub const L: Key = Key(b'l' as u16);
    pub const M: Key = Key(b'm' as u16);
    pub const N: Key = Key(b'n' as u16);
    pub const O: Key = Key(b'o' as u16);
    pub const P: Key = Key(b'p' as u16);
    pub const Q: Key = Key(b'q' as u16);
    pub const R: Key = Key(b'r' as u16);
    pub const S: Key = Key(b's' as u16);
    pub const T: Key = Key(b't' as u16);
    pub const U: Key = Key(b'u' as u16);
    pub const V: Key = Key(b'v' as u16);
    pub const W: Key = Key(b'w' as u16);
    pub const X: Key = Key(b'x' as u16);
    pub const Y: Key = Key(b'y' as u16);
    pub const Z: Key = Key(b'z' as u16);
    pub const DIGIT0: Key = Key(b'0' as u16);
    pub const DIGIT1: Key = Key(b'1' as u16);
    pub const DIGIT2: Key = Key(b'2' as u16);
    pub const DIGIT3: Key = Key(b'3' as u16);
    pub const DIGIT4: Key = Key(b'4' as u16);
    pub const DIGIT5: Key = Key(b'5' as u16);
    pub const DIGIT6: Key = Key(b'6' as u16);
    pub const DIGIT7: Key = Key(b'7' as u16);
    pub const DIGIT8: Key = Key(b'8' as u16);
    pub const DIGIT9: Key = Key(b'9' as u16);

    /// The key that types `c` (letters are case-insensitive).
    pub const fn char(c: char) -> Key {
        let v = c as u32;
        if v >= 'A' as u32 && v <= 'Z' as u32 { Key((v + 32) as u16) } else if v < 128 { Key(v as u16) } else { Key(0) }
    }

    /// Looks a key up by name: a single character, or `space enter esc tab
    /// backspace shift ctrl alt up down left right f1..f12 pageup pagedown
    /// home end insert delete pause`.
    pub fn from_name(n: &str) -> Option<Key> {
        let b = n.as_bytes();
        if b.len() == 1 && b[0].is_ascii_graphic() {
            return Some(Key::char(b[0] as char));
        }
        Some(match n {
            "space" => Key::SPACE,
            "enter" | "return" => Key::ENTER,
            "esc" | "escape" => Key::ESCAPE,
            "tab" => Key::TAB,
            "backspace" => Key::BACKSPACE,
            "shift" => Key::SHIFT,
            "ctrl" => Key::CTRL,
            "alt" => Key::ALT,
            "up" => Key::UP,
            "down" => Key::DOWN,
            "left" => Key::LEFT,
            "right" => Key::RIGHT,
            "pageup" => Key::PAGE_UP,
            "pagedown" => Key::PAGE_DOWN,
            "home" => Key::HOME,
            "end" => Key::END,
            "insert" => Key::INSERT,
            "delete" => Key::DELETE,
            "pause" => Key::PAUSE,
            _ => {
                let rest = n.strip_prefix('f')?;
                let num: u16 = rest.parse().ok()?;
                if !(1..=12).contains(&num) {
                    return None;
                }
                Key(Key::F1.0 + num - 1)
            }
        })
    }
}

/// Number of distinct key codes.
pub(crate) const KEY_COUNT: usize = 0x140;
const WORDS: usize = KEY_COUNT / 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Mouse state for the current update. Positions are in screen pixels (the
/// same space you draw in) and can be outside the screen.
#[derive(Clone, Copy, Default, Debug)]
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    /// Movement since the last update (also while the mouse is captured).
    pub dx: i32,
    pub dy: i32,
    /// Wheel clicks since the last update: positive = up.
    pub wheel: i32,
}

/// Input as the platform reports it, turned into per-update snapshots.
pub(crate) struct Input {
    held: [u64; WORDS],
    hit: [u64; WORDS],
    now: [u64; WORDS],
    prev: [u64; WORDS],
    mouse_held: u8,
    mouse_hit: u8,
    mouse_now: u8,
    mouse_prev: u8,
    pub mouse: Mouse,
    pos: (i32, i32),
    acc: (i32, i32, i32),
}

impl Input {
    pub const fn new() -> Input {
        Input {
            held: [0; WORDS],
            hit: [0; WORDS],
            now: [0; WORDS],
            prev: [0; WORDS],
            mouse_held: 0,
            mouse_hit: 0,
            mouse_now: 0,
            mouse_prev: 0,
            mouse: Mouse { x: 0, y: 0, dx: 0, dy: 0, wheel: 0 },
            pos: (0, 0),
            acc: (0, 0, 0),
        }
    }

    pub fn key(&mut self, k: Key, down: bool) {
        let i = k.0 as usize;
        if i >= KEY_COUNT {
            return;
        }
        let (w, bit) = (i / 64, 1u64 << (i % 64));
        if down {
            self.held[w] |= bit;
            self.hit[w] |= bit;
        } else {
            self.held[w] &= !bit;
        }
    }

    pub fn mouse_button(&mut self, b: MouseButton, down: bool) {
        let bit = 1u8 << b as u8;
        if down {
            self.mouse_held |= bit;
            self.mouse_hit |= bit;
        } else {
            self.mouse_held &= !bit;
        }
    }

    pub fn mouse_pos(&mut self, x: i32, y: i32) {
        self.acc.0 += x - self.pos.0;
        self.acc.1 += y - self.pos.1;
        self.pos = (x, y);
    }

    pub fn mouse_motion(&mut self, dx: i32, dy: i32) {
        self.acc.0 += dx;
        self.acc.1 += dy;
    }

    pub fn mouse_wheel(&mut self, dy: i32) {
        self.acc.2 += dy;
    }

    pub fn release_all(&mut self) {
        self.held = [0; WORDS];
        self.mouse_held = 0;
    }

    /// Take the snapshot one update will see. A key tapped and released
    /// between two updates still counts as pressed for one update.
    pub fn begin_update(&mut self) {
        self.prev = self.now;
        for i in 0..WORDS {
            self.now[i] = self.held[i] | self.hit[i];
        }
        self.hit = [0; WORDS];
        self.mouse_prev = self.mouse_now;
        self.mouse_now = self.mouse_held | self.mouse_hit;
        self.mouse_hit = 0;
        self.mouse = Mouse { x: self.pos.0, y: self.pos.1, dx: self.acc.0, dy: self.acc.1, wheel: self.acc.2 };
        self.acc = (0, 0, 0);
    }

    fn bit(set: &[u64; WORDS], k: Key) -> bool {
        let i = k.0 as usize;
        i < KEY_COUNT && set[i / 64] & (1 << (i % 64)) != 0
    }

    pub fn key_held(&self, k: Key) -> bool {
        Self::bit(&self.now, k)
    }
    pub fn key_pressed(&self, k: Key) -> bool {
        Self::bit(&self.now, k) && !Self::bit(&self.prev, k)
    }
    pub fn key_released(&self, k: Key) -> bool {
        !Self::bit(&self.now, k) && Self::bit(&self.prev, k)
    }

    fn button_in(&self, set: &[u64; WORDS], b: Button) -> bool {
        b.keys().iter().any(|&k| Self::bit(set, k))
    }
    pub fn held(&self, b: Button) -> bool {
        self.button_in(&self.now, b)
    }
    pub fn pressed(&self, b: Button) -> bool {
        self.button_in(&self.now, b) && !self.button_in(&self.prev, b)
    }
    pub fn released(&self, b: Button) -> bool {
        !self.button_in(&self.now, b) && self.button_in(&self.prev, b)
    }

    pub fn mouse_held(&self, b: MouseButton) -> bool {
        self.mouse_now & (1 << b as u8) != 0
    }
    pub fn mouse_pressed(&self, b: MouseButton) -> bool {
        let bit = 1 << b as u8;
        self.mouse_now & bit != 0 && self.mouse_prev & bit == 0
    }
    pub fn mouse_released(&self, b: MouseButton) -> bool {
        let bit = 1 << b as u8;
        self.mouse_now & bit == 0 && self.mouse_prev & bit != 0
    }

    /// Any key or button went down this update.
    pub fn any_pressed(&self) -> bool {
        (0..WORDS).any(|i| self.now[i] & !self.prev[i] != 0) || self.mouse_now & !self.mouse_prev != 0
    }
}
