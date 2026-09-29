//! Drawing on the screen.
//!
//! The screen is `Game::WIDTH` x `Game::HEIGHT` pixels, each one a palette
//! colour (`u8`, see `color`). (0, 0) is the top-left corner. Everything is
//! clipped to the screen, so drawing off-screen is always safe. Coordinates
//! and sizes accept any number type (`i32`, `f32`, `usize`, ...).

use crate::color::{PALETTE, TRANSPARENT};
use crate::font::{self, CHAR_H, CHAR_W, LINE_H};
use crate::math::{Float, Num, Vec2};
use crate::sprite::Sprite;
use crate::tilemap::{Grid, Tilemap};

/// The drawing surface handed to `Game::draw`.
pub struct Gfx {
    px: &'static mut [u8],
    w: i32,
    h: i32,
    pal: [u8; 768],
    remap: [u8; 256],
    cam: (i32, i32),
    clip: (i32, i32, i32, i32),
    pub(crate) frame: u64,
    pub(crate) fps: u32,
    fade_key: [u32; 17],
    fade: [[u8; 256]; 17],
}

fn default_palette() -> [u8; 768] {
    let mut p = [0u8; 768];
    for (i, c) in PALETTE.iter().enumerate() {
        p[i * 3] = (c >> 16) as u8;
        p[i * 3 + 1] = (c >> 8) as u8;
        p[i * 3 + 2] = *c as u8;
    }
    p
}

fn identity() -> [u8; 256] {
    let mut r = [0u8; 256];
    for (i, v) in r.iter_mut().enumerate() {
        *v = i as u8;
    }
    r
}

impl Gfx {
    pub(crate) fn new(px: &'static mut [u8], w: usize, h: usize) -> Gfx {
        Gfx {
            px,
            w: w as i32,
            h: h as i32,
            pal: default_palette(),
            remap: identity(),
            cam: (0, 0),
            clip: (0, 0, w as i32, h as i32),
            frame: 0,
            fps: 60,
            fade_key: [u32::MAX; 17],
            fade: [[0; 256]; 17],
        }
    }

    /// Reset per-frame state (camera, clip) before `Game::draw`.
    pub(crate) fn begin(&mut self) {
        self.cam = (0, 0);
        self.clip = (0, 0, self.w, self.h);
    }

    pub(crate) fn palette_rgb(&self) -> &[u8; 768] {
        &self.pal
    }

    // ------------------------------------------------------------ basics

    /// Screen width in pixels.
    pub fn width(&self) -> i32 {
        self.w
    }
    /// Screen height in pixels.
    pub fn height(&self) -> i32 {
        self.h
    }
    /// Number of updates since the game started (handy for blinking and animation).
    pub fn frame(&self) -> u64 {
        self.frame
    }
    /// Seconds since the game started (in game time).
    pub fn time(&self) -> f32 {
        self.frame as f32 / self.fps.max(1) as f32
    }

    /// Fill the whole screen with one colour (ignores camera and clip).
    pub fn clear(&mut self, c: u8) {
        let c = self.remap[c as usize];
        self.px.fill(c);
    }

    /// The screen's pixels, row by row (`width * height` colours).
    pub fn pixels(&self) -> &[u8] {
        self.px
    }
    /// Direct access for your own effects (plasma, raycasters, ...).
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        self.px
    }

    // ------------------------------------------------------------ camera, clip, colours

    /// Shift everything drawn afterwards by (-x, -y): set it to your scroll
    /// position to draw the world, back to (0, 0) for the HUD. Reset every frame.
    pub fn camera(&mut self, x: impl Num, y: impl Num) {
        self.cam = (x.to_i32(), y.to_i32());
    }
    /// The current camera position.
    pub fn camera_pos(&self) -> (i32, i32) {
        self.cam
    }
    /// Only draw inside this screen rectangle (camera ignored). Reset every frame.
    pub fn clip(&mut self, x: impl Num, y: impl Num, w: impl Num, h: impl Num) {
        let (x, y) = (x.to_i32(), y.to_i32());
        let (x1, y1) = (x + w.to_i32(), y + h.to_i32());
        self.clip = (x.max(0), y.max(0), x1.min(self.w), y1.min(self.h));
    }
    /// Draw on the whole screen again.
    pub fn clip_reset(&mut self) {
        self.clip = (0, 0, self.w, self.h);
    }
    /// Change palette entry `index` to `0xRRGGBB` (affects the whole screen).
    pub fn set_color(&mut self, index: u8, rgb: u32) {
        let i = index as usize * 3;
        self.pal[i] = (rgb >> 16) as u8;
        self.pal[i + 1] = (rgb >> 8) as u8;
        self.pal[i + 2] = rgb as u8;
    }
    /// Palette entry `index` as `0xRRGGBB`.
    pub fn get_color(&self, index: u8) -> u32 {
        let i = index as usize * 3;
        ((self.pal[i] as u32) << 16) | ((self.pal[i + 1] as u32) << 8) | self.pal[i + 2] as u32
    }
    /// Back to the default palette.
    pub fn reset_palette(&mut self) {
        self.pal = default_palette();
    }
    /// Draw colour `from` as `to` from now on (e.g. to flash or recolour sprites).
    pub fn swap_color(&mut self, from: u8, to: u8) {
        self.remap[from as usize] = to;
    }
    /// Undo all `swap_color`s.
    pub fn reset_swaps(&mut self) {
        self.remap = identity();
    }

    /// Darken everything drawn so far: `amount` 0.0 = unchanged, 1.0 = black.
    /// Good for fades and pause screens.
    pub fn fade(&mut self, amount: f32) {
        let level = ((amount.max(0.0).min(1.0)) * 16.0 + 0.5) as usize;
        if level == 0 {
            return;
        }
        let key = self.pal_hash();
        if self.fade_key[level] != key {
            let keep = (16 - level) as u32;
            for i in 0..256 {
                let c = self.get_color(i as u8);
                let target = ((((c >> 16) & 255) * keep / 16) << 16) | ((((c >> 8) & 255) * keep / 16) << 8) | ((c & 255) * keep / 16);
                let mut best = (i32::MAX, 0u8);
                for j in 0..255 {
                    let d = crate::color::distance(self.get_color(j as u8), target);
                    if d < best.0 {
                        best = (d, j as u8);
                    }
                }
                self.fade[level][i] = best.1;
            }
            self.fade_key[level] = key;
        }
        let t = &self.fade[level];
        for p in self.px.iter_mut() {
            *p = t[*p as usize];
        }
    }

    fn pal_hash(&self) -> u32 {
        let mut h = 0x811c_9dc5u32;
        for &b in self.pal.iter() {
            h = (h ^ b as u32).wrapping_mul(0x0100_0193);
        }
        h
    }

    // ------------------------------------------------------------ pixels and shapes

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: u8) {
        let (x0, y0, x1, y1) = self.clip;
        if x >= x0 && y >= y0 && x < x1 && y < y1 {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    /// Horizontal run in screen space (camera already applied), colour already remapped.
    fn span(&mut self, xa: i32, xb: i32, y: i32, c: u8) {
        let (x0, y0, x1, y1) = self.clip;
        if y < y0 || y >= y1 {
            return;
        }
        let (a, b) = (xa.max(x0), (xb + 1).min(x1));
        if a < b {
            let row = (y * self.w) as usize;
            self.px[row + a as usize..row + b as usize].fill(c);
        }
    }

    /// Set one pixel.
    pub fn pixel(&mut self, x: impl Num, y: impl Num, c: u8) {
        let c = self.remap[c as usize];
        self.put(x.to_i32() - self.cam.0, y.to_i32() - self.cam.1, c);
    }

    /// Colour of one pixel (camera applied); `0` outside the screen.
    pub fn get_pixel(&self, x: impl Num, y: impl Num) -> u8 {
        let (x, y) = (x.to_i32() - self.cam.0, y.to_i32() - self.cam.1);
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.px[(y * self.w + x) as usize]
    }

    /// A one-pixel line from (x0, y0) to (x1, y1), both ends included.
    pub fn line(&mut self, x0: impl Num, y0: impl Num, x1: impl Num, y1: impl Num, c: u8) {
        let c = self.remap[c as usize];
        let (mut x, mut y) = (x0.to_i32() - self.cam.0, y0.to_i32() - self.cam.1);
        let (x1, y1) = (x1.to_i32() - self.cam.0, y1.to_i32() - self.cam.1);
        let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
        let (sx, sy) = (if x < x1 { 1 } else { -1 }, if y < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        // Bail out of absurdly long lines instead of spinning.
        let mut guard = 0;
        loop {
            self.put(x, y, c);
            if (x == x1 && y == y1) || guard > 1 << 16 {
                break;
            }
            guard += 1;
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Outline of a rectangle (top-left corner, width, height).
    pub fn rect(&mut self, x: impl Num, y: impl Num, w: impl Num, h: impl Num, c: u8) {
        let (x, y, w, h) = (x.to_i32(), y.to_i32(), w.to_i32(), h.to_i32());
        if w <= 0 || h <= 0 {
            return;
        }
        let c2 = self.remap[c as usize];
        let (sx, sy) = (x - self.cam.0, y - self.cam.1);
        self.span(sx, sx + w - 1, sy, c2);
        self.span(sx, sx + w - 1, sy + h - 1, c2);
        for yy in sy + 1..sy + h - 1 {
            self.put(sx, yy, c2);
            self.put(sx + w - 1, yy, c2);
        }
    }

    /// Filled rectangle (top-left corner, width, height).
    pub fn fill_rect(&mut self, x: impl Num, y: impl Num, w: impl Num, h: impl Num, c: u8) {
        let (x, y, w, h) = (x.to_i32() - self.cam.0, y.to_i32() - self.cam.1, w.to_i32(), h.to_i32());
        if w <= 0 || h <= 0 {
            return;
        }
        let c = self.remap[c as usize];
        for yy in y.max(self.clip.1)..(y + h).min(self.clip.3) {
            self.span(x, x + w - 1, yy, c);
        }
    }

    /// Outline of a circle centred on (cx, cy).
    pub fn circle(&mut self, cx: impl Num, cy: impl Num, r: impl Num, c: u8) {
        let (cx, cy, r) = (cx.to_i32() - self.cam.0, cy.to_i32() - self.cam.1, r.to_i32());
        if r < 0 {
            return;
        }
        let c = self.remap[c as usize];
        let (mut x, mut y, mut err) = (r, 0, 1 - r);
        while x >= y {
            for (px, py) in [(x, y), (y, x), (-y, x), (-x, y), (-x, -y), (-y, -x), (y, -x), (x, -y)] {
                self.put(cx + px, cy + py, c);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Filled circle centred on (cx, cy).
    pub fn fill_circle(&mut self, cx: impl Num, cy: impl Num, r: impl Num, c: u8) {
        let (cx, cy, r) = (cx.to_i32() - self.cam.0, cy.to_i32() - self.cam.1, r.to_i32());
        if r < 0 {
            return;
        }
        let c = self.remap[c as usize];
        for dy in -r..=r {
            let dx = crate::math::isqrt((r * r - dy * dy) as u32 + r as u32) as i32;
            self.span(cx - dx, cx + dx, cy + dy, c);
        }
    }

    /// Outline of a triangle.
    pub fn triangle(&mut self, x0: impl Num, y0: impl Num, x1: impl Num, y1: impl Num, x2: impl Num, y2: impl Num, c: u8) {
        let (x0, y0, x1, y1, x2, y2) = (x0.to_i32(), y0.to_i32(), x1.to_i32(), y1.to_i32(), x2.to_i32(), y2.to_i32());
        self.line(x0, y0, x1, y1, c);
        self.line(x1, y1, x2, y2, c);
        self.line(x2, y2, x0, y0, c);
    }

    /// Filled triangle.
    pub fn fill_triangle(&mut self, x0: impl Num, y0: impl Num, x1: impl Num, y1: impl Num, x2: impl Num, y2: impl Num, c: u8) {
        let pts = [
            Vec2::new(x0.to_f32(), y0.to_f32()),
            Vec2::new(x1.to_f32(), y1.to_f32()),
            Vec2::new(x2.to_f32(), y2.to_f32()),
        ];
        self.fill_polygon(&pts, c);
    }

    /// Outline of a closed polygon through `points`.
    pub fn polygon(&mut self, points: &[Vec2], c: u8) {
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            self.line(a.x, a.y, b.x, b.y, c);
        }
    }

    /// Filled polygon (any shape; self-crossing ones fill by the even-odd rule).
    pub fn fill_polygon(&mut self, points: &[Vec2], c: u8) {
        let n = points.len();
        if n < 3 {
            return;
        }
        let c = self.remap[c as usize];
        let (cx, cy) = (self.cam.0 as f32, self.cam.1 as f32);
        let (mut top, mut bottom) = (f32::MAX, f32::MIN);
        for p in points {
            top = top.min(p.y - cy);
            bottom = bottom.max(p.y - cy);
        }
        let y0 = (top.floor() as i32).max(self.clip.1);
        let y1 = (bottom.ceil() as i32).min(self.clip.3 - 1);
        let mut xs = [0f32; 64];
        for y in y0..=y1 {
            let sy = y as f32 + 0.5;
            let mut k = 0;
            for i in 0..n {
                let (a, b) = (points[i], points[(i + 1) % n]);
                let (ay, by) = (a.y - cy, b.y - cy);
                if (ay <= sy && by > sy) || (by <= sy && ay > sy) {
                    if k < xs.len() {
                        xs[k] = (a.x - cx) + (sy - ay) / (by - ay) * (b.x - a.x);
                        k += 1;
                    }
                }
            }
            xs[..k].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
            let mut i = 0;
            while i + 1 < k {
                let xa = (xs[i] + 0.5).floor() as i32;
                let xb = (xs[i + 1] - 0.5).ceil() as i32;
                if xb >= xa {
                    self.span(xa, xb, y, c);
                }
                i += 2;
            }
        }
    }

    // ------------------------------------------------------------ text

    /// Draw text with the built-in 5x8 font; `\n` starts a new line.
    /// Returns the x just after the last character.
    pub fn text(&mut self, x: impl Num, y: impl Num, s: &str, c: u8) -> i32 {
        self.text_scaled(x, y, s, c, 1)
    }

    /// Text with every font pixel drawn as a `scale` x `scale` block.
    pub fn text_scaled(&mut self, x: impl Num, y: impl Num, s: &str, c: u8, scale: i32) -> i32 {
        let scale = scale.max(1);
        let c = self.remap[c as usize];
        let (x0, mut y) = (x.to_i32() - self.cam.0, y.to_i32() - self.cam.1);
        let mut x = x0;
        for ch in s.chars() {
            if ch == '\n' {
                x = x0;
                y += LINE_H * scale;
                continue;
            }
            let g = font::glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                if *bits == 0 {
                    continue;
                }
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        let (px, py) = (x + col * scale, y + row as i32 * scale);
                        for yy in 0..scale {
                            self.span(px, px + scale - 1, py + yy, c);
                        }
                    }
                }
            }
            x += CHAR_W * scale;
        }
        x + self.cam.0
    }

    /// Text centred horizontally on the screen.
    pub fn text_centered(&mut self, y: impl Num, s: &str, c: u8) {
        let x = (self.w - font::text_width(s)) / 2 + self.cam.0;
        self.text(x, y, s, c);
    }

    /// Text with a one-pixel outline, readable over any background.
    pub fn text_outlined(&mut self, x: impl Num, y: impl Num, s: &str, c: u8, outline: u8) -> i32 {
        let (x, y) = (x.to_i32(), y.to_i32());
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            self.text(x + dx, y + dy, s, outline);
        }
        self.text(x, y, s, c)
    }

    /// Width in pixels of `s` in the built-in font.
    pub fn text_width(&self, s: &str) -> i32 {
        font::text_width(s)
    }

    /// Height in pixels of `s` (one line is `CHAR_H` = 8).
    pub fn text_height(&self, s: &str) -> i32 {
        font::text_height(s)
    }

    // ------------------------------------------------------------ sprites

    fn blit_sprite(&mut self, s: &Sprite, x: i32, y: i32, flip_x: bool, flip_y: bool, scale: i32, tint: Option<u8>) {
        let scale = scale.max(1);
        let (bx, by) = (x - self.cam.0, y - self.cam.1);
        for sy in 0..s.h {
            let src_y = if flip_y { s.h - 1 - sy } else { sy };
            for sx in 0..s.w {
                let src_x = if flip_x { s.w - 1 - sx } else { sx };
                let p = s.pixels[(src_y * s.w + src_x) as usize];
                if p == TRANSPARENT {
                    continue;
                }
                let col = self.remap[tint.unwrap_or(p) as usize];
                if scale == 1 {
                    self.put(bx + sx, by + sy, col);
                } else {
                    let (px, py) = (bx + sx * scale, by + sy * scale);
                    for yy in 0..scale {
                        self.span(px, px + scale - 1, py + yy, col);
                    }
                }
            }
        }
    }

    /// Draw a sprite with its top-left corner at (x, y).
    pub fn sprite(&mut self, s: &Sprite, x: impl Num, y: impl Num) {
        self.blit_sprite(s, x.to_i32(), y.to_i32(), false, false, 1, None);
    }

    /// Draw a sprite mirrored left-right and/or upside down.
    pub fn sprite_flipped(&mut self, s: &Sprite, x: impl Num, y: impl Num, flip_x: bool, flip_y: bool) {
        self.blit_sprite(s, x.to_i32(), y.to_i32(), flip_x, flip_y, 1, None);
    }

    /// Draw a sprite `scale` times bigger (whole numbers).
    pub fn sprite_scaled(&mut self, s: &Sprite, x: impl Num, y: impl Num, scale: i32) {
        self.blit_sprite(s, x.to_i32(), y.to_i32(), false, false, scale, None);
    }

    /// Draw every visible pixel of a sprite in one colour (hit flashes, shadows).
    pub fn sprite_tinted(&mut self, s: &Sprite, x: impl Num, y: impl Num, c: u8) {
        self.blit_sprite(s, x.to_i32(), y.to_i32(), false, false, 1, Some(c));
    }

    /// Draw a sprite rotated by `angle` radians (clockwise on screen) around
    /// its centre, with the centre at (cx, cy).
    pub fn sprite_rotated(&mut self, s: &Sprite, cx: impl Num, cy: impl Num, angle: f32) {
        let (cx, cy) = (cx.to_f32() - self.cam.0 as f32, cy.to_f32() - self.cam.1 as f32);
        let (sn, cs) = (angle.sin(), angle.cos());
        let (hw, hh) = (s.w as f32 / 2.0, s.h as f32 / 2.0);
        let r = (hw * hw + hh * hh).sqrt() + 1.0;
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for y in y0.max(self.clip.1)..=y1.min(self.clip.3 - 1) {
            for x in x0.max(self.clip.0)..=x1.min(self.clip.2 - 1) {
                // rotate the screen point back into sprite space
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let sx = (dx * cs + dy * sn + hw).floor() as i32;
                let sy = (-dx * sn + dy * cs + hh).floor() as i32;
                let p = s.get(sx, sy);
                if p != TRANSPARENT {
                    let col = self.remap[p as usize];
                    self.px[(y * self.w + x) as usize] = col;
                }
            }
        }
    }

    /// Draw a raw image (`w` x `h` colours, row by row; `TRANSPARENT` skipped).
    pub fn blit(&mut self, pixels: &[u8], w: impl Num, h: impl Num, x: impl Num, y: impl Num) {
        let (w, h) = (w.to_i32(), h.to_i32());
        let (bx, by) = (x.to_i32() - self.cam.0, y.to_i32() - self.cam.1);
        for yy in 0..h {
            for xx in 0..w {
                let i = (yy * w + xx) as usize;
                if i < pixels.len() && pixels[i] != TRANSPARENT {
                    let c = self.remap[pixels[i] as usize];
                    self.put(bx + xx, by + yy, c);
                }
            }
        }
    }

    // ------------------------------------------------------------ tile maps

    /// Call `draw(g, tile, px, py)` for every on-screen tile of a map placed
    /// with its top-left corner at (x, y), tiles `tile` pixels square. `px, py`
    /// are the tile's world position (the camera still applies to what you draw).
    ///
    /// ```ignore
    /// g.tilemap(&LEVEL, 0, 0, 8, |g, t, x, y| match t {
    ///     b'#' => g.sprite(&WALL, x, y),
    ///     b'o' => g.sprite(&COIN, x, y),
    ///     _ => {}
    /// });
    /// ```
    pub fn tilemap(&mut self, map: &Tilemap, x: impl Num, y: impl Num, tile: i32, draw: impl FnMut(&mut Gfx, u8, i32, i32)) {
        self.tiles(map.w, map.h, |tx, ty| map.get(tx, ty), x.to_i32(), y.to_i32(), tile, draw);
    }

    /// Like [`Gfx::tilemap`] for a mutable [`Grid`].
    pub fn grid(&mut self, grid: &Grid, x: impl Num, y: impl Num, tile: i32, draw: impl FnMut(&mut Gfx, u8, i32, i32)) {
        self.tiles(grid.w, grid.h, |tx, ty| grid.get(tx, ty), x.to_i32(), y.to_i32(), tile, draw);
    }

    fn tiles(
        &mut self,
        w: i32,
        h: i32,
        get: impl Fn(i32, i32) -> u8,
        x: i32,
        y: i32,
        tile: i32,
        mut draw: impl FnMut(&mut Gfx, u8, i32, i32),
    ) {
        let tile = tile.max(1);
        // only the tiles that can be visible through the camera
        let tx0 = ((self.cam.0 - x).div_euclid(tile)).max(0);
        let ty0 = ((self.cam.1 - y).div_euclid(tile)).max(0);
        let tx1 = ((self.cam.0 - x + self.w).div_euclid(tile) + 1).min(w);
        let ty1 = ((self.cam.1 - y + self.h).div_euclid(tile) + 1).min(h);
        for ty in ty0..ty1 {
            for tx in tx0..tx1 {
                let t = get(tx, ty);
                draw(self, t, x + tx * tile, y + ty * tile);
            }
        }
    }
}

/// A fixed-size text buffer for formatting without the heap (used by `text!`).
pub struct FmtBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FmtBuf<N> {
    pub const fn new() -> Self {
        FmtBuf { buf: [0; N], len: 0 }
    }
    pub fn as_str(&self) -> &str {
        // Only whole UTF-8 characters are ever copied in.
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for FmtBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> core::fmt::Write for FmtBuf<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.chars() {
            let mut tmp = [0u8; 4];
            let e = ch.encode_utf8(&mut tmp).as_bytes();
            if self.len + e.len() > N {
                break; // too long: cut off
            }
            self.buf[self.len..self.len + e.len()].copy_from_slice(e);
            self.len += e.len();
        }
        Ok(())
    }
}

/// Draw formatted text: `text!(g, x, y, color, "Score: {}", score)`.
/// Returns the x just after the last character.
#[macro_export]
macro_rules! text {
    ($g:expr, $x:expr, $y:expr, $c:expr, $($arg:tt)*) => {{
        let mut __b = $crate::gfx::FmtBuf::<256>::new();
        let _ = ::core::fmt::Write::write_fmt(&mut __b, format_args!($($arg)*));
        $g.text($x, $y, __b.as_str(), $c)
    }};
}

/// The glyph height, for layout: one line of text is `CHAR_H` pixels tall.
pub const TEXT_HEIGHT: i32 = CHAR_H;
