//! First-person 3D view of a tile map (a "raycaster", like the early-90s
//! shooters): textured walls, floor and ceiling with distance fog, and
//! sprites standing in the world ("billboards"). Positions are in tile units
//! (1.0 = one tile), angles in radians (0 = +x, PI/2 = +y).
//!
//! ```ignore
//! let mut cam = Camera3d::new(vec2(1.5, 1.5), 0.0);
//! // update: turn and walk with collision
//! cam.angle += ctx.axis_x() as f32 * 0.05;
//! let step = cam.forward() * (-ctx.axis_y() as f32 * 0.06);
//! grid.move_circle(&mut cam.pos, step, 0.2, |t| t == b'#');
//! // draw
//! g.raycast(&grid, &cam, DARK_GRAY, BROWN, |t| match t {
//!     b'#' => Wall::Texture(&BRICK),
//!     b'D' => Wall::Color(RED),
//!     _ => Wall::Empty,
//! });
//! g.billboards(&cam, &mut [Billboard::new(&GHOST, vec2(4.5, 2.5))]);
//! ```

use crate::color::TRANSPARENT;
use crate::gfx::Gfx;
use crate::math::{Float, Vec2};
use crate::sprite::Sprite;
use crate::tilemap::Grid;

/// The viewer of a first-person scene.
#[derive(Clone, Copy, Debug)]
pub struct Camera3d {
    /// Position in tile units.
    pub pos: Vec2,
    /// Facing direction in radians (0 = +x, PI/2 = +y).
    pub angle: f32,
    /// Horizontal field of view in radians (default 66 degrees).
    pub fov: f32,
    /// Distance in tiles at which everything has faded to black (default 12).
    pub fog: f32,
}

impl Camera3d {
    pub fn new(pos: Vec2, angle: f32) -> Camera3d {
        Camera3d { pos, angle, fov: 1.15, fog: 12.0 }
    }
    /// Unit vector the camera looks along.
    pub fn forward(&self) -> Vec2 {
        Vec2::from_angle(self.angle)
    }
    /// Unit vector to the camera's right (for strafing).
    pub fn right(&self) -> Vec2 {
        self.forward().perp()
    }
    /// Half-screen vector along the view plane.
    fn plane(&self) -> Vec2 {
        self.right() * (self.fov * 0.5).tan()
    }
}

/// What a tile looks like to [`Gfx::raycast`].
#[derive(Clone, Copy)]
pub enum Wall<'a> {
    /// See-through: rays pass (floors, open doors, items).
    Empty,
    /// A solid wall in one colour.
    Color(u8),
    /// A solid wall with a sprite stretched over each face.
    Texture(&'a Sprite),
}

/// A sprite standing in the world, for [`Gfx::billboards`].
#[derive(Clone, Copy)]
pub struct Billboard<'a> {
    pub sprite: &'a Sprite,
    /// Where it stands, in tile units.
    pub pos: Vec2,
    /// Height in tiles (1.0 = as tall as a wall; default 0.7).
    pub size: f32,
    /// Height of its bottom above the floor, in tiles (default 0).
    pub lift: f32,
}

impl<'a> Billboard<'a> {
    pub fn new(sprite: &'a Sprite, pos: Vec2) -> Billboard<'a> {
        Billboard { sprite, pos, size: 0.7, lift: 0.0 }
    }
}

/// Darkening level (0 = none .. 15) for something `dist` tiles away.
fn fog_level(dist: f32, fog: f32) -> usize {
    ((dist / fog.max(0.1)) * 16.0) as usize
}

impl Gfx {
    fn fog_tables(&mut self) {
        let key = self.pal_hash();
        for level in 1..=16 {
            self.ensure_dark(level, key);
        }
    }

    #[inline]
    fn shaded(&self, c: u8, level: usize) -> u8 {
        let c = self.remap[c as usize];
        if level == 0 { c } else { self.fade[level.min(15)][c as usize] }
    }

    /// Draw a first-person view of `grid` from `cam` over the clip area:
    /// `ceiling` and `floor` colours, and walls as `wall(tile)` says. Draw
    /// [`billboards`](Gfx::billboards) afterwards so walls hide them properly.
    pub fn raycast<'a>(&mut self, grid: &Grid, cam: &Camera3d, ceiling: u8, floor: u8, wall: impl Fn(u8) -> Wall<'a>) {
        self.fog_tables();
        let (w, h) = (self.w, self.h);
        let (x0, y0, x1, y1) = self.clip;
        if self.zbuf.len() != w as usize {
            self.zbuf = alloc::vec![f32::MAX; w as usize];
        }
        let half = h as f32 / 2.0;
        // floor and ceiling: one fog level per row
        for y in y0..y1 {
            let fy = y as f32 + 0.5;
            let (c, dist) = if fy < half { (ceiling, half / (half - fy)) } else { (floor, half / (fy - half)) };
            let c = self.shaded(c, fog_level(dist, cam.fog));
            let row = (y * w) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(c);
        }
        let dir = cam.forward();
        let plane = cam.plane();
        let max_steps = (grid.w + grid.h) * 2 + 4;
        for x in x0..x1 {
            let camx = 2.0 * (x as f32 + 0.5) / w as f32 - 1.0;
            let rd = dir + plane * camx;
            let (mut mx, mut my) = (cam.pos.x.floor() as i32, cam.pos.y.floor() as i32);
            let ddx = if rd.x == 0.0 { f32::MAX } else { (1.0 / rd.x).abs() };
            let ddy = if rd.y == 0.0 { f32::MAX } else { (1.0 / rd.y).abs() };
            let (sx, mut tx) = if rd.x < 0.0 { (-1, (cam.pos.x - mx as f32) * ddx) } else { (1, (mx as f32 + 1.0 - cam.pos.x) * ddx) };
            let (sy, mut ty) = if rd.y < 0.0 { (-1, (cam.pos.y - my as f32) * ddy) } else { (1, (my as f32 + 1.0 - cam.pos.y) * ddy) };
            let mut hit = None;
            let mut side = 0;
            for _ in 0..max_steps {
                if tx < ty {
                    tx += ddx;
                    mx = mx.wrapping_add(sx);
                    side = 0;
                } else {
                    ty += ddy;
                    my = my.wrapping_add(sy);
                    side = 1;
                }
                match wall(grid.get(mx, my)) {
                    Wall::Empty => {}
                    other => {
                        hit = Some(other);
                        break;
                    }
                }
            }
            let Some(face) = hit else {
                self.zbuf[x as usize] = f32::MAX;
                continue;
            };
            let dist = (if side == 0 { tx - ddx } else { ty - ddy }).max(0.02);
            self.zbuf[x as usize] = dist;
            let line = h as f32 / dist;
            let top = half - line / 2.0;
            let level = fog_level(dist, cam.fog) + side * 2; // one side a bit darker
            let ya = (top.ceil() as i32).max(y0);
            let yb = ((top + line).ceil() as i32).min(y1);
            match face {
                Wall::Color(c) => {
                    let c = self.shaded(c, level);
                    for y in ya..yb {
                        self.px[(y * w + x) as usize] = c;
                    }
                }
                Wall::Texture(s) => {
                    // where along the face the ray hit, 0..1
                    let along = if side == 0 { cam.pos.y + dist * rd.y } else { cam.pos.x + dist * rd.x };
                    let mut u = ((along - along.floor()) * s.w as f32) as i32;
                    if (side == 0 && rd.x < 0.0) || (side == 1 && rd.y > 0.0) {
                        u = s.w - 1 - u; // keep textures the right way round
                    }
                    let u = u.clamp(0, s.w - 1);
                    for y in ya..yb {
                        let v = (((y as f32 + 0.5 - top) / line) * s.h as f32) as i32;
                        let p = s.pixels[(v.clamp(0, s.h - 1) * s.w + u) as usize];
                        let p = if p == TRANSPARENT { 0 } else { p };
                        self.px[(y * w + x) as usize] = self.shaded(p, level);
                    }
                }
                Wall::Empty => {}
            }
        }
    }

    /// Draw sprites standing in the first-person world, after [`raycast`](Gfx::raycast)
    /// (walls in front hide them). Sorted far to near for you.
    pub fn billboards(&mut self, cam: &Camera3d, items: &mut [Billboard]) {
        items.sort_by(|a, b| {
            let (da, db) = ((a.pos - cam.pos).length_squared(), (b.pos - cam.pos).length_squared());
            db.partial_cmp(&da).unwrap_or(core::cmp::Ordering::Equal)
        });
        for b in items.iter() {
            self.billboard(cam, b);
        }
    }

    /// Draw one sprite standing in the first-person world (see [`billboards`](Gfx::billboards)).
    pub fn billboard(&mut self, cam: &Camera3d, b: &Billboard) {
        if self.zbuf.len() != self.w as usize {
            self.zbuf = alloc::vec![f32::MAX; self.w as usize];
        }
        self.fog_tables();
        let (w, h) = (self.w as f32, self.h as f32);
        let (dir, plane) = (cam.forward(), cam.plane());
        let rel = b.pos - cam.pos;
        let inv = 1.0 / (plane.x * dir.y - dir.x * plane.y);
        let side = inv * (dir.y * rel.x - dir.x * rel.y);
        let depth = inv * (-plane.y * rel.x + plane.x * rel.y);
        if depth < 0.05 {
            return; // behind the viewer
        }
        let s = b.sprite;
        let sx = (w / 2.0) * (1.0 + side / depth);
        let height = h / depth * b.size;
        let width = height * s.w as f32 / s.h as f32;
        let bottom = h / 2.0 + (h / 2.0) / depth - b.lift * h / depth;
        let (top, left) = (bottom - height, sx - width / 2.0);
        let level = fog_level(depth, cam.fog);
        let (x0, y0, x1, y1) = self.clip;
        let (xa, xb) = ((left.ceil() as i32).max(x0), ((left + width).ceil() as i32).min(x1));
        let (ya, yb) = ((top.ceil() as i32).max(y0), (bottom.ceil() as i32).min(y1));
        for x in xa..xb {
            if depth >= self.zbuf[x as usize] {
                continue; // a wall is in front
            }
            let u = (((x as f32 + 0.5 - left) / width) * s.w as f32) as i32;
            for y in ya..yb {
                let v = (((y as f32 + 0.5 - top) / height) * s.h as f32) as i32;
                let p = s.get(u, v);
                if p != TRANSPARENT {
                    let c = self.shaded(p, level);
                    self.px[(y * self.w + x) as usize] = c;
                }
            }
        }
    }
}
