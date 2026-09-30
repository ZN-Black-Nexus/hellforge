//! Small helpers most games end up writing: particles, a menu, screen
//! shake and easing curves. All in the prelude.

use crate::gfx::Gfx;
use crate::input::{Button, MouseButton};
use crate::math::{Float, Rect, Vec2};
use crate::rng::Rng;
use crate::runner::Ctx;
use alloc::vec::Vec;

// ---------------------------------------------------------------- easing

/// Starts slow, ends fast. `t` goes 0..=1 and so does the result.
pub fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Starts fast, ends slow (good for things arriving).
pub fn ease_out(t: f32) -> f32 {
    let t = 1.0 - t.clamp(0.0, 1.0);
    1.0 - t * t
}

/// Slow at both ends.
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0) * (-2.0 * t + 2.0) / 2.0 }
}

/// Overshoots a little, then settles (good for pop-up text).
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c = 1.70158;
    1.0 + (c + 1.0) * t * t * t + c * t * t
}

/// Bounces at the end like a dropped ball.
pub fn ease_out_bounce(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let (n, d) = (7.5625, 2.75);
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        let t = t - 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        let t = t - 2.25 / d;
        n * t * t + 0.9375
    } else {
        let t = t - 2.625 / d;
        n * t * t + 0.984375
    }
}

// ---------------------------------------------------------------- particles

/// One particle of a [`Particles`] system.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Updates left to live.
    pub life: u32,
    pub color: u8,
    /// Drawn as a square this many pixels wide.
    pub size: f32,
}

/// Sparks, smoke, debris: bursts of particles that fly, fall and fade.
///
/// ```ignore
/// // in the game struct: sparks: Particles,  (Particles::new())
/// self.sparks.burst(ctx.rng(), pos, 12, 2.0, 30, &[YELLOW, ORANGE, RED]);  // update
/// self.sparks.update();                                                 // every update
/// self.sparks.draw(g);                                                  // draw
/// ```
#[derive(Clone, Debug, Default)]
pub struct Particles {
    pub list: Vec<Particle>,
    /// Added to every particle's velocity each update (e.g. `vec2(0, 0.1)` falls).
    pub gravity: Vec2,
    /// Velocity kept each update (1.0 = no slowing down).
    pub drag: f32,
}

impl Particles {
    pub fn new() -> Particles {
        Particles { list: Vec::new(), gravity: Vec2::ZERO, drag: 0.95 }
    }

    /// Add `count` particles at `at`, flying out in random directions at up to
    /// `speed` pixels per update, living up to `life` updates, in random
    /// colours from `colors`.
    pub fn burst(&mut self, rng: &mut Rng, at: Vec2, count: u32, speed: f32, life: u32, colors: &[u8]) {
        for _ in 0..count {
            let vel = Vec2::from_angle(rng.range_f(0.0, core::f32::consts::TAU)) * rng.range_f(0.2, 1.0) * speed;
            let color = if colors.is_empty() { 7 } else { *rng.pick(colors) };
            let life = rng.range((life / 2).max(1) as i32, life.max(1) as i32 + 1) as u32;
            self.list.push(Particle { pos: at, vel, life, color, size: 1.0 });
        }
    }

    /// Add one particle yourself.
    pub fn add(&mut self, p: Particle) {
        self.list.push(p);
    }

    /// Move everything one update and drop the dead.
    pub fn update(&mut self) {
        for p in self.list.iter_mut() {
            p.pos += p.vel;
            p.vel = (p.vel + self.gravity) * self.drag;
            p.life = p.life.saturating_sub(1);
        }
        self.list.retain(|p| p.life > 0);
    }

    pub fn draw(&self, g: &mut Gfx) {
        for p in &self.list {
            if p.size <= 1.0 {
                g.pixel(p.pos.x, p.pos.y, p.color);
            } else {
                g.fill_rect(p.pos.x - p.size / 2.0, p.pos.y - p.size / 2.0, p.size, p.size, p.color);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

// ---------------------------------------------------------------- menu

/// A vertical list of choices: Up/Down (or the mouse) to move, A/Start (or a
/// click) to choose.
///
/// ```ignore
/// // update:
/// if let Some(i) = self.menu.update(ctx, 3) { match i { 0 => start(), 1 => ..., _ => ctx.quit() } }
/// // draw:
/// self.menu.draw(g, 80, &["Start", "Options", "Quit"], WHITE, YELLOW);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub selected: usize,
    /// Where each item was drawn last time (for the mouse).
    rects: Vec<Rect>,
    mouse: (i32, i32),
}

impl Menu {
    pub fn new() -> Menu {
        Menu::default()
    }

    /// Handle input for a menu of `count` items; returns the item chosen
    /// this update, if any.
    pub fn update(&mut self, ctx: &Ctx, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        self.selected = self.selected.min(count - 1);
        if ctx.pressed(Button::Up) {
            self.selected = (self.selected + count - 1) % count;
        }
        if ctx.pressed(Button::Down) {
            self.selected = (self.selected + 1) % count;
        }
        let m = ctx.mouse();
        let moved = (m.x, m.y) != self.mouse;
        self.mouse = (m.x, m.y);
        let hovered = self.rects.iter().position(|r| r.contains(m.x, m.y)).filter(|&i| i < count);
        if let Some(i) = hovered {
            if moved {
                self.selected = i;
            }
            if ctx.mouse_pressed(MouseButton::Left) {
                self.selected = i;
                return Some(i);
            }
        }
        if ctx.pressed(Button::A) || ctx.pressed(Button::Start) { Some(self.selected) } else { None }
    }

    /// Draw the items centred horizontally, starting at `y`, the selected one
    /// in `highlight` with a marker.
    pub fn draw(&mut self, g: &mut Gfx, y: i32, items: &[&str], color: u8, highlight: u8) {
        self.rects.clear();
        for (i, item) in items.iter().enumerate() {
            let w = g.text_width(item);
            let (x, iy) = ((g.width() - w) / 2, y + i as i32 * 12);
            let selected = i == self.selected;
            if selected {
                g.text(x - 10, iy, ">", highlight);
            }
            g.text(x, iy, item, if selected { highlight } else { color });
            self.rects.push(Rect::new((x - 12) as f32, (iy - 2) as f32, (w + 24) as f32, 12.0));
        }
    }
}

// ---------------------------------------------------------------- screen shake

/// Screen shake that fades out. Start it with [`Shake::start`], call
/// [`Shake::update`] every update, and pass [`Shake::offset`] to `g.camera`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shake {
    left: u32,
    total: u32,
    strength: f32,
    offset: (i32, i32),
}

impl Shake {
    pub fn new() -> Shake {
        Shake::default()
    }
    /// Shake by up to `strength` pixels for `updates` updates.
    pub fn start(&mut self, strength: f32, updates: u32) {
        self.strength = strength;
        self.left = updates;
        self.total = updates.max(1);
    }
    pub fn update(&mut self, rng: &mut Rng) {
        if self.left == 0 {
            self.offset = (0, 0);
            return;
        }
        let s = self.strength * self.left as f32 / self.total as f32;
        self.offset = (rng.range_f(-s, s).round() as i32, rng.range_f(-s, s).round() as i32);
        self.left -= 1;
    }
    /// The current offset: `g.camera(shake.offset().0, shake.offset().1)`.
    pub fn offset(&self) -> (i32, i32) {
        self.offset
    }
}
