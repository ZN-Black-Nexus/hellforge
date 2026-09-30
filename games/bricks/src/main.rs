//! Bricks: a small breakout game. Left/Right (or the mouse) move the paddle,
//! A (Z, Space) or a click serves the ball. Clear every brick to win.

#![no_std]
#![no_main]

use hellforge::prelude::*;

// The wall of bricks. Each character is a brick's colour (hex digits are the
// base colours, as in sprites); '.' is an empty slot.
const WALL: Tilemap = tilemap![
    "8888888888888888",
    "9999999999999999",
    "aaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbb",
    "cccccccccccccccc",
];
const BRICK_W: i32 = 20;
const BRICK_H: i32 = 8;
const WALL_TOP: i32 = 24;
const PADDLE_W: f32 = 40.0;
const PADDLE_Y: f32 = 166.0;
const BALL_R: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Title,
    Serve,
    Play,
    Won,
    Over,
}

struct Spark {
    pos: Vec2,
    vel: Vec2,
    life: i32,
    color: u8,
}

struct Bricks {
    state: State,
    bricks: Grid,
    paddle: f32, // left edge of the paddle
    ball: Vec2,
    vel: Vec2,
    score: u32,
    best: u32,
    lives: u32,
    sparks: Vec<Spark>,
    mouse_x: i32,
}

/// Brick character -> palette colour.
fn brick_color(t: u8) -> u8 {
    match t {
        b'0'..=b'9' => t - b'0',
        b'a'..=b'f' => t - b'a' + 10,
        _ => WHITE,
    }
}

impl Bricks {
    fn new_game(&mut self) {
        self.bricks = Grid::from_map(&WALL);
        self.score = 0;
        self.lives = 3;
        self.state = State::Serve;
    }

    fn lose_ball(&mut self, ctx: &mut Ctx) {
        self.lives -= 1;
        self.state = if self.lives == 0 { State::Over } else { State::Serve };
        if self.state == State::Over {
            self.save_best(ctx);
        }
    }

    fn save_best(&mut self, ctx: &mut Ctx) {
        if self.score > self.best {
            self.best = self.score;
            ctx.save(0, &self.best.to_le_bytes());
        }
    }

    /// Move the ball one small step; returns true if it hit a brick.
    fn step_ball(&mut self, ctx: &mut Ctx, step: Vec2) -> bool {
        let prev = self.ball;
        self.ball += step;
        let w = ctx.width() as f32;
        if self.ball.x < BALL_R || self.ball.x > w - BALL_R {
            self.vel.x = -self.vel.x;
            self.ball.x = prev.x;
        }
        if self.ball.y < BALL_R {
            self.vel.y = -self.vel.y;
            self.ball.y = prev.y;
        }
        // Paddle: the further from its centre, the steeper the bounce.
        let pad = rect(self.paddle, PADDLE_Y, PADDLE_W, 5);
        if self.vel.y > 0.0 && pad.contains(self.ball.x, self.ball.y + BALL_R) {
            let off = (self.ball.x - (self.paddle + PADDLE_W / 2.0)) / (PADDLE_W / 2.0);
            let speed = self.vel.length();
            self.vel = Vec2::from_angle(-PI / 2.0 + off.clamp(-1.0, 1.0)) * speed;
            self.ball.y = PADDLE_Y - BALL_R;
        }
        // Bricks: which cell is the ball in now, and which was it in before?
        let cell = |p: Vec2| ((p.x as i32).div_euclid(BRICK_W), (p.y as i32 - WALL_TOP).div_euclid(BRICK_H));
        let (tx, ty) = cell(self.ball);
        let t = self.bricks.get(tx, ty);
        if t == 0 || t == b'.' {
            return false;
        }
        self.bricks.set(tx, ty, b'.');
        self.score += 10 * (WALL.h - ty) as u32;
        let (px, _) = cell(prev);
        if px != tx {
            self.vel.x = -self.vel.x; // came in from the side
        } else {
            self.vel.y = -self.vel.y;
        }
        self.ball = prev;
        self.vel *= 1.02; // a little faster every brick
        let centre = vec2(tx * BRICK_W + BRICK_W / 2, WALL_TOP + ty * BRICK_H + BRICK_H / 2);
        for _ in 0..8 {
            let vel = vec2(ctx.rand_range_f(-1.5, 1.5), ctx.rand_range_f(-2.0, 0.5));
            self.sparks.push(Spark { pos: centre, vel, life: ctx.rand_range(15, 35), color: brick_color(t) });
        }
        true
    }
}

impl Game for Bricks {
    const TITLE: &'static str = "Bricks";

    fn new(ctx: &mut Ctx) -> Self {
        let mut saved = [0u8; 4];
        let best = if ctx.load(0, &mut saved) == 4 { u32::from_le_bytes(saved) } else { 0 };
        Bricks {
            state: State::Title,
            bricks: Grid::from_map(&WALL),
            paddle: 140.0,
            ball: Vec2::ZERO,
            vel: Vec2::ZERO,
            score: 0,
            best,
            lives: 3,
            sparks: Vec::new(),
            mouse_x: ctx.mouse().x,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) {
        for s in self.sparks.iter_mut() {
            s.pos += s.vel;
            s.vel.y += 0.1;
            s.life -= 1;
        }
        self.sparks.retain(|s| s.life > 0);

        if matches!(self.state, State::Title | State::Won | State::Over) {
            if ctx.pressed(Button::A) || ctx.pressed(Button::Start) || ctx.mouse_pressed(MouseButton::Left) {
                self.new_game();
            }
            return;
        }

        // Paddle: keys, or follow the mouse whenever it moves.
        self.paddle += ctx.axis_x() as f32 * 4.0;
        let mouse = ctx.mouse();
        if mouse.x != self.mouse_x {
            self.paddle = mouse.x as f32 - PADDLE_W / 2.0;
            self.mouse_x = mouse.x;
        }
        self.paddle = self.paddle.clamp(0.0, ctx.width() as f32 - PADDLE_W);

        if self.state == State::Serve {
            self.ball = vec2(self.paddle + PADDLE_W / 2.0, PADDLE_Y - BALL_R - 1.0);
            if ctx.pressed(Button::A) || ctx.mouse_pressed(MouseButton::Left) {
                self.vel = vec2(ctx.rand_range_f(-1.2, 1.2), -2.5);
                self.state = State::Play;
            }
            return;
        }

        // Several small steps per update so a fast ball can't skip a brick.
        let step = self.vel / 4.0;
        for _ in 0..4 {
            if self.step_ball(ctx, step) {
                break;
            }
        }
        if self.ball.y > ctx.height() as f32 + 4.0 {
            self.lose_ball(ctx);
        } else if self.bricks.count(b'.') == self.bricks.tiles.len() {
            self.state = State::Won;
            self.save_best(ctx);
        }
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.clear(BLACK);
        for ty in 0..self.bricks.h {
            for tx in 0..self.bricks.w {
                let t = self.bricks.get(tx, ty);
                if t == b'.' {
                    continue;
                }
                let (x, y) = (tx * BRICK_W, WALL_TOP + ty * BRICK_H);
                g.fill_rect(x + 1, y + 1, BRICK_W - 2, BRICK_H - 2, brick_color(t));
                g.fill_rect(x + 1, y + 1, BRICK_W - 2, 1, WHITE); // shine
            }
        }
        for s in &self.sparks {
            g.pixel(s.pos.x, s.pos.y, s.color);
        }
        g.fill_rect(self.paddle, PADDLE_Y, PADDLE_W, 5, LIGHT_GRAY);
        g.fill_rect(self.paddle, PADDLE_Y, PADDLE_W, 1, WHITE);
        if matches!(self.state, State::Serve | State::Play) {
            g.fill_circle(self.ball.x, self.ball.y, BALL_R, WHITE);
        }

        text!(g, 4, 4, WHITE, "SCORE {}", self.score);
        text!(g, 130, 4, LIGHT_GRAY, "BEST {}", self.best);
        for i in 0..self.lives {
            g.fill_circle(310 - i as i32 * 8, 7, 2, RED);
        }

        let blink = g.frame() / 30 % 2 == 0;
        match self.state {
            State::Title => {
                let w = g.text_width("BRICKS") * 3;
                g.text_scaled((g.width() - w) / 2, 80, "BRICKS", ORANGE, 3);
                if blink {
                    g.text_centered(120, "Press Z or click to start", WHITE);
                }
            }
            State::Serve if blink => g.text_centered(120, "Press Z to serve", WHITE),
            State::Won => {
                g.text_centered(100, "YOU CLEARED THE WALL!", YELLOW);
                g.text_centered(120, "Press Z to play again", WHITE);
            }
            State::Over => {
                g.text_centered(100, "GAME OVER", RED);
                g.text_centered(120, "Press Z to play again", WHITE);
            }
            _ => {}
        }
    }

    fn debug(&self, out: &mut String) {
        let _ = write!(
            out,
            "state {:?} score {} lives {} bricks left {} ball ({:.0},{:.0}) vel ({:.2},{:.2}) paddle {:.0}",
            self.state,
            self.score,
            self.lives,
            self.bricks.tiles.len() - self.bricks.count(b'.'),
            self.ball.x,
            self.ball.y,
            self.vel.x,
            self.vel.y,
            self.paddle
        );
    }
}

hellforge::main!(Bricks);
