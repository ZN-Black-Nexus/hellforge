//! Sprites written as ASCII art, turned into pixels at compile time.
//!
//! ```ignore
//! const PLAYER: Sprite = sprite![
//!     "..7777..",
//!     ".7c77c7.",
//!     "77777777",
//!     ".7.77.7.",
//! ];
//! g.sprite(&PLAYER, x, y);
//! ```
//!
//! Each character is one pixel: `.` or space is transparent, `0`-`9` and
//! `a`-`f` are the 16 base colours (hex digits, so `7` is WHITE, `8` RED, `c`
//! BLUE; see `color`). Any other character must be given a colour with
//! `sprite!(legend: [('#', shade(Ramp::Brown, 6))], ...)`. All rows must have
//! the same width; mistakes stop the build with a message.

use crate::color::TRANSPARENT;

/// An image made of palette colours. `pixels` is row-major, `w * h` long;
/// [`TRANSPARENT`] pixels are not drawn.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    pub pixels: &'static [u8],
}

impl Sprite {
    /// Colour at (x, y), or `TRANSPARENT` outside the sprite.
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return TRANSPARENT;
        }
        self.pixels[(y * self.w + x) as usize]
    }
}

/// Pick the frame of an animation for a moment in time: `frames` shown one
/// after another, each for `ticks_per_frame` updates, looping.
///
/// ```ignore
/// let f = anim(&WALK, ctx.frame(), 8);
/// ```
pub fn anim<T>(frames: &[T], tick: u64, ticks_per_frame: u32) -> &T {
    let n = frames.len().max(1) as u64;
    &frames[((tick / ticks_per_frame.max(1) as u64) % n) as usize]
}

#[doc(hidden)]
pub const fn __width(rows: &[&str]) -> usize {
    if rows.is_empty() {
        panic!("sprite!/tilemap!: needs at least one row");
    }
    let w = rows[0].len();
    let mut i = 1;
    while i < rows.len() {
        if rows[i].len() != w {
            panic!("sprite!/tilemap!: every row must have the same number of characters");
        }
        i += 1;
    }
    if w == 0 {
        panic!("sprite!/tilemap!: rows can't be empty");
    }
    w
}

#[doc(hidden)]
pub const fn __color(c: u8, legend: &[(u8, u8)]) -> u8 {
    let mut i = 0;
    while i < legend.len() {
        if legend[i].0 == c {
            return legend[i].1;
        }
        i += 1;
    }
    match c {
        b'.' | b' ' => TRANSPARENT,
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("sprite!: unknown character (use . for transparent, 0-9/a-f for colours, or add it to legend: [...])"),
    }
}

#[doc(hidden)]
pub const fn __parse<const N: usize>(rows: &[&str], legend: &[(u8, u8)]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    let mut r = 0;
    while r < rows.len() {
        let b = rows[r].as_bytes();
        let mut c = 0;
        while c < b.len() {
            out[i] = __color(b[c], legend);
            i += 1;
            c += 1;
        }
        r += 1;
    }
    out
}

/// Build a [`Sprite`] from rows of ASCII art (see the module docs).
///
/// ```ignore
/// const COIN: Sprite = sprite![".aa.", "a99a", "a99a", ".aa."];
/// const WALL: Sprite = sprite!(legend: [('#', shade(Ramp::Gray, 5))], "##", "##");
/// ```
#[macro_export]
macro_rules! sprite {
    (legend: [$(($ch:literal, $col:expr)),* $(,)?], $($row:literal),+ $(,)?) => {{
        const __ROWS: &[&str] = &[$($row),+];
        const __LEGEND: &[(u8, u8)] = &[$(($ch as u8, $col)),*];
        const __W: usize = $crate::sprite::__width(__ROWS);
        const __H: usize = __ROWS.len();
        const __PX: [u8; __W * __H] = $crate::sprite::__parse::<{ __W * __H }>(__ROWS, __LEGEND);
        $crate::sprite::Sprite { w: __W as i32, h: __H as i32, pixels: &__PX }
    }};
    ($($row:literal),+ $(,)?) => {
        $crate::sprite!(legend: [], $($row),+)
    };
}
