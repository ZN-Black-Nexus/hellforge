//! Tile maps written as text, and collision against them.
//!
//! ```ignore
//! const LEVEL: Tilemap = tilemap![
//!     "####################",
//!     "#..............o...#",
//!     "#..P.......#####...#",
//!     "####################",
//! ];
//! let mut grid = Grid::from_map(&LEVEL);        // mutable copy
//! let (px, py) = grid.find(b'P').unwrap();       // tile coordinates
//! grid.set(px, py, b'.');
//! ```
//!
//! A tile is just the byte of its character; you decide what `#` or `o`
//! means. [`Grid::move_box`] moves a box through the solid tiles, which is all
//! a platformer or top-down game needs for walls.

use crate::math::{Rect, Vec2};
use alloc::vec::Vec;

/// A read-only grid of tile characters, made with [`tilemap!`](crate::tilemap!).
#[derive(Clone, Copy, Debug)]
pub struct Tilemap {
    pub w: i32,
    pub h: i32,
    pub tiles: &'static [u8],
}

impl Tilemap {
    /// Tile at (tx, ty); `0` outside the map.
    pub fn get(&self, tx: i32, ty: i32) -> u8 {
        if tx < 0 || ty < 0 || tx >= self.w || ty >= self.h {
            return 0;
        }
        self.tiles[(ty * self.w + tx) as usize]
    }
}

#[doc(hidden)]
pub const fn __tiles<const N: usize>(rows: &[&str]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    let mut r = 0;
    while r < rows.len() {
        let b = rows[r].as_bytes();
        let mut c = 0;
        while c < b.len() {
            out[i] = b[c];
            i += 1;
            c += 1;
        }
        r += 1;
    }
    out
}

/// Build a [`Tilemap`] from rows of text (all rows the same width).
#[macro_export]
macro_rules! tilemap {
    ($($row:literal),+ $(,)?) => {{
        const __ROWS: &[&str] = &[$($row),+];
        const __W: usize = $crate::sprite::__width(__ROWS);
        const __H: usize = __ROWS.len();
        const __T: [u8; __W * __H] = $crate::tilemap::__tiles::<{ __W * __H }>(__ROWS);
        $crate::tilemap::Tilemap { w: __W as i32, h: __H as i32, tiles: &__T }
    }};
}

/// Which sides a moving box bumped into (see [`Grid::move_box`]).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Hit {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    /// Landed on something (standing on the ground in a platformer).
    pub bottom: bool,
}

impl Hit {
    pub fn any(&self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// A mutable tile grid.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: i32,
    pub h: i32,
    pub tiles: Vec<u8>,
}

impl Grid {
    /// A `w` x `h` grid filled with `fill`.
    pub fn new(w: i32, h: i32, fill: u8) -> Grid {
        Grid { w, h, tiles: alloc::vec![fill; (w.max(0) * h.max(0)) as usize] }
    }

    /// A mutable copy of a [`Tilemap`].
    pub fn from_map(m: &Tilemap) -> Grid {
        Grid { w: m.w, h: m.h, tiles: m.tiles.to_vec() }
    }

    /// Tile at (tx, ty); `0` outside the grid.
    pub fn get(&self, tx: i32, ty: i32) -> u8 {
        if tx < 0 || ty < 0 || tx >= self.w || ty >= self.h {
            return 0;
        }
        self.tiles[(ty * self.w + tx) as usize]
    }

    /// Change a tile (ignored outside the grid).
    pub fn set(&mut self, tx: i32, ty: i32, t: u8) {
        if tx >= 0 && ty >= 0 && tx < self.w && ty < self.h {
            self.tiles[(ty * self.w + tx) as usize] = t;
        }
    }

    /// First tile equal to `t`, scanning rows top to bottom.
    pub fn find(&self, t: u8) -> Option<(i32, i32)> {
        let i = self.tiles.iter().position(|&c| c == t)?;
        Some((i as i32 % self.w, i as i32 / self.w))
    }

    /// Every tile equal to `t`.
    pub fn find_all(&self, t: u8) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for (i, &c) in self.tiles.iter().enumerate() {
            if c == t {
                v.push((i as i32 % self.w, i as i32 / self.w));
            }
        }
        v
    }

    /// How many tiles equal `t`.
    pub fn count(&self, t: u8) -> usize {
        self.tiles.iter().filter(|&&c| c == t).count()
    }

    /// Replace every `from` tile with `to`.
    pub fn replace(&mut self, from: u8, to: u8) {
        for c in self.tiles.iter_mut() {
            if *c == from {
                *c = to;
            }
        }
    }

    /// Does the rectangle (in pixels, tiles `tile` pixels square) touch any
    /// tile for which `solid` is true? Outside the grid counts as the `0` tile.
    pub fn rect_hits(&self, r: Rect, tile: i32, solid: impl Fn(u8) -> bool) -> bool {
        let ts = tile as f32;
        let x0 = crate::math::Num::to_i32(r.x / ts);
        let y0 = crate::math::Num::to_i32(r.y / ts);
        let x1 = crate::math::Num::to_i32((r.x + r.w - 0.001) / ts);
        let y1 = crate::math::Num::to_i32((r.y + r.h - 0.001) / ts);
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                if solid(self.get(tx, ty)) {
                    return true;
                }
            }
        }
        false
    }

    /// Move a box of `size` pixels at `pos` (top-left) by `vel`, stopping at
    /// solid tiles. Moves along x then y, so it slides along walls. Sets the
    /// velocity to 0 on each axis that hit something and reports the sides.
    ///
    /// ```ignore
    /// self.vel.y += 0.25; // gravity
    /// let hit = self.grid.move_box(&mut self.pos, &mut self.vel, vec2(6, 8), 8, |t| t == b'#');
    /// if hit.bottom && ctx.pressed(Button::A) { self.vel.y = -4.0; }
    /// ```
    pub fn move_box(&self, pos: &mut Vec2, vel: &mut Vec2, size: Vec2, tile: i32, solid: impl Fn(u8) -> bool) -> Hit {
        let mut hit = Hit::default();
        let ts = tile as f32;
        // Move in steps no longer than half a tile so fast objects can't tunnel.
        let steps = ((vel.x.abs().max(vel.y.abs()) / (ts * 0.5)) as i32 + 1).max(1);
        let (sx, sy) = (vel.x / steps as f32, vel.y / steps as f32);
        for _ in 0..steps {
            if sx != 0.0 && !hit.left && !hit.right {
                let r = Rect { x: pos.x + sx, y: pos.y, w: size.x, h: size.y };
                if self.rect_hits(r, tile, &solid) {
                    // snap against the tile edge
                    if sx > 0.0 {
                        let edge = crate::math::Num::to_i32((pos.x + size.x + sx) / ts) as f32 * ts;
                        pos.x = edge - size.x;
                        hit.right = true;
                    } else {
                        let edge = (crate::math::Num::to_i32((pos.x + sx) / ts) + 1) as f32 * ts;
                        pos.x = edge;
                        hit.left = true;
                    }
                } else {
                    pos.x += sx;
                }
            }
            if sy != 0.0 && !hit.top && !hit.bottom {
                let r = Rect { x: pos.x, y: pos.y + sy, w: size.x, h: size.y };
                if self.rect_hits(r, tile, &solid) {
                    if sy > 0.0 {
                        let edge = crate::math::Num::to_i32((pos.y + size.y + sy) / ts) as f32 * ts;
                        pos.y = edge - size.y;
                        hit.bottom = true;
                    } else {
                        let edge = (crate::math::Num::to_i32((pos.y + sy) / ts) + 1) as f32 * ts;
                        pos.y = edge;
                        hit.top = true;
                    }
                } else {
                    pos.y += sy;
                }
            }
        }
        if hit.left || hit.right {
            vel.x = 0.0;
        }
        if hit.top || hit.bottom {
            vel.y = 0.0;
        }
        hit
    }
}
