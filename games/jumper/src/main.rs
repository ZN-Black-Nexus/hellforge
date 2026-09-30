//! Jumper: a small platformer. Left/Right run, A (Z, Space) or Up jumps
//! (hold it to jump higher). Grab coins, stomp walkers, avoid spikes and pits,
//! and reach the flag.

#![no_std]
#![no_main]

use hellforge::prelude::*;

// The level: '#' ground, '=' stone ledge, 'o' coin, '^' spikes, 'e' walker,
// 'P' start, 'F' flag. 8x8 pixel tiles.
const LEVEL: Tilemap = tilemap![
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#..............................................................................................#",
    "#...................................ooooo......................................................#",
    "#...................................=====...................ooooo..............................#",
    "#...........................ooeooo..........................=====..............................#",
    "#...........................======..........................................ooooooo............#",
    "#...........ooooo...................................oooooo..................=======............#",
    "#...........=====...................................======.....................................#",
    "#.....................oooo..............##...oooo.....................oooo...........##........#",
    "#.......................................##...........................................##........#",
    "#.P...............e...........^^...e....##........e.....^^^.......e.............e....##.^^...F.#",
    "######################....###################....#####################....######################",
    "######################....###################....#####################....######################",
];
const TILE: i32 = 8;

const BRICK: Sprite = sprite!(
    legend: [('#', shade(Ramp::Brown, 6)), ('-', shade(Ramp::Brown, 3)), ('+', shade(Ramp::Brown, 9))],
    "+++++++-",
    "#######-",
    "#######-",
    "--------",
    "+++-++++",
    "###-####",
    "###-####",
    "--------",
);
const STONE: Sprite = sprite![
    "66666665",
    "6d6d6d65",
    "66666665",
    "55555555",
    "66656666",
    "6d656d6d",
    "66656666",
    "55555555",
];
const SPIKE: Sprite = sprite![
    "........",
    "........",
    ".6...6..",
    ".6...6..",
    "666.666.",
    "666.666.",
    "66666666",
    "55555555",
];
const COIN: [Sprite; 3] = [
    sprite!["........", "...aa...", "..a99a..", "..a99a..", "..a99a..", "..a99a..", "...aa...", "........"],
    sprite!["........", "...aa...", "...9a...", "...9a...", "...9a...", "...9a...", "...aa...", "........"],
    sprite!["........", "...a....", "...a....", "...a....", "...a....", "...a....", "...a....", "........"],
];
const FLAG: Sprite = sprite![
    "7bbbb...",
    "7bbbbbb.",
    "7bbbb...",
    "7.......",
    "7.......",
    "7.......",
    "7.......",
    "7.......",
];
// Hero, facing right (flipped when running left).
const HERO: [Sprite; 3] = [
    sprite!["...aa...", "..aaaa..", "..a1a1..", "..aaaa..", ".cccccc.", "..cccc..", "..c..c..", ".cc..cc."],
    sprite!["...aa...", "..aaaa..", "..a1a1..", "..aaaa..", ".cccccc.", "..cccc..", "..cc.c..", "...c.cc."],
    sprite!["...aa...", "..aaaa..", "..a1a1..", "..aaaa..", "cccccccc", "..cccc..", ".c....c.", "........"],
];
const WALKER: [Sprite; 2] = [
    sprite!["........", "..8888..", ".888888.", "88788788", "88888888", ".888888.", ".8.88.8.", "8..88..8"],
    sprite!["........", "........", "..8888..", ".888888.", "88788788", "88888888", ".888888.", "88.88.88"],
];

const GRAVITY: f32 = 0.25;
const MAX_FALL: f32 = 4.5;
const RUN: f32 = 1.6;
const ACCEL: f32 = 0.25;
const JUMP: f32 = -4.2;
const HERO_SIZE: Vec2 = Vec2::new(6.0, 8.0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Title,
    Play,
    Dying(u32),
    Won,
    Over,
}

struct Walker {
    pos: Vec2,
    vel: Vec2,
    dir: f32,
    alive: bool,
}

struct Jumper {
    state: State,
    level: Grid,
    start: Vec2,
    pos: Vec2,
    vel: Vec2,
    facing_left: bool,
    on_ground: bool,
    coyote: u32,
    jump_buffer: u32,
    walkers: Vec<Walker>,
    coins: u32,
    total_coins: u32,
    lives: u32,
    time: u32,
    cam: f32,
}

fn solid(t: u8) -> bool {
    t == b'#' || t == b'='
}

impl Jumper {
    fn load_level(&mut self) {
        self.level = Grid::from_map(&LEVEL);
        let (sx, sy) = self.level.find(b'P').unwrap_or((2, 2));
        self.level.set(sx, sy, b'.');
        self.start = vec2(sx * TILE + 1, sy * TILE);
        self.walkers.clear();
        for (tx, ty) in self.level.find_all(b'e') {
            self.level.set(tx, ty, b'.');
            self.walkers.push(Walker { pos: vec2(tx * TILE, ty * TILE), vel: Vec2::ZERO, dir: -1.0, alive: true });
        }
        self.total_coins = self.level.count(b'o') as u32;
        self.coins = 0;
        self.time = 0;
        self.respawn();
    }

    fn respawn(&mut self) {
        self.pos = self.start;
        self.vel = Vec2::ZERO;
        self.facing_left = false;
        self.state = State::Play;
    }

    fn hero_rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, HERO_SIZE.x, HERO_SIZE.y)
    }

    /// Every tile the rectangle touches.
    fn tiles_under(&self, r: Rect) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for ty in (r.y / TILE as f32).floor() as i32..=((r.bottom() - 0.01) / TILE as f32).floor() as i32 {
            for tx in (r.x / TILE as f32).floor() as i32..=((r.right() - 0.01) / TILE as f32).floor() as i32 {
                v.push((tx, ty));
            }
        }
        v
    }

    fn die(&mut self) {
        self.lives -= 1;
        self.state = State::Dying(60);
    }

    fn update_hero(&mut self, ctx: &mut Ctx) {
        // run with a little inertia
        self.vel.x = approach(self.vel.x, ctx.axis_x() as f32 * RUN, ACCEL);
        if self.vel.x != 0.0 {
            self.facing_left = self.vel.x < 0.0;
        }
        // Jumps are buffered for a few updates and still work just after running off a ledge.
        let jump_key = ctx.pressed(Button::A) || ctx.pressed(Button::Up);
        self.jump_buffer = if jump_key { 6 } else { self.jump_buffer.saturating_sub(1) };
        self.coyote = if self.on_ground { 6 } else { self.coyote.saturating_sub(1) };
        if self.jump_buffer > 0 && self.coyote > 0 {
            self.vel.y = JUMP;
            self.jump_buffer = 0;
            self.coyote = 0;
        }
        // let go early for a short hop
        if self.vel.y < -1.5 && !(ctx.held(Button::A) || ctx.held(Button::Up)) {
            self.vel.y = -1.5;
        }
        self.vel.y = (self.vel.y + GRAVITY).min(MAX_FALL);
        let hit = self.level.move_box(&mut self.pos, &mut self.vel, HERO_SIZE, TILE, solid);
        self.on_ground = hit.bottom;

        // coins, spikes, the flag
        let body = self.hero_rect().expand(-1.0);
        for (tx, ty) in self.tiles_under(body) {
            match self.level.get(tx, ty) {
                b'o' => {
                    self.level.set(tx, ty, b'.');
                    self.coins += 1;
                }
                b'^' if body.bottom() > (ty * TILE + 3) as f32 => self.die(),
                b'F' => self.state = State::Won,
                _ => {}
            }
        }
        if self.pos.y > (self.level.h * TILE) as f32 {
            self.die(); // fell into a pit
        }
    }

    fn update_walkers(&mut self) {
        let hero = self.hero_rect();
        let mut stomped = false;
        let mut hurt = false;
        for w in self.walkers.iter_mut().filter(|w| w.alive) {
            w.vel.x = w.dir * 0.5;
            w.vel.y = (w.vel.y + GRAVITY).min(MAX_FALL);
            let hit = self.level.move_box(&mut w.pos, &mut w.vel, vec2(8, 8), TILE, solid);
            // turn at walls and at ledges
            let ahead = if w.dir > 0.0 { w.pos.x + 9.0 } else { w.pos.x - 1.0 };
            let below = self.level.get((ahead / TILE as f32).floor() as i32, ((w.pos.y + 9.0) / TILE as f32).floor() as i32);
            if hit.left || hit.right || (hit.bottom && !solid(below)) {
                w.dir = -w.dir;
            }
            let body = Rect::new(w.pos.x, w.pos.y + 2.0, 8.0, 6.0);
            if hero.overlaps(&body) {
                if self.vel.y > 0.0 && hero.bottom() < body.y + 4.0 {
                    w.alive = false; // stomped
                    stomped = true;
                } else {
                    hurt = true;
                }
            }
        }
        if stomped {
            self.vel.y = -3.0;
        } else if hurt {
            self.die();
        }
    }
}

impl Game for Jumper {
    const TITLE: &'static str = "Jumper";

    fn new(_ctx: &mut Ctx) -> Self {
        let mut j = Jumper {
            state: State::Title,
            level: Grid::from_map(&LEVEL),
            start: Vec2::ZERO,
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            facing_left: false,
            on_ground: false,
            coyote: 0,
            jump_buffer: 0,
            walkers: Vec::new(),
            coins: 0,
            total_coins: 0,
            lives: 3,
            time: 0,
            cam: 0.0,
        };
        j.load_level();
        j.state = State::Title;
        j
    }

    fn update(&mut self, ctx: &mut Ctx) {
        match self.state {
            State::Title | State::Won | State::Over => {
                if ctx.pressed(Button::A) || ctx.pressed(Button::Start) {
                    self.lives = 3;
                    self.load_level();
                }
            }
            State::Dying(t) => {
                self.state = if t > 0 {
                    State::Dying(t - 1)
                } else if self.lives == 0 {
                    State::Over
                } else {
                    self.vel = Vec2::ZERO;
                    State::Play
                };
                if self.state == State::Play {
                    self.respawn();
                }
            }
            State::Play => {
                self.time += 1;
                self.update_hero(ctx);
                if self.state == State::Play {
                    self.update_walkers();
                }
            }
        }
        // the camera follows the hero, but never shows past the level's ends
        let max = (self.level.w * TILE - ctx.width()) as f32;
        self.cam = (self.pos.x - ctx.width() as f32 / 2.0).clamp(0.0, max);
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.clear(DARK_BLUE);
        // far hills scroll slower than the level (parallax)
        let off = (self.cam / 3.0) as i32 % 90;
        for i in -1..5 {
            let x = i * 90 - off;
            g.fill_triangle(x, 176, x + 45, 118, x + 90, 176, DARK_PURPLE);
        }
        g.camera(self.cam, 0);
        g.grid(&self.level, 0, 0, TILE, |g, t, x, y| match t {
            b'#' => g.sprite(&BRICK, x, y),
            b'=' => g.sprite(&STONE, x, y),
            b'^' => g.sprite(&SPIKE, x, y),
            b'F' => g.sprite(&FLAG, x, y),
            b'o' => {
                let f = g.frame();
                g.sprite(anim(&COIN, f + x as u64, 8), x, y)
            }
            _ => {}
        });
        let frame = g.frame();
        for w in self.walkers.iter().filter(|w| w.alive) {
            g.sprite_flipped(anim(&WALKER, frame, 10), w.pos.x, w.pos.y, w.dir > 0.0, false);
        }
        let hero = if !self.on_ground {
            &HERO[2]
        } else if self.vel.x.abs() > 0.1 {
            anim(&HERO[..2], frame, 6)
        } else {
            &HERO[0]
        };
        let visible = match self.state {
            State::Dying(t) => t % 8 < 4, // blink
            State::Title => false,
            _ => true,
        };
        if visible {
            g.sprite_flipped(hero, self.pos.x - 1.0, self.pos.y, self.facing_left, false);
        }
        g.camera(0, 0);

        text!(g, 4, 3, YELLOW, "COINS {}/{}", self.coins, self.total_coins);
        text!(g, 130, 3, WHITE, "TIME {}", self.time / 60);
        for i in 0..self.lives {
            g.sprite(&HERO[0], 300 - i as i32 * 10, 2);
        }
        match self.state {
            State::Title => {
                let w = g.text_width("JUMPER") * 3;
                g.text_scaled((g.width() - w) / 2, 50, "JUMPER", YELLOW, 3);
                g.text_centered(90, "Run, jump, grab the coins, reach the flag", WHITE);
                if frame / 30 % 2 == 0 {
                    g.text_centered(110, "Press Z to start", WHITE);
                }
            }
            State::Won => {
                g.text_centered(70, "YOU MADE IT!", GREEN);
                text!(g, 110, 90, WHITE, "{} coins in {} s", self.coins, self.time / 60);
                g.text_centered(110, "Press Z to play again", LIGHT_GRAY);
            }
            State::Over => {
                g.text_centered(80, "GAME OVER", RED);
                g.text_centered(100, "Press Z to try again", LIGHT_GRAY);
            }
            _ => {}
        }
    }

    fn debug(&self, out: &mut String) {
        let _ = write!(
            out,
            "state {:?} pos ({:.1},{:.1}) vel ({:.2},{:.2}) ground {} coins {}/{} lives {} walkers alive {}",
            self.state,
            self.pos.x,
            self.pos.y,
            self.vel.x,
            self.vel.y,
            self.on_ground,
            self.coins,
            self.total_coins,
            self.lives,
            self.walkers.iter().filter(|w| w.alive).count()
        );
    }
}

hellforge::main!(Jumper);
