# Hellforge

A tiny, libre game engine in pure Rust for super-lightweight games, built
from the code and ideas of [Hellbyte](https://github.com/ZN-Black-Nexus/hellbyte).
It is designed to be easy for AI coding agents to use: one small API, assets
written as text, and a headless mode that lets an agent run a game and look at
its screen without a display.

* **One file per game and platform.** A game builds to a single static
  binary: Linux on almost any CPU without libc, and Windows without a C runtime
  (macOS is still to come). No data files, no dependencies.
* **Tiny.** No heap needed by the engine; the screen is 8 bits per pixel; a
  game runs in a few hundred KB of RAM.
* **Runs anywhere there's a screen or a terminal:** an X11 window (also
  XWayland and `ssh -X`), the Linux framebuffer, a native Windows window, or
  any terminal, including over SSH.
* **Made for AI agents** (and people): see [AGENTS.md](AGENTS.md).

```rust
#![no_std]
#![no_main]
use hellforge::prelude::*;

const HERO: Sprite = sprite![
    "..aaaa..",
    ".aaaaaa.",
    "aa1aa1aa",
    "aaaaaaaa",
    ".aa11aa.",
    "..aaaa..",
];

struct MyGame {
    pos: Vec2,
}

impl Game for MyGame {
    const TITLE: &'static str = "My Game";

    fn new(_ctx: &mut Ctx) -> Self {
        MyGame { pos: vec2(156, 86) }
    }

    fn update(&mut self, ctx: &mut Ctx) {
        self.pos += ctx.axis().normalized() * 1.5;
    }

    fn draw(&mut self, g: &mut Gfx) {
        g.clear(DARK_BLUE);
        g.sprite(&HERO, self.pos.x, self.pos.y);
        g.text(4, 4, "Arrows or WASD move, Esc quits", WHITE);
    }
}

hellforge::main!(MyGame);
```

## Quick start

```sh
scripts/new-game.sh snake "Snake"       # copy games/template to games/snake
cargo run -p snake                      # play it in a window
cargo run -q -p snake -- --term         # ...or in the terminal

# Run it without a screen: scripted input, a screenshot, a text view of the
# screen and your game's own debug output
cargo run -q -p snake -- --frames 90 --input "right:30 a down:20" \
    --shot /tmp/snake.png --ascii --debug
```

## What's in the box

| | |
|---|---|
| Screen | Any size up to 4096x4096 (default 320x180), 256-colour palette: 16 named colours + 15 ramps of 16 shades, changeable at run time |
| Drawing | Pixels, lines, rectangles, circles, triangles, polygons, a 5x8 font (with lowercase), sprites (flip, scale, tint, rotate), tile maps, camera, clipping, colour swaps, fades |
| Assets | Sprites and maps as ASCII art in your code (`sprite!`, `tilemap!`), checked at compile time |
| Input | Virtual buttons (arrows/WASD + Z X C V, Enter, Tab) with pressed/held/released, raw keys, mouse (position, buttons, wheel, captured mouse look) |
| Maths | `Vec2`, `Rect`, `sqrt`/`sin`/`cos`/`atan2`/`pow`/... for `f32` in pure Rust (identical on every CPU), random numbers |
| 3D | A first-person view of any tile map (raycaster): textured walls, floor and ceiling with fog, sprites standing in the world |
| Game help | Tile grids with box collision (`move_box`), first-person movement and rays, sprite animation, saves |
| Memory | `Vec`, `String`, `Box`, `format!` through a small built-in allocator |
| Testing | `--frames`, `--input` scripts, `--shot` PNG, `--ascii`, `--hash`, `--debug`, `--bench` |

Controls every game shares: arrows or WASD, Z X C V (buttons A B X Y),
Enter (Start), Tab (Select), Esc quits, Alt+Enter toggles fullscreen.

## Building for every platform

```sh
scripts/fetch-tools.sh            # once: nightly Rust + rust-src, qemu (tests), a few linkers
scripts/build-all.sh template     # every OS and CPU into dist/
scripts/test-qemu.sh template     # run the Linux builds on every CPU under qemu
```

The per-target linker settings live in `.cargo/config.toml`; Windows
binaries are built from Linux without its SDK. The template is about 125 KB
on Linux (static, no libc) and 90 KB on Windows, and runs in about 200 KB of
RAM.

## Layout

```
crates/hellforge      the engine (no_std): drawing, input, maths, runner, platforms
crates/hellforge-rt   memcpy and friends in Rust (no libc anywhere)
games/                one crate per game: template (the starting point), bricks
                      (breakout), jumper (platformer), starsweep (shooter),
                      crypt (first-person dungeon)
scripts/              new-game, build-all, test-qemu, tool fetching
AGENTS.md             the guide for AI agents (and humans)
```

## License

GPL-3.0-or-later, like Hellbyte. See `LICENSE`.
