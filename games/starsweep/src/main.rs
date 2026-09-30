//! Star Sweep: a small side-scrolling shooter. Arrows/WASD fly, hold A
//! (Z, Space) to fire. Survive the waves; the game gets faster over time.

#![no_std]
#![no_main]

use hellforge::prelude::*;

const SHIP: Sprite = sprite![
    "..cc........",
    "..ccc.......",
    ".cc77cc.....",
    "8cc7777cccc.",
    ".cc77cc.....",
    "..ccc.......",
    "..cc........",
];
const DRONE: Sprite = sprite![
    "...bb...",
    "..bbbb..",
    ".bb77bb.",
    "bbb71bbb",
    ".bb77bb.",
    "..bbbb..",
    "...bb...",
];
const CHASER: Sprite = sprite![
    "....88...",
    "..8888...",
    "888818888",
    "..8888...",
    "....88...",
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Title,
    Play,
    Over,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Flies left along a sine wave.
    Drone,
    /// Tougher; drifts towards the ship's height and shoots at it.
    Chaser,
}

struct Enemy {
    kind: Kind,
    pos: Vec2,
    base_y: f32,
    age: u32,
    hp: i32,
    flash: u32,
}

struct Shot {
    pos: Vec2,
    vel: Vec2,
}

struct Spark {
    pos: Vec2,
    vel: Vec2,
    life: i32,
    color: u8,
}

struct Star {
    pos: Vec2,
    speed: f32,
}

struct StarSweep {
    state: State,
    ship: Vec2,
    cooldown: u32,
    invulnerable: u32,
    lives: u32,
    score: u32,
    best: u32,
    time: u32,
    enemies: Vec<Enemy>,
    shots: Vec<Shot>,
    enemy_shots: Vec<Shot>,
    sparks: Vec<Spark>,
    stars: Vec<Star>,
}

impl StarSweep {
    fn start(&mut self) {
        self.state = State::Play;
        self.ship = vec2(20, 86);
        self.lives = 3;
        self.score = 0;
        self.time = 0;
        self.invulnerable = 90;
        self.enemies.clear();
        self.shots.clear();
        self.enemy_shots.clear();
    }

    fn explode(&mut self, ctx: &mut Ctx, at: Vec2, n: u32) {
        for _ in 0..n {
            let vel = Vec2::from_angle(ctx.rand_range_f(0.0, TAU)) * ctx.rand_range_f(0.3, 2.2);
            let color = *ctx.rng().pick(&[YELLOW, ORANGE, RED, WHITE]);
            self.sparks.push(Spark { pos: at, vel, life: ctx.rand_range(12, 30), color });
        }
    }

    fn spawn(&mut self, ctx: &mut Ctx) {
        // More often as time goes on.
        let every = 70u32.saturating_sub(self.time / 300).max(18);
        if self.time % every != 0 {
            return;
        }
        let y = ctx.rand_range_f(20.0, 160.0);
        if ctx.chance(0.25 + (self.time as f32 / 20000.0).min(0.35)) {
            self.enemies.push(Enemy { kind: Kind::Chaser, pos: vec2(330, y), base_y: y, age: 0, hp: 3, flash: 0 });
        } else {
            // a little wave of drones
            for i in 0..4 {
                let p = vec2(330 + i * 16, y);
                self.enemies.push(Enemy { kind: Kind::Drone, pos: p, base_y: y, age: i as u32 * 8, hp: 1, flash: 0 });
            }
        }
    }

    fn hit_ship(&mut self, ctx: &mut Ctx) {
        if self.invulnerable > 0 {
            return;
        }
        let at = self.ship + vec2(6, 3);
        self.explode(ctx, at, 30);
        self.lives -= 1;
        self.invulnerable = 120;
        if self.lives == 0 {
            self.state = State::Over;
            if self.score > self.best {
                self.best = self.score;
                ctx.save(0, &self.best.to_le_bytes());
            }
        }
    }
}

impl Game for StarSweep {
    const TITLE: &'static str = "Star Sweep";

    fn new(ctx: &mut Ctx) -> Self {
        let mut stars = Vec::new();
        for i in 0..60 {
            let speed = [0.3, 0.7, 1.5][i % 3];
            stars.push(Star { pos: vec2(ctx.rand_range(0, 320), ctx.rand_range(0, 180)), speed });
        }
        let mut saved = [0u8; 4];
        let best = if ctx.load(0, &mut saved) == 4 { u32::from_le_bytes(saved) } else { 0 };
        StarSweep {
            state: State::Title,
            ship: vec2(20, 86),
            cooldown: 0,
            invulnerable: 0,
            lives: 3,
            score: 0,
            best,
            time: 0,
            enemies: Vec::new(),
            shots: Vec::new(),
            enemy_shots: Vec::new(),
            sparks: Vec::new(),
            stars,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) {
        for s in self.stars.iter_mut() {
            s.pos.x -= s.speed;
            if s.pos.x < 0.0 {
                s.pos.x += 320.0;
            }
        }
        for s in self.sparks.iter_mut() {
            s.pos += s.vel;
            s.vel *= 0.95;
            s.life -= 1;
        }
        self.sparks.retain(|s| s.life > 0);
        if self.state != State::Play {
            if ctx.pressed(Button::A) || ctx.pressed(Button::Start) {
                self.start();
            }
            return;
        }
        self.time += 1;
        self.invulnerable = self.invulnerable.saturating_sub(1);

        // the ship
        self.ship += ctx.axis().normalized() * 2.0;
        self.ship.x = self.ship.x.clamp(0.0, 200.0);
        self.ship.y = self.ship.y.clamp(12.0, 172.0);
        self.cooldown = self.cooldown.saturating_sub(1);
        if ctx.held(Button::A) && self.cooldown == 0 {
            self.shots.push(Shot { pos: self.ship + vec2(12, 3), vel: vec2(5, 0) });
            self.cooldown = 8;
        }
        for s in self.shots.iter_mut().chain(self.enemy_shots.iter_mut()) {
            s.pos += s.vel;
        }
        self.shots.retain(|s| s.pos.x < 330.0);
        self.enemy_shots.retain(|s| s.pos.x > -10.0 && s.pos.y > -10.0 && s.pos.y < 190.0);

        // enemies
        self.spawn(ctx);
        let ship_y = self.ship.y;
        let mut new_shots = Vec::new();
        for e in self.enemies.iter_mut() {
            e.age += 1;
            e.flash = e.flash.saturating_sub(1);
            match e.kind {
                Kind::Drone => {
                    e.pos.x -= 1.4;
                    e.pos.y = e.base_y + (e.age as f32 * 0.07).sin() * 24.0;
                }
                Kind::Chaser => {
                    e.pos.x -= 0.8;
                    e.pos.y = approach(e.pos.y, ship_y, 0.6);
                    if e.age % 90 == 45 {
                        let aim = (self.ship - e.pos).normalized() * 1.8;
                        new_shots.push(Shot { pos: e.pos, vel: aim });
                    }
                }
            }
        }
        self.enemy_shots.extend(new_shots);

        // our shots against enemies
        let mut blasts = Vec::new();
        for s in self.shots.iter_mut() {
            for e in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                let w = if e.kind == Kind::Drone { 8.0 } else { 9.0 };
                if Rect::new(e.pos.x, e.pos.y, w, 7.0).contains(s.pos.x, s.pos.y) {
                    e.hp -= 1;
                    e.flash = 6;
                    s.pos.x = 999.0; // used up
                    if e.hp == 0 {
                        self.score += if e.kind == Kind::Drone { 10 } else { 50 };
                        blasts.push(e.pos + vec2(4, 3));
                    }
                    break;
                }
            }
        }
        for b in blasts {
            self.explode(ctx, b, 14);
        }
        self.enemies.retain(|e| e.hp > 0 && e.pos.x > -20.0);

        // anything touching the ship?
        let ship = Rect::new(self.ship.x + 2.0, self.ship.y + 1.0, 9.0, 5.0);
        let rammed = self.enemies.iter().any(|e| Rect::new(e.pos.x, e.pos.y, 8.0, 7.0).overlaps(&ship));
        let shot = self.enemy_shots.iter().any(|s| ship.contains(s.pos.x, s.pos.y));
        if rammed || shot {
            self.hit_ship(ctx);
        }
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.clear(BLACK);
        for s in &self.stars {
            let c = if s.speed > 1.0 { WHITE } else if s.speed > 0.5 { LIGHT_GRAY } else { DARK_GRAY };
            g.pixel(s.pos.x, s.pos.y, c);
        }
        for e in &self.enemies {
            let spr = if e.kind == Kind::Drone { &DRONE } else { &CHASER };
            if e.flash > 0 {
                g.sprite_tinted(spr, e.pos.x, e.pos.y, WHITE);
            } else {
                g.sprite(spr, e.pos.x, e.pos.y);
            }
        }
        for s in &self.shots {
            g.fill_rect(s.pos.x - 3.0, s.pos.y, 4, 1, YELLOW);
        }
        for s in &self.enemy_shots {
            g.fill_circle(s.pos.x, s.pos.y, 1, PINK);
        }
        for s in &self.sparks {
            g.pixel(s.pos.x, s.pos.y, s.color);
        }
        if self.state == State::Play && (self.invulnerable == 0 || g.frame() % 8 < 5) {
            g.sprite(&SHIP, self.ship.x, self.ship.y);
        }
        text!(g, 4, 3, WHITE, "SCORE {}", self.score);
        text!(g, 130, 3, LIGHT_GRAY, "BEST {}", self.best);
        for i in 0..self.lives {
            g.sprite(&SHIP, 290 - i as i32 * 14, 2);
        }
        match self.state {
            State::Title => {
                let w = g.text_width("STAR SWEEP") * 2;
                g.text_scaled((g.width() - w) / 2, 60, "STAR SWEEP", BLUE, 2);
                g.text_centered(90, "Arrows fly, hold Z to fire", WHITE);
                if g.frame() / 30 % 2 == 0 {
                    g.text_centered(110, "Press Z to start", WHITE);
                }
            }
            State::Over => {
                g.text_centered(80, "GAME OVER", RED);
                g.text_centered(100, "Press Z to play again", LIGHT_GRAY);
            }
            State::Play => {}
        }
    }

    fn debug(&self, out: &mut String) {
        let _ = write!(
            out,
            "state {:?} time {} score {} lives {} ship ({:.0},{:.0}) enemies {} shots {} enemy shots {}",
            self.state,
            self.time,
            self.score,
            self.lives,
            self.ship.x,
            self.ship.y,
            self.enemies.len(),
            self.shots.len(),
            self.enemy_shots.len()
        );
    }
}

hellforge::main!(StarSweep);
