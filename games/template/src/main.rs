//! Template: a sprite you move around. Copy `games/template` to start a game.

#![no_std]
#![no_main]

use hellforge::prelude::*;

// Hex digits are the 16 base colours (a = YELLOW, 1 = DARK_BLUE), '.' is see-through.
const HERO: Sprite = sprite![
    "..aaaa..",
    ".aaaaaa.",
    "aa1aa1aa",
    "aaaaaaaa",
    "aa1111aa",
    ".aaaaaa.",
    "..a..a..",
    ".aa..aa.",
];

struct MyGame {
    pos: Vec2,
}

impl Game for MyGame {
    const TITLE: &'static str = "Template";

    fn new(ctx: &mut Ctx) -> Self {
        MyGame { pos: vec2(ctx.width() / 2, ctx.height() / 2) }
    }

    fn update(&mut self, ctx: &mut Ctx) {
        // Arrow keys or WASD; normalized() keeps diagonals the same speed.
        self.pos += ctx.axis().normalized() * 1.5;
        self.pos.x = self.pos.x.clamp(0.0, (ctx.width() - HERO.w) as f32);
        self.pos.y = self.pos.y.clamp(0.0, (ctx.height() - HERO.h) as f32);
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.clear(DARK_BLUE);
        g.sprite(&HERO, self.pos.x, self.pos.y);
        g.text(4, 4, "Arrows or WASD move, Esc quits", WHITE);
        text!(g, 4, 14, LIGHT_GRAY, "x={} y={}", self.pos.x as i32, self.pos.y as i32);
    }

    fn debug(&self, out: &mut String) {
        let _ = write!(out, "pos {:.1},{:.1}", self.pos.x, self.pos.y);
    }
}

hellforge::main!(MyGame);
