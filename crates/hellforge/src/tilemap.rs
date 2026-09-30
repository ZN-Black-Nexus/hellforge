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

use crate::math::{Num, Rect, Vec2};
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

/// Where a ray from [`Grid::cast_ray`] stopped.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    /// Distance from the start, in tiles.
    pub dist: f32,
    /// The point where it hit.
    pub point: Vec2,
    /// Coordinates of the tile it hit.
    pub tile: (i32, i32),
    /// The tile it hit.
    pub t: u8,
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
        let ts = tile.max(1) as f32;
        let x0 = Num::to_i32(r.x / ts);
        let y0 = Num::to_i32(r.y / ts);
        let x1 = Num::to_i32((r.x + r.w - 0.001) / ts);
        let y1 = Num::to_i32((r.y + r.h - 0.001) / ts);
        // Everything outside the grid is the `0` tile: check it once.
        if (x0 < 0 || y0 < 0 || x1 >= self.w || y1 >= self.h) && solid(0) {
            return true;
        }
        for ty in y0.max(0)..=y1.min(self.h - 1) {
            for tx in x0.max(0)..=x1.min(self.w - 1) {
                if solid(self.get(tx, ty)) {
                    return true;
                }
            }
        }
        false
    }

    /// First-person movement in tile units (1.0 = one tile): move a circle of
    /// `radius` at `pos` by `delta`, sliding along solid tiles.
    ///
    /// ```ignore
    /// let step = cam.forward() * (ctx.axis_y() as f32 * -0.05);
    /// grid.move_circle(&mut cam.pos, step, 0.25, |t| t == b'#');
    /// ```
    pub fn move_circle(&self, pos: &mut Vec2, delta: Vec2, radius: f32, solid: impl Fn(u8) -> bool) -> Hit {
        let mut hit = Hit::default();
        let r = radius;
        let x = pos.x + delta.x;
        if !self.rect_hits(Rect::new(x - r, pos.y - r, 2.0 * r, 2.0 * r), 1, &solid) {
            pos.x = x;
        } else if delta.x > 0.0 {
            hit.right = true;
        } else if delta.x < 0.0 {
            hit.left = true;
        }
        let y = pos.y + delta.y;
        if !self.rect_hits(Rect::new(pos.x - r, y - r, 2.0 * r, 2.0 * r), 1, &solid) {
            pos.y = y;
        } else if delta.y > 0.0 {
            hit.bottom = true;
        } else if delta.y < 0.0 {
            hit.top = true;
        }
        hit
    }

    /// Follow a ray from `from` in direction `dir` (tile units) until it enters
    /// a tile for which `solid` is true, up to `max` tiles away.
    pub fn cast_ray(&self, from: Vec2, dir: Vec2, max: f32, solid: impl Fn(u8) -> bool) -> Option<RayHit> {
        let d = dir.normalized();
        if d == Vec2::ZERO {
            return None;
        }
        let (mut mx, mut my) = (Num::to_i32(from.x), Num::to_i32(from.y));
        let ddx = if d.x == 0.0 { f32::MAX } else { (1.0 / d.x).abs() };
        let ddy = if d.y == 0.0 { f32::MAX } else { (1.0 / d.y).abs() };
        let (sx, mut tx) = if d.x < 0.0 { (-1, (from.x - mx as f32) * ddx) } else { (1, (mx as f32 + 1.0 - from.x) * ddx) };
        let (sy, mut ty) = if d.y < 0.0 { (-1, (from.y - my as f32) * ddy) } else { (1, (my as f32 + 1.0 - from.y) * ddy) };
        loop {
            let dist = if tx < ty {
                mx += sx;
                tx += ddx;
                tx - ddx
            } else {
                my += sy;
                ty += ddy;
                ty - ddy
            };
            if dist > max {
                return None;
            }
            let t = self.get(mx, my);
            if solid(t) {
                return Some(RayHit { dist, point: from + d * dist, tile: (mx, my), t });
            }
            if mx < -1 || my < -1 || mx > self.w || my > self.h {
                return None; // left the grid
            }
        }
    }

    /// Can you see from `a` to `b` (tile units) without a solid tile between?
    pub fn line_of_sight(&self, a: Vec2, b: Vec2, solid: impl Fn(u8) -> bool) -> bool {
        let d = b - a;
        self.cast_ray(a, d, d.length(), solid).is_none()
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
        if !(vel.x.is_finite() && vel.y.is_finite()) {
            *vel = Vec2::ZERO;
        }
        let steps = ((vel.x.abs().max(vel.y.abs()) / (ts * 0.5)) as i32 + 1).clamp(1, 1024);
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
