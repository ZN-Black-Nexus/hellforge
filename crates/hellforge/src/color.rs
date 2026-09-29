//! The 256-colour palette.
//!
//! A colour is a palette index (`u8`). Indices 0-15 are the named base
//! colours below. Indices 16-254 are 15 ramps of 16 shades each, dark to
//! light, reached with [`shade`]. [`rgb`] finds the nearest palette colour for
//! any RGB value (at compile time when used in a `const`). The palette can be
//! changed while the game runs with `Gfx::set_color`.
//!
//! ```ignore
//! g.clear(DARK_BLUE);
//! g.fill_rect(10, 10, 20, 20, shade(Ramp::Green, 12));
//! const SKY: u8 = rgb(120, 180, 255);
//! ```

pub const BLACK: u8 = 0;
pub const DARK_BLUE: u8 = 1;
pub const DARK_PURPLE: u8 = 2;
pub const DARK_GREEN: u8 = 3;
pub const BROWN: u8 = 4;
pub const DARK_GRAY: u8 = 5;
pub const LIGHT_GRAY: u8 = 6;
pub const WHITE: u8 = 7;
pub const RED: u8 = 8;
pub const ORANGE: u8 = 9;
pub const YELLOW: u8 = 10;
pub const GREEN: u8 = 11;
pub const BLUE: u8 = 12;
pub const INDIGO: u8 = 13;
pub const PINK: u8 = 14;
pub const PEACH: u8 = 15;

/// Marks see-through pixels in sprites (the `.` in `sprite!` art). Drawing
/// functions never write this value.
pub const TRANSPARENT: u8 = 255;

/// The base colours as 0xRRGGBB.
const BASE: [u32; 16] = [
    0x000000, 0x1a2a5c, 0x6e2a5e, 0x16784b, 0x9c5a38, 0x5b5657, 0xbcbfc6, 0xffffff, 0xe8313f, 0xf5921f, 0xf7dc3a,
    0x46d157, 0x3a9bf0, 0x7d6fb0, 0xf27ab0, 0xf9c9a1,
];

/// The 15 colour ramps of the extended palette.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Ramp {
    Gray,
    Red,
    Orange,
    Yellow,
    Lime,
    Green,
    Teal,
    Cyan,
    Sky,
    Blue,
    Violet,
    Purple,
    Magenta,
    Rose,
    Brown,
}

/// Full-strength colour of each ramp (reached at shade 10).
const RAMP: [u32; 15] = [
    0x9a9a9a, 0xe0302a, 0xe8801c, 0xe8d020, 0x98d024, 0x30b040, 0x20a888, 0x20c0d0, 0x3a9ae8, 0x3050e0, 0x6a40d8,
    0xa040c8, 0xd038b0, 0xe04878, 0x9a6440,
];

/// Palette index of `level` (0 = darkest, 15 = lightest) on a colour ramp.
pub const fn shade(ramp: Ramp, level: u8) -> u8 {
    16 + ramp as u8 * 16 + if level > 15 { 15 } else { level }
}

const fn ramp_rgb(r: usize, level: u32) -> u32 {
    let c = RAMP[r];
    let (cr, cg, cb) = ((c >> 16) & 255, (c >> 8) & 255, c & 255);
    if r == 0 {
        // neutral grey: an even ladder from near-black to white
        let v = 8 + level * 247 / 15;
        return (v << 16) | (v << 8) | v;
    }
    let (r2, g2, b2) = if level <= 10 {
        ((cr * (level + 1)) / 11, (cg * (level + 1)) / 11, (cb * (level + 1)) / 11)
    } else {
        let t = level - 10; // 1..5: towards white
        (cr + (255 - cr) * t / 6, cg + (255 - cg) * t / 6, cb + (255 - cb) * t / 6)
    };
    (r2 << 16) | (g2 << 8) | b2
}

const fn build() -> [u32; 256] {
    let mut p = [0u32; 256];
    let mut i = 0;
    while i < 16 {
        p[i] = BASE[i];
        i += 1;
    }
    let mut r = 0;
    while r < 15 {
        let mut l = 0;
        while l < 16 {
            p[16 + r * 16 + l] = ramp_rgb(r, l as u32);
            l += 1;
        }
        r += 1;
    }
    p
}

/// The default palette as 0xRRGGBB values.
pub const PALETTE: [u32; 256] = build();

/// Perceptual-ish colour distance ("redmean"), cheap and good at keeping hues.
pub(crate) const fn distance(a: u32, b: u32) -> i32 {
    let (ar, ag, ab) = (((a >> 16) & 255) as i32, ((a >> 8) & 255) as i32, (a & 255) as i32);
    let (br, bg, bb) = (((b >> 16) & 255) as i32, ((b >> 8) & 255) as i32, (b & 255) as i32);
    let rmean = (ar + br) / 2;
    let (dr, dg, db) = (ar - br, ag - bg, ab - bb);
    (((512 + rmean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - rmean) * db * db) >> 8)
}

/// The palette colour closest to an RGB value. Works in `const` items.
pub const fn rgb(r: u8, g: u8, b: u8) -> u8 {
    hex(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

/// The palette colour closest to `0xRRGGBB`. Works in `const` items.
pub const fn hex(c: u32) -> u8 {
    let mut best = 0;
    let mut best_d = i32::MAX;
    let mut i = 0;
    while i < 255 {
        let d = distance(PALETTE[i], c);
        if d < best_d {
            best_d = d;
            best = i;
        }
        i += 1;
    }
    best as u8
}
