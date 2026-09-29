# AGENTS.md: making games with Hellforge

This file is for AI coding agents (and humans in a hurry). It tells you how
to write, run and *check* a Hellforge game without guessing. Read it before
writing code.

## What Hellforge is

A tiny, dependency-free, pure-Rust game engine. A game is **one Rust type
that implements `Game`**. The engine gives you a small screen of palette
colours (default 320x180), input, a fixed 60 updates per second, text,
sprites drawn as ASCII art, tile maps, collision helpers, maths, random
numbers and saves. It builds to one small static binary per platform and
also runs headless, so you can test a game and look at its screen from the
command line.

## Hard rules

1. Every game lives in `games/<name>/` (a crate in this workspace).
2. `src/main.rs` starts with `#![no_std]` and `#![no_main]`, then
   `use hellforge::prelude::*;`, and ends with `hellforge::main!(MyGame);`.
3. **No external crates and no `std`.** Everything you need is in the
   prelude: `Vec`, `String`, `Box`, `vec!`, `format!`, `ToString`,
   `Write` (for `write!`), float maths (`sqrt`, `sin`, `cos`, `atan2`,
   `floor`, `powf`, `abs`, ...), `PI`, `TAU`, `log!`.
   Use `core::...` instead of `std::...` if you need something else from the
   core library (`core::mem::swap`, `core::cmp::Ordering`).
4. There is no `println!`: use `log!("x = {}", x)` (prints to stderr).
5. Colours are `u8` palette indices. Use the named colours (`WHITE`, `RED`,
   ...), `shade(Ramp::Blue, 10)`, or `rgb(r, g, b)` (works in `const`).
6. The screen is `WIDTH` x `HEIGHT` pixels, (0, 0) top-left, x right, y down.
   Drawing functions take any number type (`i32`, `f32`, `usize`, ...), so
   `g.fill_rect(self.x, self.y, 8, 8, RED)` works when `x` is an `f32`.
7. `update` changes state, `draw` only draws. Never skip `update` logic
   based on time measured by yourself: count updates (`ctx.frame()`) or use
   `ctx.dt()` (seconds per update).

## Workflow

```sh
scripts/new-game.sh snake "Snake"                    # copies games/template
cargo run -p snake                                   # play in a window (arrows, Z/X, Esc quits)
cargo run -q -p snake -- --term                      # play in the terminal

# Look at the game without a screen (do this after every change):
cargo run -q -p snake -- --frames 90 --input "right:30 a down:20" \
    --shot /tmp/snake.png --ascii --debug
```

Then **open `/tmp/snake.png`** (it is 2x size) and check it looks the way you
intended. If you cannot view images, read the `--ascii` output: one character
per block of pixels, `.` is the background colour and `0`-`f` are the 16
base colours (hex digits, same as in `sprite!`). `--ascii 160` gives more
detail. `--debug` prints whatever your `Game::debug` writes (positions,
score, state) - use it to check logic.

### Headless options

| Option | Meaning |
|---|---|
| `--frames N` | Run N updates, then stop (default: the length of `--input`, or 1). |
| `--input "STEPS"` | Scripted input. Steps are separated by spaces; each is `NAME[+NAME...][:FRAMES]` (default 1 frame). Names: buttons `up down left right a b x y start select`, keys `key.q` `key.space` `key.esc` `key.f1`..., mouse `click@X,Y` `rclick@X,Y` `mouse@X,Y` (screen pixels), or `wait`. Example: `"right:30 a+right:5 wait:10 click@160,90"`. The same button in two steps in a row becomes two separate presses. |
| `--shot FILE.png` | Save the final screen (2x; `--scale N` to change). With `--every N`, save every N updates; `{}` in the name becomes the update number. |
| `--ascii [COLS]` | Print the final screen as text (default 80 columns). |
| `--debug` | Print `Game::debug()` at the end. |
| `--hash` | Print a hash of the final screen. The same inputs always give the same hash (random numbers use `--seed`, default 1). |
| `--bench` | Speed and memory. |

A panic prints its message with file and line, e.g.
`panic: index out of bounds ... (at games/snake/src/main.rs:42:17)`.

## The API in one page

```rust
#![no_std]
#![no_main]
use hellforge::prelude::*;

struct MyGame { /* your state */ }

impl Game for MyGame {
    const TITLE: &'static str = "My Game";   // window title + save folder
    const WIDTH: u32 = 320;                   // screen size in pixels (optional)
    const HEIGHT: u32 = 180;
    const FPS: u32 = 60;                      // updates per second (optional)
    const QUIT_ON_ESCAPE: bool = true;        // (optional)

    fn new(ctx: &mut Ctx) -> Self { MyGame { } }
    fn update(&mut self, ctx: &mut Ctx) { }
    fn draw(&mut self, g: &mut Gfx) { }
    fn debug(&self, out: &mut String) { }    // optional, for --debug
}

hellforge::main!(MyGame);                     // or main!(MyGame, heap = 8 * 1024 * 1024)
```

**Input (`ctx`)**: `held(Button::A)`, `pressed(b)` (once per press),
`released(b)`, `axis_x()` / `axis_y()` (-1, 0, 1), `axis()` (a `Vec2`; use
`.normalized()` for diagonals), `any_pressed()`, `key_held(Key::Q)`,
`key_pressed(Key::SPACE)`, `mouse()` (`.x .y .dx .dy .wheel`, screen pixels),
`mouse_pressed(MouseButton::Left)`, `capture_mouse(true)` (mouse look).
Buttons: `Up Down Left Right` (arrows / WASD), `A` (Z, Space, J),
`B` (X, K), `X` (C, L), `Y` (V, I), `Start` (Enter, P), `Select` (Tab, M).

**Time**: `ctx.frame()` updates so far, `ctx.time()` seconds, `ctx.dt()`
seconds per update, `ctx.fps()`.

**Random**: `ctx.rand_range(0, 10)` (0..=9), `ctx.rand_range_f(-1.0, 1.0)`,
`ctx.rand_float()`, `ctx.chance(0.25)`, `ctx.rng().pick(&items)`,
`ctx.rng().shuffle(&mut v)`.

**Other**: `ctx.width()`, `ctx.height()`, `ctx.quit()`,
`ctx.save(slot, &bytes) -> bool`, `ctx.load(slot, &mut buf) -> usize`,
`ctx.load_vec(slot) -> Option<Vec<u8>>`.

**Drawing (`g`)**: `clear(c)`, `pixel(x, y, c)`, `get_pixel(x, y)`,
`line(x0, y0, x1, y1, c)`, `rect(x, y, w, h, c)` (outline), `fill_rect(...)`,
`circle(cx, cy, r, c)`, `fill_circle(...)`, `triangle(...)`,
`fill_triangle(x0, y0, x1, y1, x2, y2, c)`, `polygon(&[Vec2], c)`,
`fill_polygon(&[Vec2], c)`,
`text(x, y, "str", c) -> end_x` (5x8 font, `\n` allowed), `text_scaled(x, y, s, c, 2)`,
`text_centered(y, s, c)`, `text_outlined(x, y, s, c, outline)`,
`text_width(s)`, and `text!(g, x, y, c, "Score {}", score)` for formatting.
Sprites: `sprite(&S, x, y)`, `sprite_flipped(&S, x, y, flip_x, flip_y)`,
`sprite_scaled(&S, x, y, 3)`, `sprite_tinted(&S, x, y, WHITE)` (hit flash),
`sprite_rotated(&S, cx, cy, angle)`.
Maps: `tilemap(&MAP, x, y, 8, |g, tile, px, py| ...)`, `grid(&grid, ...)`.
View: `camera(x, y)` (scroll the world; set back to `(0, 0)` for the HUD),
`clip(x, y, w, h)`, `clip_reset()`.
Colours: `set_color(i, 0xRRGGBB)`, `reset_palette()`, `swap_color(from, to)`,
`reset_swaps()`, `fade(0.0..=1.0)` (darken everything drawn so far).
Raw: `pixels()`, `pixels_mut()`, `width()`, `height()`, `frame()`, `time()`.

**Colours**: `BLACK DARK_BLUE DARK_PURPLE DARK_GREEN BROWN DARK_GRAY
LIGHT_GRAY WHITE RED ORANGE YELLOW GREEN BLUE INDIGO PINK PEACH` are 0-15.
`shade(Ramp::Green, 0..=15)` gives 16 shades (dark to light) of `Gray Red
Orange Yellow Lime Green Teal Cyan Sky Blue Violet Purple Magenta Rose Brown`.
`rgb(255, 128, 0)` / `hex(0xff8000)` picks the nearest palette colour.

**Sprites** are ASCII art: `.` or space = transparent, `0`-`9` `a`-`f` = the
16 base colours (hex digits: `7` white, `8` red, `a` yellow, `b` green, `c`
blue). All rows the same width.

```rust
const SHIP: Sprite = sprite![
    "...77...",
    "..7cc7..",
    ".777777.",
    "78.77.87",
];
const BRICK: Sprite = sprite!(legend: [('#', shade(Ramp::Brown, 7))], "####", "#..#", "####");
const WALK: [Sprite; 2] = [sprite!["7.", ".7"], sprite![".7", "7."]];
g.sprite(anim(&WALK, g.frame(), 8), x, y);   // in draw: anim(frames, tick, ticks_per_frame)
```

**Tile maps** are text. The tile is the character's byte (`b'#'`).

```rust
const LEVEL: Tilemap = tilemap![
    "################",
    "#..............#",
    "#..P.....o.....#",
    "################",
];
let mut grid = Grid::from_map(&LEVEL);        // mutable copy
let (tx, ty) = grid.find(b'P').unwrap();       // tile coordinates
grid.set(tx, ty, b'.');
// move a box, sliding along walls; `hit.bottom` = standing on ground
let hit = grid.move_box(&mut pos, &mut vel, vec2(6, 8), 8, |t| t == b'#');
```

**Maths**: `Vec2` (`vec2(x, y)`, `+ - * /`, `length()`, `normalized()`,
`dot`, `angle()`, `rotated(a)`, `from_angle(a)`, `distance(o)`, `lerp`,
`approach`), `Rect` (`rect(x, y, w, h)`, `overlaps(&r)`, `contains(x, y)`,
`center()`), `lerp`, `approach(from, to, step)`, `wrap(v, max)`, `clamp`
(built into `f32`/`i32`). Angles are radians, 0 = right, `PI / 2` = down.

## Patterns

```rust
// Game states
enum State { Title, Playing, GameOver }
// in update: match self.state { State::Title => if ctx.pressed(Button::A) { ... } ... }

// Many objects: a Vec of structs, removed with retain
self.bullets.retain(|b| b.pos.y > -8.0);

// Timers: count updates
self.cooldown = self.cooldown.saturating_sub(1);
if ctx.held(Button::A) && self.cooldown == 0 { self.fire(); self.cooldown = 10; }

// Collision between two boxes
if player_rect.overlaps(&enemy_rect) { ... }

// Blinking text (in draw)
if g.frame() / 20 % 2 == 0 { g.text_centered(120, "PRESS Z", WHITE); }

// Scrolling: draw the world with a camera, then the HUD without it
g.camera(self.cam_x, 0);   /* world */   g.camera(0, 0);   /* HUD */

// Screen shake: pick an offset in update, apply it in draw
self.shake = if self.shake_time > 0 { (ctx.rand_range(-2, 3), ctx.rand_range(-2, 3)) } else { (0, 0) };
g.camera(self.shake.0, self.shake.1);
```

## Common mistakes

| Symptom | Fix |
|---|---|
| `undefined symbol: __hellforge_entry` | Add `hellforge::main!(MyGame);` at the end of `main.rs`. |
| `cannot find macro vec/format` or type `Vec`/`String` | `use hellforge::prelude::*;` |
| `failed to resolve: use of unresolved module std` | No `std`: use the prelude or `core::`. |
| `no method named sqrt/sin found for f32` | Import the prelude (it brings the `Float` trait). |
| `mismatched types: expected u8` for a colour | Colours are `u8`: `WHITE`, `shade(...)`, `rgb(...)`. |
| `sprite!/tilemap!: every row must have the same number of characters` | Make all rows of the art equally long. |
| `sprite!: unknown character` | Only `.`, space, `0-9`, `a-f` unless you add a `legend:`. |
| `panic: memory allocation of N bytes failed` | Raise the heap: `hellforge::main!(MyGame, heap = 8 * 1024 * 1024);` |
| Movement faster diagonally | `ctx.axis().normalized()` |
| A press counts every update | Use `ctx.pressed(b)` (not `held`) for one-shot actions. |

## Before you say you're done

1. `cargo build -p NAME` has no warnings.
2. A headless run with realistic input looks right in `--shot` (and `--ascii`).
3. `--debug` shows sane state after a long run, e.g.
   `--frames 3600 --input "right:600 a:5 left:600"` (no panic).
4. The title/start, playing and game-over/restart paths all work.

## Building for every platform

`scripts/build-all.sh NAME` builds the game for Linux (30 CPUs, static, no
libc), Windows (x86_64, x86, ARM64) and macOS (universal) into `dist/`. It
needs `scripts/fetch-tools.sh` once (nightly Rust and a few linkers).
`scripts/test-qemu.sh NAME` runs it on every CPU under qemu and compares the
`--hash` with the native build.

## Engine layout (only if you change the engine)

`crates/hellforge/src/`: `lib.rs` (Game, prelude, main!), `gfx.rs` (drawing),
`color.rs`, `font.rs`, `sprite.rs`, `tilemap.rs`, `input.rs`, `math.rs`,
`rng.rs`, `runner.rs` (Ctx, main loop, headless mode), `heap.rs`
(allocator), `platform/` (Linux, Windows, macOS, terminal). Run
`cargo test -p hellforge` after engine changes.
