//! Crypt: a small first-person dungeon. Up/Down walk, Left/Right turn (hold
//! B = X key to side-step instead), A (Z, Space) fires the wand, hold Select
//! (Tab) for the map. Collect every gem, then step into the portal. Ghosts
//! chase you once they see you.

#![no_std]
#![no_main]

use hellforge::prelude::*;

// '#' stone, 'B' mossy stone, 'g' gem, 'G' ghost, 'P' start, 'X' portal.
// Positions below are in tile units: tile (3, 1) spans x 3.0..4.0, y 1.0..2.0.
const MAP: Tilemap = tilemap![
    "####################",
    "#P.....#.....B....g#",
    "#.####.#.###.B.##..#",
    "#.#g...#...#...#G..#",
    "#.#.######.#####.###",
    "#...B....#.....#...#",
    "###.B.##.###.#.###.#",
    "#g..B.#G.....#.....#",
    "#.###.#.######.###.#",
    "#.....#......G.#g..#",
    "#.#########.####.#.#",
    "#G..........#....#X#",
    "####################",
];

const STONE: Sprite = sprite!(
    legend: [('+', shade(Ramp::Gray, 10)), ('#', shade(Ramp::Gray, 7)), ('-', shade(Ramp::Gray, 3))],
    "+++++++-+++++++-",
    "#######-#######-",
    "#######-#######-",
    "----------------",
    "+++-+++++++-++++",
    "###-#######-####",
    "###-#######-####",
    "----------------",
    "+++++++-+++++++-",
    "#######-#######-",
    "#######-#######-",
    "----------------",
    "+++-+++++++-++++",
    "###-#######-####",
    "###-#######-####",
    "----------------",
);
const MOSS: Sprite = sprite!(
    legend: [('+', shade(Ramp::Green, 8)), ('#', shade(Ramp::Teal, 5)), ('-', shade(Ramp::Green, 2))],
    "+++++++-+++++++-",
    "#######-#######-",
    "#######-#######-",
    "----------------",
    "+++-+++++++-++++",
    "###-#######-####",
    "###-#######-####",
    "----------------",
    "+++++++-+++++++-",
    "#######-#######-",
    "#######-#######-",
    "----------------",
    "+++-+++++++-++++",
    "###-#######-####",
    "###-#######-####",
    "----------------",
);
const GEM: Sprite = sprite!["...cc...", "..cccc..", ".cc7ccc.", "cc7ccccc", ".cccccc.", "..cccc..", "...cc...", "........"];
const GHOST: Sprite = sprite![
    "..7777..",
    ".777777.",
    "77177177",
    "77777777",
    "77787777",
    "77777777",
    "77777777",
    "7.77.77.",
];
const PORTAL: Sprite = sprite!["..cccc..", ".c....c.", "c..dd..c", "c.d..d.c", "c.d..d.c", "c..dd..c", ".c....c.", "..cccc.."];
const WAND: Sprite = sprite!["....a...", "...aaa..", "....a...", "....4...", "....4...", "....4...", "....4...", "....4..."];

const RADIUS: f32 = 0.25;

fn solid(t: u8) -> bool {
    t == b'#' || t == b'B'
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Title,
    Play,
    Won,
    Over,
}

struct Ghost {
    pos: Vec2,
    hp: i32,
    flash: u32,
    bite: u32,
}

struct Crypt {
    state: State,
    grid: Grid,
    cam: Camera3d,
    gems: Vec<Vec2>,
    total_gems: usize,
    ghosts: Vec<Ghost>,
    portal: Vec2,
    health: i32,
    cooldown: u32,
    muzzle: u32,
    hurt: u32,
    time: u32,
    show_map: bool,
}

fn centre(tx: i32, ty: i32) -> Vec2 {
    vec2(tx as f32 + 0.5, ty as f32 + 0.5)
}

impl Crypt {
    fn start(&mut self) {
        self.grid = Grid::from_map(&MAP);
        let (px, py) = self.grid.find(b'P').unwrap_or((1, 1));
        self.cam = Camera3d::new(centre(px, py), 0.0);
        self.gems = self.grid.find_all(b'g').into_iter().map(|(x, y)| centre(x, y)).collect();
        self.total_gems = self.gems.len();
        self.ghosts = self.grid.find_all(b'G').into_iter().map(|(x, y)| Ghost { pos: centre(x, y), hp: 3, flash: 0, bite: 0 }).collect();
        let (xx, xy) = self.grid.find(b'X').unwrap_or((1, 1));
        self.portal = centre(xx, xy);
        for t in [b'P', b'g', b'G', b'X'] {
            self.grid.replace(t, b'.');
        }
        self.health = 5;
        self.time = 0;
        self.state = State::Play;
    }

    fn fire(&mut self) {
        self.cooldown = 15;
        self.muzzle = 4;
        let fwd = self.cam.forward();
        let wall = self.grid.cast_ray(self.cam.pos, fwd, 30.0, solid).map(|h| h.dist).unwrap_or(30.0);
        // the nearest ghost close enough to the line of fire, in front of the wall
        let mut best: Option<(usize, f32)> = None;
        for (i, g) in self.ghosts.iter().enumerate() {
            let rel = g.pos - self.cam.pos;
            let along = rel.dot(fwd);
            let off = rel.cross(fwd).abs();
            if along > 0.0 && along < wall && off < 0.35 && best.is_none_or(|(_, d)| along < d) {
                best = Some((i, along));
            }
        }
        if let Some((i, _)) = best {
            self.ghosts[i].hp -= 1;
            self.ghosts[i].flash = 6;
        }
        self.ghosts.retain(|g| g.hp > 0);
    }
}

impl Game for Crypt {
    const TITLE: &'static str = "Crypt";

    fn new(_ctx: &mut Ctx) -> Self {
        let mut c = Crypt {
            state: State::Title,
            grid: Grid::from_map(&MAP),
            cam: Camera3d::new(vec2(1.5, 1.5), 0.0),
            gems: Vec::new(),
            total_gems: 0,
            ghosts: Vec::new(),
            portal: Vec2::ZERO,
            health: 5,
            cooldown: 0,
            muzzle: 0,
            hurt: 0,
            time: 0,
            show_map: false,
        };
        c.start();
        c.state = State::Title;
        c
    }

    fn update(&mut self, ctx: &mut Ctx) {
        self.show_map = ctx.held(Button::Select);
        if self.state != State::Play {
            ctx.capture_mouse(false);
            if ctx.pressed(Button::A) || ctx.pressed(Button::Start) {
                self.start();
            }
            return;
        }
        ctx.capture_mouse(true);
        self.time += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        self.muzzle = self.muzzle.saturating_sub(1);
        self.hurt = self.hurt.saturating_sub(1);

        // turn (keys or mouse), walk, side-step with B held
        let strafe = ctx.held(Button::B);
        if !strafe {
            self.cam.angle += ctx.axis_x() as f32 * 0.05;
        }
        self.cam.angle += ctx.mouse().dx as f32 * 0.004;
        let mut step = self.cam.forward() * (-ctx.axis_y() as f32 * 0.06);
        if strafe {
            step += self.cam.right() * (ctx.axis_x() as f32 * 0.05);
        }
        self.grid.move_circle(&mut self.cam.pos, step, RADIUS, solid);

        if ctx.held(Button::A) && self.cooldown == 0 {
            self.fire();
        }

        // gems, then the portal
        let me = self.cam.pos;
        self.gems.retain(|g| g.distance(me) > 0.5);
        if self.gems.is_empty() && self.portal.distance(me) < 0.6 {
            self.state = State::Won;
        }

        // ghosts drift towards you once they can see you, and bite
        for g in self.ghosts.iter_mut() {
            g.flash = g.flash.saturating_sub(1);
            g.bite = g.bite.saturating_sub(1);
            let to_me = me - g.pos;
            if to_me.length() < 8.0 && self.grid.line_of_sight(g.pos, me, solid) {
                self.grid.move_circle(&mut g.pos, to_me.normalized() * 0.025, 0.3, solid);
            }
            if to_me.length() < 0.55 && g.bite == 0 {
                g.bite = 40;
                self.health -= 1;
                self.hurt = 10;
            }
        }
        if self.health <= 0 {
            self.state = State::Over;
        }
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.raycast(&self.grid, &self.cam, shade(Ramp::Gray, 2), shade(Ramp::Brown, 3), |t| match t {
            b'#' => Wall::Texture(&STONE),
            b'B' => Wall::Texture(&MOSS),
            _ => Wall::Empty,
        });
        let bob = (g.time() * 3.0).sin() * 0.05;
        let mut things: Vec<Billboard> = self.gems.iter().map(|&p| Billboard { size: 0.3, lift: 0.1 + bob, ..Billboard::new(&GEM, p) }).collect();
        for gh in &self.ghosts {
            things.push(Billboard { size: 0.75, lift: 0.1 - bob, ..Billboard::new(&GHOST, gh.pos) });
        }
        if self.gems.is_empty() {
            things.push(Billboard { size: 0.9, lift: 0.05, ..Billboard::new(&PORTAL, self.portal) });
        }
        g.billboards(&self.cam, &mut things);

        // the wand, the crosshair and the HUD
        let (cx, cy) = (g.width() / 2, g.height() / 2);
        g.line(cx - 3, cy, cx + 3, cy, WHITE);
        g.line(cx, cy - 3, cx, cy + 3, WHITE);
        let kick = if self.muzzle > 0 { 4 } else { 0 };
        g.sprite_scaled(&WAND, cx - 12, g.height() - 24 + kick, 3);
        if self.muzzle > 0 {
            g.fill_circle(cx, g.height() - 24 + kick, 4, YELLOW);
        }
        if self.hurt > 0 {
            g.fade(0.25);
        }
        text!(g, 4, 3, BLUE, "GEMS {}/{}", self.total_gems - self.gems.len(), self.total_gems);
        for i in 0..self.health {
            g.fill_circle(g.width() - 8 - i * 9, 7, 3, RED);
        }
        if self.gems.is_empty() && self.state == State::Play {
            g.text_centered(14, "The portal is open!", INDIGO);
        }

        // map: hold Select (Tab)
        if self.show_map && self.state == State::Play {
            let s = 5; // pixels per tile
            let (ox, oy) = (g.width() - self.grid.w * s - 4, 16);
            g.fill_rect(ox - 2, oy - 2, self.grid.w * s + 4, self.grid.h * s + 4, BLACK);
            g.grid(&self.grid, ox, oy, s, |g, t, x, y| {
                if solid(t) {
                    g.fill_rect(x, y, s, s, if t == b'B' { DARK_GREEN } else { DARK_GRAY });
                }
            });
            let at = |p: Vec2| vec2(ox, oy) + p * s as f32;
            for gm in &self.gems {
                let q = at(*gm);
                g.fill_rect(q.x - 1.0, q.y - 1.0, 2, 2, BLUE);
            }
            for gh in &self.ghosts {
                let q = at(gh.pos);
                g.fill_rect(q.x - 1.0, q.y - 1.0, 2, 2, RED);
            }
            let me = at(self.cam.pos);
            let nose = me + self.cam.forward() * 4.0;
            g.line(me.x, me.y, nose.x, nose.y, YELLOW);
            g.fill_rect(me.x - 1.0, me.y - 1.0, 3, 3, YELLOW);
        }
        match self.state {
            State::Title => {
                g.fade(0.5);
                let w = g.text_width("CRYPT") * 3;
                g.text_scaled((g.width() - w) / 2, 50, "CRYPT", INDIGO, 3);
                g.text_centered(85, "Find every gem, then the portal.", WHITE);
                g.text_centered(97, "Arrows move, Z fires, hold X to side-step", LIGHT_GRAY);
                if g.frame() / 30 % 2 == 0 {
                    g.text_centered(120, "Press Z to enter", WHITE);
                }
            }
            State::Won => {
                g.fade(0.5);
                g.text_centered(80, "YOU ESCAPED THE CRYPT", GREEN);
                text!(g, 120, 95, WHITE, "in {} seconds", self.time / 60);
                g.text_centered(115, "Press Z to play again", LIGHT_GRAY);
            }
            State::Over => {
                g.fade(0.6);
                g.text_centered(85, "THE GHOSTS GOT YOU", RED);
                g.text_centered(105, "Press Z to try again", LIGHT_GRAY);
            }
            State::Play => {}
        }
    }

    fn debug(&self, out: &mut String) {
        let _ = write!(
            out,
            "state {:?} pos ({:.2},{:.2}) angle {:.2} gems {}/{} ghosts {} health {}",
            self.state,
            self.cam.pos.x,
            self.cam.pos.y,
            self.cam.angle,
            self.total_gems - self.gems.len(),
            self.total_gems,
            self.ghosts.len(),
            self.health
        );
    }
}

hellforge::main!(Crypt);
