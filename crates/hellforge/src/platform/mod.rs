//! Everything that touches the operating system. Games never use this
//! directly: `hellforge::main!` connects a game to the platform entry point.
//!
//! * Linux: our own `_start` and raw system calls (no libc), X11 over its
//!   socket protocol, the framebuffer + evdev, or any terminal.
//! * Windows: Win32 through `raw-dylib` imports (no C runtime), or the console.
//! * macOS: Cocoa through the Objective-C runtime, or the terminal.

pub mod tui;

#[cfg(all(target_os = "linux", not(test)))]
pub mod linux;
#[cfg(all(target_os = "macos", not(test)))]
pub mod macos;
#[cfg(all(target_os = "windows", not(test)))]
pub mod windows;

use crate::input::{Key, MouseButton};

/// Services the engine needs from the operating system.
pub trait Host {
    /// Command-line argument `i` (0 is the program).
    fn arg(&self, i: usize) -> Option<&'static [u8]>;
    /// Write to standard output.
    fn out(&mut self, b: &[u8]);
    /// Write to standard error (logs).
    fn err(&mut self, b: &[u8]);
    /// Monotonic clock in microseconds.
    fn now_us(&mut self) -> u64;
    fn sleep_us(&mut self, us: u64);
    /// Create/truncate a file for writing (UTF-8 path).
    fn create(&mut self, path: &[u8]) -> Option<isize>;
    /// Open a file for reading.
    fn open(&mut self, path: &[u8]) -> Option<isize>;
    fn write(&mut self, f: isize, data: &[u8]) -> bool;
    /// Read up to `buf.len()` bytes; 0 at the end.
    fn read(&mut self, f: isize, buf: &mut [u8]) -> usize;
    fn close(&mut self, f: isize);
    /// The directory for this game's saves (created if needed), no trailing separator.
    fn data_dir(&mut self, game: &[u8], out: &mut Buf<512>) -> bool;
    /// Path separator for building file names.
    fn sep(&self) -> u8 {
        b'/'
    }
    /// Peak resident memory in KiB, if the OS can tell.
    fn peak_rss_kb(&mut self) -> u64 {
        0
    }
    /// Play interactively (window, framebuffer or terminal) until the game quits.
    fn play(&mut self, d: &mut dyn Driver, o: &Opts) -> i32;
}

/// The game, as the platform backends drive it.
pub trait Driver {
    /// Screen size in pixels.
    fn size(&self) -> (usize, usize);
    fn title(&self) -> &'static str;
    /// Updates per second.
    fn fps(&self) -> u32;
    /// Run one fixed update step.
    fn update(&mut self);
    /// Draw the current state into the screen.
    fn render(&mut self);
    fn pixels(&self) -> &[u8];
    fn palette(&self) -> &[u8; 768];
    fn key(&mut self, k: Key, down: bool);
    /// Pointer position in screen pixels.
    fn mouse_pos(&mut self, x: i32, y: i32);
    /// Relative motion while the pointer is captured.
    fn mouse_motion(&mut self, dx: i32, dy: i32);
    fn mouse_button(&mut self, b: MouseButton, down: bool);
    fn mouse_wheel(&mut self, dy: i32);
    /// Forget held keys (the window lost focus).
    fn release_all(&mut self);
    fn quit_requested(&self) -> bool;
    /// The game wants a hidden, captured pointer (mouse look).
    fn wants_capture(&self) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Auto,
    Term,
    X11,
    Fb,
    Window,
}

/// Options for interactive play, from the command line.
pub struct Opts {
    pub backend: Backend,
    /// Window size multiplier (0 = automatic).
    pub scale: u32,
    pub fullscreen: bool,
    pub nomouse: bool,
    /// Terminal colours: 0 auto, 24, 256, 16, 2.
    pub colors: u16,
    /// Frame cap for the terminal renderer.
    pub term_fps: u32,
}

/// Starting window size: the screen scaled up to roughly 960 pixels wide.
pub fn window_scale(w: usize, h: usize, forced: u32) -> usize {
    if forced > 0 {
        return forced as usize;
    }
    (960 / w.max(1)).min(720 / h.max(1)).max(1)
}

/// Where a `sw` x `sh` picture goes inside a `ww` x `wh` window, keeping its
/// shape (square pixels): `(x, y, width, height)`, centred with bars around.
pub fn fit(sw: usize, sh: usize, ww: usize, wh: usize) -> (usize, usize, usize, usize) {
    let (sw, sh) = (sw.max(1), sh.max(1));
    let (dw, dh) = if ww * sh > wh * sw { (wh * sw / sh, wh) } else { (ww, ww * sh / sw) };
    let (dw, dh) = (dw.max(1), dh.max(1));
    (ww.saturating_sub(dw) / 2, wh.saturating_sub(dh) / 2, dw, dh)
}

/// Window pixel -> screen pixel, for the letterbox from [`fit`].
pub fn to_screen(wx: i32, wy: i32, fit: (usize, usize, usize, usize), sw: usize, sh: usize) -> (i32, i32) {
    let (ox, oy, dw, dh) = fit;
    let x = ((wx - ox as i32) as i64 * sw as i64).div_euclid(dw.max(1) as i64) as i32;
    let y = ((wy - oy as i32) as i64 * sh as i64).div_euclid(dh.max(1) as i64) as i32;
    (x, y)
}

/// Fixed-timestep clock: how many updates are due now.
pub struct Clock {
    step: u64,
    next: u64,
}

impl Clock {
    pub fn new(fps: u32, now: u64) -> Clock {
        Clock { step: 1_000_000 / fps.max(1) as u64, next: now }
    }
    /// Number of updates to run now (at most 4; after a long stall it skips
    /// ahead instead of trying to catch up).
    pub fn due(&mut self, now: u64) -> u32 {
        let mut n = 0;
        while now >= self.next && n < 4 {
            self.next += self.step;
            n += 1;
        }
        if now > self.next + self.step * 8 {
            self.next = now;
        }
        n
    }
    /// Microseconds until the next update.
    pub fn wait(&self, now: u64) -> u64 {
        self.next.saturating_sub(now)
    }
}

/// Fixed-capacity byte string (paths, messages): no heap needed.
pub struct Buf<const N: usize> {
    pub b: [u8; N],
    pub len: usize,
}

impl<const N: usize> Buf<N> {
    pub const fn new() -> Self {
        Buf { b: [0; N], len: 0 }
    }
    pub fn push(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.len + 1 < N {
                self.b[self.len] = c;
                self.len += 1;
            }
        }
        self
    }
    pub fn num(&mut self, v: u64) -> &mut Self {
        let mut t = [0u8; 20];
        let mut i = 20;
        let mut n = v;
        loop {
            i -= 1;
            t[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.push(&t[i..])
    }
    pub fn hex(&mut self, v: u32) -> &mut Self {
        for i in (0..8).rev() {
            self.push(&[b"0123456789abcdef"[((v >> (i * 4)) & 15) as usize]]);
        }
        self
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.b[..self.len]
    }
    /// NUL-terminated view for system calls.
    pub fn cstr(&mut self) -> &[u8] {
        self.b[self.len.min(N - 1)] = 0;
        &self.b[..self.len + 1]
    }
}

impl<const N: usize> Default for Buf<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Game names become folder names: keep letters, digits, `-` and `_`.
pub fn folder_name(title: &str, out: &mut Buf<64>) {
    for c in title.bytes() {
        if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' {
            out.push(&[c]);
        } else if c == b' ' {
            out.push(b"_");
        }
    }
    if out.len == 0 {
        out.push(b"hellforge-game");
    }
}

pub(crate) fn parse_u32(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut v: u32 = 0;
    for &c in s {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u32)?;
    }
    Some(v)
}

#[cfg(not(test))]
unsafe extern "Rust" {
    /// Defined by `hellforge::main!` in the game crate.
    fn __hellforge_entry(host: &mut dyn Host) -> i32;
}

/// Hand control to the game (called once by each platform's entry point).
#[cfg(not(test))]
pub(crate) fn enter(host: &mut dyn Host) -> i32 {
    // SAFETY: `hellforge::main!` defines this function in the game.
    unsafe { __hellforge_entry(host) }
}

/// Print a fatal message, put the screen/terminal back and exit (panic handler).
#[cfg(not(test))]
pub fn panic_exit(msg: &[u8]) -> ! {
    #[cfg(target_os = "linux")]
    linux::panic_exit(msg);
    #[cfg(target_os = "windows")]
    windows::panic_exit(msg);
    #[cfg(target_os = "macos")]
    macos::panic_exit(msg);
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = msg;
        loop {}
    }
}
